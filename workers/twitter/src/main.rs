//! Twitter worker binary.

#![allow(clippy::module_name_repetitions)]
#![deny(missing_docs)]

use base::start_worker;
use eyre::{Result, WrapErr};
use sg_core::{mq::RabbitMQ, utils::FigmentExt};
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

    let worker = TwitterWorker::new(config.clone(), mq);
    start_worker(worker, config.node_config).await?;

    Ok(())
}
