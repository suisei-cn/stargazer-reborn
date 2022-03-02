use eyre::Result;
use lapin::options::{BasicPublishOptions, ExchangeDeclareOptions};
use lapin::types::FieldTable;
use lapin::{BasicProperties, Channel, Connection, ConnectionProperties, ExchangeKind};
use uuid::Uuid;

use crate::models::Tweet;

pub struct MessageQueue {
    channel: Channel,
}

impl MessageQueue {
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
    pub async fn publish(&self, entity_id: Uuid, tweet: Tweet) -> Result<()> {
        drop(
            self.channel
                .basic_publish(
                    "events",
                    &format!("{}.twitter", entity_id),
                    BasicPublishOptions::default(),
                    &*serde_json::to_vec(&tweet).unwrap(),
                    BasicProperties::default(),
                )
                .await?,
        );
        Ok(())
    }
}
