//! Message queue for the Twitter worker.

use eyre::Result;
use lapin::options::{BasicPublishOptions, ExchangeDeclareOptions};
use lapin::types::FieldTable;
use lapin::{BasicProperties, Channel, Connection, ConnectionProperties, ExchangeKind};
use uuid::Uuid;

use crate::twitter::Tweet;

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
    pub async fn publish(&self, entity_id: Uuid, tweet: Tweet) -> Result<()> {
        drop(
            self.channel
                .basic_publish(
                    "events",
                    &format!("{}.twitter", entity_id),
                    BasicPublishOptions::default(),
                    &*serde_json::to_vec(&tweet)?,
                    BasicProperties::default(),
                )
                .await?,
        );
        Ok(())
    }
}
