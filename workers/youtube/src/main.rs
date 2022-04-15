#![allow(clippy::module_name_repetitions)]

use std::sync::Arc;

use eyre::{Result, WrapErr};
use parking_lot::RwLock;
use tracing_subscriber::EnvFilter;

use sg_core::mq::RabbitMQ;
use sg_core::protocol::WorkerRpcExt;

use crate::config::Config;
use crate::registry::Registry;
use crate::server::serve;
use crate::worker::YoutubeWorker;

mod config;
mod models;
mod registry;
mod server;
mod worker;

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let config = Config::from_env().wrap_err("Failed to load config from environment variables")?;

    let mq = RabbitMQ::new(&config.amqp_url, &config.amqp_exchange)
        .await
        .wrap_err("Failed to connect to AMQP")?;

    let registry = Arc::new(RwLock::new(Registry::new(config.clone())));

    let worker_fut = YoutubeWorker::new(config.clone(), registry.clone()).join(
        config.coordinator_url.clone(),
        config.id,
        "youtube",
    );
    let server = serve(&config, registry, mq);

    tokio::select! {
        Err(e) = worker_fut => Err(e.wrap_err("Failed to start worker")),
        Err(e) = server => Err(e.wrap_err("Failed to start server")),
        else => Ok(())
    }
}
