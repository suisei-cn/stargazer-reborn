use color_eyre::Result;
use once_cell::sync::Lazy;
use reqwest::StatusCode;
use sg_api::model::UserQuery;
use teloxide::{prelude::*, types::Message, utils::command::BotCommands};
use tracing::{debug, info};

use crate::{get_chat_avatar, is_admin, use_bot, use_bot_username, use_client, Bot, TokenExt};

#[derive(Debug, Clone, BotCommands)]
#[command(rename = "lowercase", description = "These commands are supported")]
pub enum Command {
    Start,
    #[command(description = "Display this text.")]
    Help,
    #[command(description = "Register in the system.")]
    Register,
    #[command(description = "Set preferences.")]
    Setting,
    #[command(description = "Delete your account.")]
    Unregister {
        confirmation: String,
    },
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

impl Command {
    pub(crate) async fn handle(self, msg: Message) -> Result<()> {
        let bot = use_bot();
        let reply = make_reply!(bot, msg);

        debug!(?self, ?msg, "Received command");
        if matches!(self, Command::Help | Command::Start) {
            static DESCRIPTION: Lazy<String> = Lazy::new(|| {
                Command::descriptions()
                    .username(use_bot_username())
                    .to_string()
            });

            return reply(DESCRIPTION.clone()).await;
        }

        if is_admin(&msg, bot).await? {
            match self {
                Command::Register => handle_register(bot, msg).await,
                Command::Setting => handle_setting(bot, msg).await,
                Command::Unregister { confirmation } => {
                    handle_unregister(confirmation, bot, msg).await
                }
                Command::Help | Command::Start => unreachable!(),
            }
        } else {
            reply("Admin privilege is required for this action.".to_owned()).await
        }
    }
}

async fn handle_register(bot: &Bot, msg: Message) -> Result<()> {
    let client = use_client();

    let reply = make_reply!(bot, msg);

    let chat_id = msg.chat.id.to_string();
    let username = msg.chat.username();
    let avatar = if let Some(username) = username {
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
        Ok(user) => {
            info!(?user, "New user");
            let token = client
                .new_token(UserQuery::ByIm {
                    im: "telegram".to_owned(),
                    im_payload: msg.chat.id.to_string(),
                })
                .await?;

            let behalf = match username {
                Some(username) => format!("@{}", username),
                None => "This chat".to_string(),
            };

            reply(format!(
                "{} is now registered! Use <a href=\"{}\">this link</a> to start subscribing (expires in {})",
                behalf,
                token.as_url(),
                token.valid_until_formatted()?
            ))
            .await?;
        }
        // When user already exists, we just generate a new token
        Err(error)
            if error
                .as_api()
                .map_or(false, |api| api.matches_status(StatusCode::CONFLICT)) =>
        {
            let token = client
                .new_token(UserQuery::ByIm {
                    im: "telegram".to_owned(),
                    im_payload: msg.chat.id.to_string(),
                })
                .await?;

            let behalf = match username {
                Some(username) => format!("@{}", username),
                None => "This chat".to_string(),
            };

            reply(format!(
                "{} has already been registered! Use <a href=\"{}\">this link</a> to update preference (expires in {})",
                behalf,
                token.as_url(),
                token.valid_until_formatted()?
            )).await?;
        }
        // Other errors
        Err(error) => {
            reply("Internal error".to_owned()).await?;
            return Err(error.into());
        }
    };
    Ok(())
}

async fn handle_setting(bot: &Bot, msg: Message) -> Result<()> {
    let reply = make_reply!(bot, msg);

    match use_client()
        .new_token(UserQuery::ByIm {
            im: "telegram".to_owned(),
            im_payload: msg.chat.id.to_string(),
        })
        .await
    {
        Ok(token) => {
            reply(format!(
                "Use <a href=\"{}\">this link</a> to update setting (expires in {})",
                token.as_url(),
                token.valid_until_formatted()?
            ))
            .await
        }
        Err(error) if error.matches_api_status(StatusCode::NOT_FOUND) => {
            reply("You have not been registered yet! Call /register first.".to_owned()).await
        }
        Err(error) => {
            reply("Internal error".to_owned()).await?;
            Err(error.into())
        }
    }
}

async fn handle_unregister(confirmation: String, bot: &Bot, msg: Message) -> Result<()> {
    const CONFIRMATION: &str = "confirm";
    const GET_CONCENT: &str = "Please use `/unregister confirm` to confirm deleting account";

    let reply = make_reply!(bot, msg);
    let client = use_client();

    match confirmation.to_lowercase().as_str().trim() {
        CONFIRMATION => {
            let chat_id = msg.chat.id.to_string();
            let res = client
                .del_user(UserQuery::ByIm {
                    im: "telegram".to_owned(),
                    im_payload: chat_id,
                })
                .await;

            match res {
                Ok(_) => {
                    reply("Account deleted".to_owned()).await?;
                }
                Err(error) if error.matches_api_status(StatusCode::NOT_FOUND) => {
                    reply("This chat is not registered.".to_owned()).await?;
                }
                Err(error) => {
                    reply("Internal error".to_owned()).await?;

                    return Err(error.into());
                }
            }
        }
        _ => {
            reply(GET_CONCENT.to_owned()).await?;
        }
    }

    Ok(())
}
