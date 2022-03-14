#![allow(clippy::module_name_repetitions)]

mod bililive;
mod config;
mod worker;

use crate::config::Config;
use crate::worker::BililiveWorker;
use eyre::{Result, WrapErr};
use sg_core::mq::MessageQueue;
use sg_core::protocol::WorkerRpcExt;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let config = Config::from_env().wrap_err("Failed to load config from environment variables")?;

    let mq = MessageQueue::new(&*config.amqp_url)
        .await
        .wrap_err("Failed to connect to AMQP")?;

    BililiveWorker::new(mq)
        .join(config.coordinator_url, config.id, "bililive")
        .await
        .wrap_err("Failed to start worker")?;

    Ok(())
}
