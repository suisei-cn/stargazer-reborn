use std::{marker::PhantomData, sync::Arc};

use color_eyre::{eyre::Context, Result};
use futures::{stream::FuturesUnordered, StreamExt};
use once_cell::sync::OnceCell;
use sg_api::client::Client;
use sg_core::{
    models::Event,
    mq::{MessageQueue, RabbitMQ},
};
use teloxide::{
    adaptors::{AutoSend, DefaultParseMode},
    prelude::*,
    types::{ChatId, ParseMode, Recipient},
    Bot as TeloxideBot,
};
use tokio::select;
use tracing::{debug, error, info};

use crate::{answer, config::Config, Command};

type Bot = AutoSend<DefaultParseMode<TeloxideBot>>;

static BOT: OnceCell<Bot> = OnceCell::new();
static CLIENT: OnceCell<Client> = OnceCell::new();

/// Start the service.
///
/// # Errors
/// If the service fails to start or any error occurred during the service.
pub async fn start() -> Result<()> {
    let config = Config::from_env()?;
    init(&config).await?;

    select! {
        res = start_bot() => {
            error!("Bot quit");
            res?;
        }
        res = start_event_handler(&config) => {
            error!("Event handler quit");
            res?;
        }
    }
    Ok(())
}

async fn init(config: &Config) -> Result<()> {
    let reqwest_client = reqwest::Client::new();
    let bot = TeloxideBot::with_client(&config.bot_token, reqwest_client.clone())
        .parse_mode(ParseMode::Html)
        .auto_send();
    let me = bot.get_me().await?;
    info!(username = %me.username(), "Telegram Bot API logged in");
    BOT.set(bot).expect("Bot is already initialized");

    let mut client = Client::with_client(reqwest_client, config.api_url.clone())?;
    client
        .login_and_store(&config.api_username, &config.api_password)
        .await?;
    CLIENT.set(client).expect("Client is already initialized");

    Ok(())
}

async fn start_bot() -> Result<()> {
    let bot = BOT.get().expect("Bot is not initialized").clone();

    teloxide::commands_repl(bot, answer, PhantomData::<Command>).await;
    Ok(())
}

async fn start_event_handler(config: &Config) -> Result<()> {
    let mq = RabbitMQ::new(&config.amqp_url, &config.amqp_exchange).await?;
    let mut stream = mq.consume(None).await;

    while let Some(res) = stream.next().await {
        let (middlewares, event) = res?;
        if !middlewares.is_empty() {
            debug!("Unexpected middlewares, skip handling");
            continue;
        }
        tokio::spawn(async move {
            if let Err(error) = handle_event(event).await {
                error!(%error, "Failed to handle event");
            }
        });
    }

    Ok(())
}

async fn handle_event(event: Event) -> Result<()> {
    let Event {
        id,
        kind,
        entity,
        fields,
    } = event;
    debug!(%id, %kind, %entity, ?fields, "Handling event");

    let client = CLIENT.get().expect("Client not initialized");
    let interest = client.get_interest(entity, kind, "telegram").await?;
    let bot = BOT.get().expect("Bot not initialized");

    let text = "Test"; // TODO: implement composing message

    let mut stream = interest
        .users
        .into_iter()
        .map(|user| async move {
            let cid: i64 = user.im_payload.parse().wrap_err("Bad chat id")?;
            let res = bot.send_message(Recipient::Id(ChatId(cid)), text).await?;
            debug!(chat = %res.chat.id, id = res.id, "Message sent");
            Result::<_>::Ok(())
        })
        .collect::<FuturesUnordered<_>>();

    while let Some(res) = stream.next().await {
        if let Err(error) = res {
            error!(%error, "Failed to send message");
        }
    }

    Ok(())
}
