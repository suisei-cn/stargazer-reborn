//! Message queue for workers.

use std::convert::Infallible;
use std::fmt::{Display, Formatter};
use std::ops::Deref;
use std::pin::Pin;
use std::str::FromStr;
use std::{iter, vec};

use async_trait::async_trait;
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

/// Interface of a message queue.
#[async_trait]
pub trait MessageQueue: Send + Sync {
    /// Publish a tweet to the message queue.
    ///
    /// # Errors
    /// Returns an error if the message can't be published.
    async fn publish(&self, event: Event, middlewares: Middlewares) -> Result<()>;
    /// Consume messages from the message queue.
    ///
    /// # Errors
    /// Returns an error if the message can't be consumed.
    async fn consume(
        &self,
        middleware: Option<&str>,
    ) -> Pin<Box<dyn Stream<Item = Result<(Middlewares, Event)>> + Send>>;
}

#[async_trait]
impl<T: Deref<Target = dyn MessageQueue> + Send + Sync> MessageQueue for T {
    async fn publish(&self, event: Event, middlewares: Middlewares) -> Result<()> {
        self.deref().publish(event, middlewares).await
    }

    async fn consume(
        &self,
        middleware: Option<&str>,
    ) -> Pin<Box<dyn Stream<Item = Result<(Middlewares, Event)>> + Send>> {
        self.deref().consume(middleware).await
    }
}

/// A message queue backed by `RabbitMQ`.
pub struct RabbitMQ {
    exchange: String,
    channel: Channel,
}

impl RabbitMQ {
    /// Connect to a `RabbitMQ` server.
    ///
    /// # Errors
    /// Returns an error if the connection fails or the exchange can't be declared.
    pub async fn new(addr: &str, exchange: &str) -> Result<Self> {
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
                exchange,
                ExchangeKind::Topic,
                ExchangeDeclareOptions {
                    durable: true,
                    ..Default::default()
                },
                FieldTable::default(),
            )
            .await?;

        Ok(Self {
            exchange: exchange.to_string(),
            channel,
        })
    }
    async fn consumer_connect(&self, middleware: Option<&str>) -> Result<Consumer> {
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
                &self.exchange,
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
}

