#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_migrations;

use std::sync::Arc;

use chrono::NaiveDateTime;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::SqliteConnection;
use eyre::Result;
use eyre::{Context, ContextCompat};
use futures_util::StreamExt;
use tap::Pipe;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

use sg_core::models::Event;
use sg_core::mq::{MessageQueue, Middlewares, RabbitMQ};
use sg_core::utils::FigmentExt;

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

    let config = Config::from_env("MIDDLEWARE_")
        .wrap_err("Failed to load config from environment variables")?;

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

    while let Some(Ok((next, event))) = consumer.next().await {
        let event_id = event.id;
        info!(%event_id, ?next, "Received event");

        if let Err(error) = handle_event(next, event, &scheduler) {
            error!(%event_id, ?error, "Failed to process event");
        }
    }
    Ok(())
}

fn handle_event(next: Middlewares, mut event: Event, scheduler: &Arc<Scheduler>) -> Result<()> {
    let id = event
        .fields
        .remove("x-delay-id")
        .wrap_err("Missing `x-delay-at`")?
        .as_i64()
        .wrap_err("Not a integer: `x-delay-at`")?;

    let cancel = if let Some(cancel) = event.fields.remove("x-delay-cancel") {
        // If `x-delay-cancel` is set to true, we cancel the task
        cancel
            .as_bool()
            .wrap_err("Not a boolean: `x-delay-cancel`")?
    } else {
        // There's no `x-delay-cancel` field, so this is a new delayed message
        false
    };

    if cancel {
        scheduler.remove_task(id);
    } else {
        let deliver_at = event
            .fields
            .remove("x-delay-at")
            .wrap_err("Missing `x-delay-at`")?
            .as_i64()
            .wrap_err("Not a timestamp: `x-delay-at`")?
            .pipe(|ts| NaiveDateTime::from_timestamp(ts, 0));

        let msg = DelayedMessage::new(id, next, event, deliver_at);
        scheduler.add_task(msg, true);
    }

    Ok(())
}
