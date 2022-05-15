use color_eyre::Result;
use once_cell::sync::Lazy;
use regex::Regex;
use sg_api::model::UserQuery;
use teloxide::{prelude::*, types::Message, utils::command::BotCommands};
use tracing::{debug, trace};

use crate::{get_bot_username, get_client, Bot, TokenExt};

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
    Unregister { confirmation: String },
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

macro_rules! make_reply {
    ($bot:ident,$msg:ident) => {{
        |reply: String| async {
            $bot.send_message($msg.chat.id, reply)
                .reply_to_message_id($msg.id)
                .send()
                .await?;
            Result::<()>::Ok(())
        }
    }};
}

/// Answer to command
#[allow(clippy::missing_errors_doc)]
#[allow(clippy::missing_panics_doc)]
pub async fn answer(bot: Bot, msg: Message, command: Command) -> Result<()> {
    let reply = make_reply!(bot, msg);

    debug!(?command, ?msg, "Received command");

    match command {
        Command::Help => {
            static DESCRIPTION: Lazy<String> = Lazy::new(|| {
                Command::descriptions()
                    .username(get_bot_username())
                    .to_string()
            });
            reply(DESCRIPTION.clone()).await?;
        }
        Command::Register => handle_register(bot, msg).await?,
        Command::Setting => handle_setting(bot, msg).await?,
        Command::Unregister { confirmation } => handle_unregister(confirmation, bot, msg).await?,
    };

    Ok(())
}

async fn handle_register(bot: Bot, msg: Message) -> Result<()> {
    let client = get_client();

    let reply = make_reply!(bot, msg);

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
                .expect("Command in private chat must have `from`")
                .full_name()
        },
        ToOwned::to_owned,
    );
    match client.add_user("telegram", chat_id, avatar, name).await {
        Ok(_) => {
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
        Err(error) => {
            match error.as_api() {
                Some(api_err) if api_err.error[0].as_str() == "Conflict" => {}
                _ => {
                    reply("Internal error".to_owned()).await?;
                    return Err(error.into());
                }
            }
            let token = client
                .new_token(UserQuery::ByIm {
                    im: "telegram".to_owned(),
                    im_payload: msg.chat.id.to_string(),
                })
                .await?;

            reply(format!(
                "This account has already been registered! Use <a href=\"https://stargazer.sh/?token={}\">this link</a> to update preference (valid for {})",
                token.token,
                token.valid_until_formatted()?
            )).await?;
        }
    };
    Ok(())
}

async fn handle_setting(bot: Bot, msg: Message) -> Result<()> {
    let reply = make_reply!(bot, msg);

    let token = get_client()
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

    Ok(())
}

async fn handle_unregister(confirmation: String, bot: Bot, msg: Message) -> Result<()> {
    const CONFIRMATION: &str = "confirm";
    const GET_CONFIRM: &str = "Please use `/unregister confirm` to confirm deleting account";

    let reply = make_reply!(bot, msg);

    let client = get_client();

    match confirmation.to_lowercase().as_str().trim() {
        CONFIRMATION => {
            let chat_id = msg.chat.id.to_string();
            match client
                .del_user(UserQuery::ByIm {
                    im: "telegram".to_owned(),
                    im_payload: chat_id,
                })
                .await
            {
                Ok(_) => {
                    reply("Account deleted".to_owned()).await?;
                }
                Err(error)
                    if error
                        .as_api()
                        .map_or(false, |x| x.error[0].as_str() == "Not Found") =>
                {
                    reply("This account is not registered yet".to_owned()).await?;
                }
                Err(error) => {
                    reply("Internal error".to_owned()).await?;

                    return Err(error.into());
                }
            }
        }
        _ => {
            reply(GET_CONFIRM.to_owned()).await?;
        }
    }

    Ok(())
}

#[tokio::test]
async fn test_get_chat_avatar() {
    let url = get_chat_avatar("durov").await.unwrap().unwrap();
    println!("{}", url);
}