#[async_trait]
impl MessageQueue for RabbitMQ {
    async fn publish(&self, event: Event, middlewares: Middlewares) -> Result<()> {
        info!(event_id = %event.id, event_kind = %event.kind, ?middlewares, "Publishing event");
        drop(
            self.channel
                .basic_publish(
                    &self.exchange,
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

    async fn consume(
        &self,
        middleware: Option<&str>,
    ) -> Pin<Box<dyn Stream<Item = Result<(Middlewares, Event)>> + Send>> {
        let consumer = self.consumer_connect(middleware).await;
        info!(middleware = ?middleware, "Listening for events.");
        match consumer {
            Ok(consumer) => Box::pin(consumer.map(|msg| match msg {
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
            Err(e) => Box::pin(stream::once(future::ready(Err(e)))),
        }
    }
}

/// A set of middlewares.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
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

/// Mock implementations.
#[cfg(any(test, feature = "mock"))]
pub mod mock {
    use std::pin::Pin;

    use async_trait::async_trait;
    use eyre::Result;
    use futures_util::{Stream, StreamExt, TryStreamExt};
    use tokio::sync::broadcast;
    use tokio_stream::wrappers::BroadcastStream;

    use crate::models::Event;
    use crate::mq::{MessageQueue, Middlewares};

    /// A mock message queue.
    pub struct MockMQ {
        tx: broadcast::Sender<(String, Event)>,
    }

    impl Default for MockMQ {
        fn default() -> Self {
            let (tx, _) = broadcast::channel(128);
            Self { tx }
        }
    }

    #[async_trait]
    impl MessageQueue for MockMQ {
        async fn publish(&self, event: Event, middlewares: Middlewares) -> Result<()> {
            let key = if middlewares.middlewares.is_empty() {
                "events".to_string()
            } else {
                format!("events.{}", middlewares)
            };
            self.tx.send((key, event))?;
            Ok(())
        }

        async fn consume(
            &self,
            middleware: Option<&str>,
        ) -> Pin<Box<dyn Stream<Item = Result<(Middlewares, Event)>> + Send>> {
            let interested = middleware.map(std::string::ToString::to_string);
            Box::pin(
                BroadcastStream::new(self.tx.subscribe())
                    .try_filter_map(move |(key, event)| {
                        let interested = interested.clone();
                        async move {
                            Ok(match interested {
                                Some(middleware) if key.ends_with(&format!(".{}", middleware)) => {
                                    Some((Middlewares::from_routing_key(&key), event))
                                }
                                None if !key.contains('.') => {
                                    Some((Middlewares::from_routing_key(&key), event))
                                }
                                _ => None,
                            })
                        }
                    })
                    .map(|item| Ok(item?)),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use futures_util::StreamExt;
    use mongodb::bson::Uuid;
    use serde_json::json;
    use tokio::time::timeout;

    use crate::models::Event;
    #[cfg(feature = "mock")]
    use crate::mq::mock::MockMQ;
    use crate::mq::{MessageQueue, Middlewares, RabbitMQ};

    #[tokio::test]
    async fn tests() {
        let mq = RabbitMQ::new("amqp://guest:guest@localhost:5672", "test")
            .await
            .unwrap();
        must_seq(&mq).await;
        must_filter(&mq).await;

        #[cfg(feature = "mock")]
        {
            let mq = MockMQ::default();
            must_seq(&mq).await;
            must_filter(&mq).await;
        }
    }

    async fn must_filter(mq: &impl MessageQueue) {
        let msg_a = Event::from_serializable("a", Uuid::new(), json!({"k": "va"})).unwrap();
        let msg_b = Event::from_serializable("b", Uuid::new(), json!({"k": "vb"})).unwrap();
        let msg_c = Event::from_serializable("c", Uuid::new(), json!({"k": "vc"})).unwrap();

        let mut bare_consumer = mq.consume(None).await;
        let mut mw_consumer = mq.consume(Some("mq_filter_test")).await;

        mq.publish(msg_a.clone(), Middlewares::default())
            .await
            .unwrap();
        mq.publish(msg_b.clone(), "mq_filter_test".parse().unwrap())
            .await
            .unwrap();
        mq.publish(msg_c.clone(), "nested.mq_filter_test".parse().unwrap())
            .await
            .unwrap();
        mq.publish(
            msg_a.clone(),
            "mq_filter_test.some_other_mw".parse().unwrap(),
        )
        .await
        .unwrap();

        assert_eq!(
            bare_consumer.next().await.unwrap().unwrap(),
            (Middlewares::default(), msg_a.clone()),
            "bare consumer should receive the first message"
        );
        assert!(
            timeout(Duration::from_millis(500), bare_consumer.next())
                .await
                .is_err(),
            "bare consumer should receive nothing"
        );

        assert_eq!(
            mw_consumer.next().await.unwrap().unwrap(),
            (Middlewares::default(), msg_b.clone()),
            "mw consumer should receive the second message"
        );
        assert_eq!(
            mw_consumer.next().await.unwrap().unwrap(),
            ("nested".parse().unwrap(), msg_c.clone()),
            "mw consumer should receive the third message"
        );
        assert!(
            timeout(Duration::from_millis(500), mw_consumer.next())
                .await
                .is_err(),
            "mw consumer should receive nothing"
        );
    }

    async fn must_seq(mq: &impl MessageQueue) {
        let mut consumer = mq.consume(Some("mq_seq_test")).await;

        for i in 1..100usize {
            mq.publish(
                Event::from_serializable(&i.to_string(), Uuid::new(), json!({})).unwrap(),
                "mq_seq_test".parse().unwrap(),
            )
            .await
            .unwrap();
        }

        for i in 1..100usize {
            let (_, e) = consumer.next().await.unwrap().unwrap();
            assert_eq!(
                e.kind,
                &*i.to_string(),
                "messages should be received in sequence"
            );
        }
    }
}
