//! Twitter worker binary.

#![allow(clippy::module_name_repetitions)]
#![deny(missing_docs)]

use eyre::{Result, WrapErr};
use tracing_subscriber::EnvFilter;

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
    tracing_subscriber::fmt().with_env_filter(EnvFilter::from_default_env()).init();

    let config = Config::from_env().wrap_err("Failed to load config from environment variables")?;

    let mq = MessageQueue::new(&*config.amqp_url).await.wrap_err("Failed to connect to AMQP")?;

    TwitterWorker::new(config.clone(), mq)
        .join(config.coordinator_url, config.id, "twitter")
        .await.wrap_err("Failed to start worker")?;

    Ok(())
}
