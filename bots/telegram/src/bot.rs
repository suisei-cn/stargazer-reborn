use std::marker::PhantomData;

use color_eyre::{eyre::Context, Result};
use futures::{stream::FuturesUnordered, StreamExt};
use sg_core::{
    models::Event,
    mq::{MessageQueue, RabbitMQ},
};
use teloxide::{
    adaptors::DefaultParseMode,
    prelude::*,
    types::{ChatId, Recipient},
    Bot as TeloxideBot,
};
use tokio::select;
use tracing::{debug, error, info};

use crate::{config::Config, Command};

pub type Bot = DefaultParseMode<TeloxideBot>;

mod statics {
    use color_eyre::Result;
    use once_cell::sync::OnceCell;
    use sg_api::client::Client;
    use teloxide::{prelude::*, types::ParseMode, Bot as TeloxideBot};
    use tracing::{info, warn};

    use crate::{Bot, Config};

    static BOT: OnceCell<Bot> = OnceCell::new();
    static CLIENT: OnceCell<Client> = OnceCell::new();
    static BOT_USERNAME: OnceCell<String> = OnceCell::new();
    static CONFIG: OnceCell<Config> = OnceCell::new();

    #[must_use]
    pub fn use_bot<'a>() -> &'a Bot {
        BOT.get().expect("Bot is not initialized")
    }

    #[must_use]
    pub fn use_client<'a>() -> &'a Client {
        CLIENT.get().expect("Client is not initialized")
    }

    #[must_use]
    pub fn use_bot_username<'a>() -> &'a str {
        BOT_USERNAME.get().expect("Bot username is not initialized")
    }

    #[must_use]
    pub fn use_config<'a>() -> &'a Config {
        CONFIG.get().expect("Config is not initialized")
    }

    pub async fn try_init(config: Config) -> Result<()> {
        let reqwest_client = reqwest::Client::new();

        let bot = TeloxideBot::with_client(&config.tg_token, reqwest_client.clone())
            .parse_mode(ParseMode::Html);
        let me = bot.get_me().send().await?;
        info!(username = %me.username(), "Telegram Bot API logged in");
        if BOT_USERNAME.set(me.username().to_owned()).is_err() {
            warn!("`init()` has been executed for multiple times");
        };

        drop(BOT.set(bot));

        let mut client = Client::with_client(reqwest_client, config.api_url.clone())?;
        client
            .login_and_store(&config.api_username, &config.api_password)
            .await?;
        info!(username = %config.api_username, "API logged in");
        drop(CLIENT.set(client));

        drop(CONFIG.set(config));

        Ok(())
    }

    pub async fn init(config: Config) {
        try_init(config).await.expect("Init failed");
    }

    pub async fn try_init_from_env() -> Result<()> {
        let config = Config::from_env()?;
        try_init(config).await
    }

    pub async fn init_from_env() {
        try_init_from_env().await.expect("Init from env failed");
    }
}

pub use statics::*;

/// Start the service.
///
/// # Errors
/// If the service fails to start or any error occurred during the service.
pub async fn start() -> Result<()> {
    let config = Config::from_env()?;
    init(config).await;

    select! {
        _ = tokio::signal::ctrl_c() => {
            info!("Received SIGINT, exiting...");
        }
        res = start_bot() => {
            error!("Bot quit");
            res?;
        }
        res = start_event_handler() => {
            error!("Event handler quit");
            res?;
        }
    }
    Ok(())
}

async fn start_bot() -> Result<()> {
    let bot = use_bot().clone();

    teloxide::commands_repl(
        bot,
        |msg, cmd: Command| async move { cmd.handle(msg).await },
        PhantomData::<Command>,
    )
    .await;

    Ok(())
}

async fn start_event_handler() -> Result<()> {
    let config = use_config();
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

    let client = use_client();
    let interest = client.get_interest(entity, kind, "telegram").await?;
    let bot = use_bot();

    let text = "Test"; // TODO: implement composing message

    let mut stream = interest
        .users
        .into_iter()
        .map(|user| async move {
            let cid: i64 = user.im_payload.parse().wrap_err("Bad chat id")?;
            let res = bot
                .send_message(Recipient::Id(ChatId(cid)), text)
                .send()
                .await?;
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

#[tokio::test]
async fn test_bot() {
    use tracing::level_filters::LevelFilter;

    let level = std::env::var("TG_LOG")
        .as_deref()
        .unwrap_or("info")
        .parse::<LevelFilter>()
        .unwrap();

    tracing_subscriber::fmt().with_max_level(level).init();
    dotenv::dotenv().unwrap();
    color_eyre::install().unwrap();

    tokio::spawn(sg_api::server::serve());
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    init_from_env().await;
    start_bot().await.unwrap();
}
