#![allow(clippy::module_name_repetitions)]

use eyre::Result;

use sg_core::protocol::WorkerRpcExt;

use crate::config::Config;
use crate::mq::MessageQueue;
use crate::worker::TwitterWorker;

mod config;
mod models;
mod mq;
mod twitter;
mod worker;

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    tracing_subscriber::fmt().init();

    let config = Config::from_env()?;

    let mq = MessageQueue::new(&*config.amqp_url).await?;

    TwitterWorker::new(config.twitter_token, mq)
        .join(config.coordinator_url, config.id, "twitter")
        .await?;

    Ok(())
}
