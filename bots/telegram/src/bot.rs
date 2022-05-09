use color_eyre::{eyre::ensure, Result};
use futures::StreamExt;
use sg_api::client::Client;
use sg_core::mq::{MessageQueue, RabbitMQ};
use teloxide::{
    adaptors::{AutoSend, DefaultParseMode},
    prelude::*,
    types::ParseMode,
    Bot as TeloxideBot,
};
use tokio::select;
use tracing::{error, info};

use crate::config::Config;

type Bot = AutoSend<DefaultParseMode<TeloxideBot>>;

pub struct TgBot<B> {
    config: Config,
    bot: B,
    client: Client,
}

impl TgBot<Bot> {
    pub async fn from_env() -> Result<Self> {
        let config = Config::from_env()?;
        let bot = TeloxideBot::new(&config.bot_token)
            .parse_mode(ParseMode::Html)
            .auto_send();
        let me = bot.get_me().await?;
        info!(username = %me.username(), "Telegram Bot API logged in");
        let client = Client::new(config.api_url.clone())?;

        Ok(Self::new(config, bot, client))
    }

    async fn start_bot(&self) -> Result<()> {
        Ok(())
    }

    async fn start_event_handler(&self) -> Result<()> {
        let mq = RabbitMQ::new(&self.config.amqp_url, &self.config.amqp_exchange).await?;
        let mut stream = mq.consume(None).await;

        while let Some(Ok((middlewares, event))) = stream.next().await {
            // ensure!(middlewares.is_empty(), "unexpected middlewares"); // TODO: impl Deref<[String]> for Middlewares
        }

        Ok(())
    }

    pub async fn start(self) -> Result<()> {
        select! {
            res = self.start_bot() => {
                error!("Bot quit");
                res?;
            },
            res = self.start_event_handler() => {
                error!("Event handler quit");
                res?;
            },
        }
        Ok(())
    }
}

impl<B> TgBot<B> {
    pub fn new(config: Config, bot: B, client: Client) -> Self {
        Self {
            config,
            bot,
            client,
        }
    }
}
