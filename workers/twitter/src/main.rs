//! Twitter worker binary.

#![allow(clippy::module_name_repetitions)]
#![deny(missing_docs)]

use eyre::Result;

use sg_core::protocol::WorkerRpcExt;

use crate::config::Config;
use crate::mq::MessageQueue;
use crate::worker::TwitterWorker;

pub mod config;
pub mod mq;
pub mod twitter;
pub mod worker;

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    tracing_subscriber::fmt().init();

    let config = Config::from_env()?;

    let mq = MessageQueue::new(&*config.amqp_url).await?;

    TwitterWorker::new(config.clone(), mq)
        .join(config.coordinator_url, config.id, "twitter")
        .await?;

    Ok(())
}
