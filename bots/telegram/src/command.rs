use color_eyre::Result;
use once_cell::sync::{Lazy, OnceCell};
use regex::Regex;
use sg_api::model::UserQuery;
use teloxide::{prelude::*, types::Message, utils::command::BotCommands};
use tracing::{debug, trace};

use crate::{Bot, TokenExt, CLIENT};

#[derive(Debug, Clone, BotCommands)]
#[command(rename = "lowercase", description = "These commands are supported")]
pub enum Command {
    #[command(description = "Display this text.")]
    Help,
    #[command(description = "Register in the system.")]
    Register,
    #[command(description = "Set preferences.")]
    Setting,
    #[command(description = "Delete your account.")]
    DeleteAccount { confirmation: String },
}

fn match_url(text: &str) -> Option<&str> {
    static PATTERN: Lazy<Regex> =
        Lazy::new(|| Regex::new(r#"<img class="tgme_page_photo_image" src="(.*?)""#).unwrap());

    Some(PATTERN.captures(text)?.get(1)?.as_str())
}

async fn get_chat_avatar(username: &str) -> Result<Option<reqwest::Url>> {
    let html = reqwest::get(format!("https://t.me/{}", username))
        .await?
        .text()
        .await?;
    trace!("{}", html);
    Ok(match_url(&html).and_then(|url| url.parse().ok()))
}

/// Answer to command
#[allow(clippy::missing_errors_doc)]
#[allow(clippy::missing_panics_doc)]
pub async fn answer(bot: Bot, msg: Message, command: Command) -> Result<()> {
    let client = CLIENT.get().expect("Client is not initialized");
    let reply = |reply: String| async {
        bot.send_message(msg.chat.id, reply)
            .reply_to_message_id(msg.id)
            .send()
            .await?;
        Result::<()>::Ok(())
    };

    debug!(?command, ?msg, "Received command");

    match command {
        Command::Help => {
            static BOT_USER_NAME: OnceCell<Option<String>> = OnceCell::new();
            let mut description = Command::descriptions();
            match BOT_USER_NAME.get() {
                Some(Some(username)) => description = description.username(username),
                Some(None) => {}
                None => {
                    let me = bot.get_me().send().await?;
                    drop(BOT_USER_NAME.set(me.user.username));
                    // Safe b/c just initialized
                    if let Some(username) = unsafe { BOT_USER_NAME.get_unchecked() } {
                        description = description.username(username);
                    }
                }
            };
            reply(description.to_string()).await?;
        }
        Command::Register => {
            let chat_id = msg.chat.id.to_string();
            let avatar = if let Some(username) = msg.chat.username() {
                get_chat_avatar(username).await?
            } else {
                None
            };

            // Title is available in all public chats
            // If it is not available, it means that the chat is private
            let name = msg.chat.title().map_or_else(
                || {
                    msg.from()
                        .expect("Command in private chat must be sent from someone")
                        .full_name()
                },
                ToOwned::to_owned,
            );
            client.add_user("telegram", chat_id, avatar, name).await?;
            let token = client
                .new_token(UserQuery::ByIm {
                    im: "telegram".to_owned(),
                    im_payload: msg.chat.id.to_string(),
                })
                .await?;

            reply(format!(
                    "Registered! Use <a href=\"https://stargazer.sh/?token={}\">this link</a> to start subscribing (valid for {})",
                    token.token,
                    token.valid_until_formatted()?
                )
            ).await?;
        }
        Command::Setting => {
            let token = client
                .new_token(UserQuery::ByIm {
                    im: "telegram".to_owned(),
                    im_payload: msg.chat.id.to_string(),
                })
                .await?;

            reply(format!(
                "Use <a href=\"https://stargazer.sh/?token={}\">this link</a> to update setting (valid for {})",
                token.token,
                token.valid_until_formatted()?
            )).await?;
        }
        Command::DeleteAccount { confirmation } => {
            const CONFIRMATION: &str = "confirm";
            const GET_CONFIRM: &str =
                "Please use <code>/delete_account confirm</code> to confirm deleting account";

            match confirmation.to_lowercase().as_str().trim() {
                CONFIRMATION => {
                    let chat_id = msg.chat.id.to_string();
                    client
                        .del_user(UserQuery::ByIm {
                            im: "telegram".to_owned(),
                            im_payload: chat_id,
                        })
                        .await?;
                    reply("Account deleted".to_owned()).await?;
                }
                _ => {
                    reply(GET_CONFIRM.to_owned()).await?;
                }
            }
        }
    };

    Ok(())
}

#[tokio::test]
async fn test_get_chat_avatar() {
    let url = get_chat_avatar("durov").await.unwrap().unwrap();
    println!("{}", url);
}
