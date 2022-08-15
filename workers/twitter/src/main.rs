//! Twitter worker binary.

#![allow(clippy::module_name_repetitions)]
#![deny(missing_docs)]

use eyre::{Result, WrapErr};
use sg_core::{mq::RabbitMQ, protocol::WorkerRpcExt, utils::FigmentExt};
use tracing_subscriber::EnvFilter;

use crate::{config::Config, worker::TwitterWorker};

pub mod config;
pub mod twitter;
pub mod worker;

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let config =
        Config::from_env("WORKER_").wrap_err("Failed to load config from environment variables")?;

    let mq = RabbitMQ::new(&config.amqp_url, &config.amqp_exchange)
        .await
        .wrap_err("Failed to connect to AMQP")?;

    TwitterWorker::new(config.clone(), mq)
        .join(config.coordinator_url, config.id, "twitter")
        .await
        .wrap_err("Failed to start worker")?;

    Ok(())
}
