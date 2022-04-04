//! Message queue for workers.

use std::convert::Infallible;
use std::fmt::{Display, Formatter};
use std::str::FromStr;
use std::{iter, vec};

use eyre::Result;
use futures_util::{future, stream, Stream, StreamExt};
use itertools::Itertools;
use lapin::options::{
    BasicConsumeOptions, BasicPublishOptions, ExchangeDeclareOptions, QueueBindOptions,
    QueueDeclareOptions,
};
use lapin::types::FieldTable;
use lapin::{BasicProperties, Channel, Connection, ConnectionProperties, Consumer, ExchangeKind};
use tap::TapFallible;
use tracing::{debug, error, info};

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
                "stargazer-reborn",
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
    pub async fn publish(&self, event: Event, middlewares: Middlewares) -> Result<()> {
        info!(event_id = %event.id, event_kind = %event.kind, ?middlewares, "Publishing event");
        drop(
            self.channel
                .basic_publish(
                    "stargazer-reborn",
                    &iter::once(String::from("event"))
                        .chain(middlewares.into_iter())
                        .join("."),
                    BasicPublishOptions::default(),
                    &*serde_json::to_vec(&event)?,
                    BasicProperties::default(),
                )
                .await?,
        );
        Ok(())
    }
    async fn consume_connect(&self, middleware: Option<&str>) -> Result<Consumer> {
        let routing_key = middleware.map_or_else(
            || String::from("event"),
            |middleware| format!("#.{}", middleware),
        );
        let queue = self
            .channel
            .queue_declare(
                "",
                QueueDeclareOptions {
                    exclusive: true,
                    ..Default::default()
                },
                FieldTable::default(),
            )
            .await?;
        self.channel
            .queue_bind(
                queue.name().as_str(),
                "stargazer-reborn",
                &routing_key,
                QueueBindOptions::default(),
                FieldTable::default(),
            )
            .await?;
        Ok(self
            .channel
            .basic_consume(
                queue.name().as_str(),
                middleware.unwrap_or(""),
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await?)
    }
    /// Consume messages from the message queue.
    ///
    /// # Errors
    /// Returns an error if the message can't be consumed.
    pub async fn consume(
        &self,
        middleware: Option<&str>,
    ) -> impl Stream<Item = Result<(Middlewares, Event)>> + Unpin {
        let consumer = self.consume_connect(middleware).await;
        info!(middleware = ?middleware, "Listening for events.");
        match consumer {
            Ok(consumer) => future::Either::Left(consumer.map(|msg| match msg {
                Ok(msg) => Ok((
                    Middlewares::from_routing_key(msg.routing_key.as_str()),
                    serde_json::from_slice(&msg.data).tap_err(|e| {
                        error!(routing_key = %msg.routing_key, error = ?e, "Failed to parse event");
                    })?,
                )),
                Err(e) => {
                    error!(error = ?e, "Error consuming message.");
                    Err(e.into())
                }
            })),
            Err(e) => future::Either::Right(stream::once(future::ready(Err(e)))),
        }
    }
}

/// A set of middlewares.
#[derive(Debug, Default)]
pub struct Middlewares {
    middlewares: Vec<String>,
}

impl Middlewares {
    /// Obtain a middleware set from a routing key, removing its first and last component.
    #[must_use]
    pub fn from_routing_key(s: &str) -> Self {
        let mut middlewares: Vec<_> = s.split('.').skip(1).map(ToString::to_string).collect();
        middlewares.pop();
        Self { middlewares }
    }
}

impl FromStr for Middlewares {
    type Err = Infallible;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Ok(Self {
            middlewares: s.split('.').map(ToString::to_string).collect(),
        })
    }
}

impl Display for Middlewares {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.middlewares.join("."))
    }
}

impl IntoIterator for Middlewares {
    type Item = String;
    type IntoIter = vec::IntoIter<String>;

    fn into_iter(self) -> Self::IntoIter {
        self.middlewares.into_iter()
    }
}
