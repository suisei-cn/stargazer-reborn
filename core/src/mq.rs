//! Message queue for workers.

use eyre::Result;
use lapin::options::{BasicPublishOptions, ExchangeDeclareOptions};
use lapin::types::FieldTable;
use lapin::{BasicProperties, Channel, Connection, ConnectionProperties, ExchangeKind};
use tracing::{debug, info};

use crate::models::Event;

/// A connection to a `RabbitMQ` server.
pub struct MessageQueue {
    channel: Channel,
}

impl MessageQueue {
    /// Connect to a `RabbitMQ` server.
    ///
    /// # Errors
    /// Returns an error if the connection fails or the exchange can't be declared.
    pub async fn new(addr: &str) -> Result<Self> {
        let channel = Connection::connect(
            addr,
            ConnectionProperties::default()
                .with_executor(tokio_executor_trait::Tokio::current())
                .with_reactor(tokio_reactor_trait::Tokio),
        )
        .await?
        .create_channel()
        .await?;

        debug!("Declaring exchange");
        channel
            .exchange_declare(
                "events",
                ExchangeKind::Topic,
                ExchangeDeclareOptions {
                    durable: true,
                    ..Default::default()
                },
                FieldTable::default(),
            )
            .await?;

        Ok(Self { channel })
    }

    /// Publish a tweet to the message queue.
    ///
    /// # Errors
    /// Returns an error if the message can't be published.
    pub async fn publish(&self, event: Event) -> Result<()> {
        info!(event_id = %event.id, event_kind = %event.kind, "Publishing event");
        drop(
            self.channel
                .basic_publish(
                    "stargazer-reborn",
                    "events",
                    BasicPublishOptions::default(),
                    &*serde_json::to_vec(&event)?,
                    BasicProperties::default(),
                )
                .await?,
        );
        Ok(())
    }
}
