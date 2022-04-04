use eyre::{Result, WrapErr};
use futures_util::StreamExt;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

use sg_core::mq::{MessageQueue, RabbitMQ};

use crate::config::Config;
use crate::translate::{BaiduTranslator, Translator};

mod config;
mod translate;

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let config = Config::from_env().wrap_err("Failed to load config from environment variables")?;

    let translator = BaiduTranslator::new(config.baidu_app_id, config.baidu_app_secret);

    let mq = RabbitMQ::new(&config.amqp_url, &config.amqp_exchange)
        .await
        .wrap_err("Failed to connect to AMQP")?;

    let mut consumer = mq.consume(Some("translate")).await;

    while let Some(Ok((next, event))) = consumer.next().await {
        info!(event_id = %event.id, ?next, "Received event");
        let event = match translator.translate_event(event.clone()).await {
            Ok(translated) => translated,
            Err(e) => {
                error!(?e, "Failed to translate event, ignore");
                event
            }
        };
        if let Err(error) = mq.publish(event, next).await {
            error!(?error, "Failed to publish translated event");
        }
    }

    Ok(())
}
