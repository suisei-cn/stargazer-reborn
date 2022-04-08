#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_migrations;

use std::sync::Arc;

use chrono::NaiveDateTime;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::SqliteConnection;
use eyre::Context;
use eyre::Result;
use futures_util::StreamExt;
use tracing::info;
use tracing_subscriber::EnvFilter;

use sg_core::mq::{MessageQueue, RabbitMQ};

use crate::config::Config;
use crate::db::DelayedMessage;
use crate::scheduler::Scheduler;
use crate::schema::delayed_messages::dsl::delayed_messages;

mod config;
mod db;
mod scheduler;
mod schema;

embed_migrations!();

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let config = Config::from_env().wrap_err("Failed to load config from environment variables")?;

    let pool = Pool::new(ConnectionManager::<SqliteConnection>::new(
        &config.database_url,
    ))
    .wrap_err("Failed to connect to SQLite database")?;

    embedded_migrations::run(&pool.get()?).wrap_err("Failed to run migration script")?;

    let mq = RabbitMQ::new(&config.amqp_url, &config.amqp_exchange)
        .await
        .wrap_err("Failed to connect to AMQP")?;
    let mut consumer = mq.consume(Some("delay")).await;

    let scheduler = Arc::new(Scheduler::new(pool, mq));
    scheduler.cleanup();
    scheduler.load();

    while let Some(Ok((next, mut event))) = consumer.next().await {
        info!(event_id = %event.id, ?next, "Received event");

        if let (Some(id), Some(deliver_at)) = (
            event.fields.remove("x-delay-id").and_then(|v| v.as_i64()),
            event
                .fields
                .remove("x-delay-at")
                .and_then(|v| v.as_i64())
                .map(|i| NaiveDateTime::from_timestamp(i, 0)),
        ) {
            let delayed_message = DelayedMessage::new(id, next, event, deliver_at);
            scheduler.add_task(delayed_message, true);
        }
    }
    Ok(())
}
