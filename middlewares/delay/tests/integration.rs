use std::process::Command;
use std::str::FromStr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use assert_cmd::cargo::CommandCargoExt;
use futures_util::StreamExt;
use serde_json::json;
use tokio::time::{sleep, timeout};
use tracing::level_filters::LevelFilter;
use uuid::Uuid;

use sg_core::models::Event;
use sg_core::mq::{MessageQueue, Middlewares, RabbitMQ};

#[tokio::test]
async fn must_delay_and_send() {
    tracing_subscriber::fmt()
        .with_max_level(LevelFilter::DEBUG)
        .init();

    let delay_at = SystemTime::now() + Duration::from_secs(5);
    let ts = delay_at.duration_since(UNIX_EPOCH).unwrap().as_secs();

    let original = Event {
        id: Uuid::nil().into(),
        kind: "".to_string(),
        entity: Uuid::nil().into(),
        fields: json!({
            "a": "b",
            "x-delay-id": 114_514,
            "x-delay-at": ts
        })
        .as_object()
        .unwrap()
        .clone(),
    };
    let expected = Event {
        id: Uuid::nil().into(),
        kind: "".to_string(),
        entity: Uuid::nil().into(),
        fields: json!({
            "a": "b",
        })
        .as_object()
        .unwrap()
        .clone(),
    };

    let mut cmd = Command::cargo_bin("delay").unwrap();
    let mut program = cmd
        .env("MIDDLEWARE_AMQP_URL", "amqp://guest:guest@localhost:5672")
        .env("MIDDLEWARE_AMQP_EXCHANGE", "test")
        .env("MIDDLEWARE_DATABASE_URL", ":memory:")
        .spawn()
        .unwrap();

    let mq = RabbitMQ::new("amqp://guest:guest@localhost:5672", "test")
        .await
        .unwrap();
    let mut consumer = mq.consume(Some("delay_debug")).await;

    sleep(Duration::from_secs(1)).await;

    mq.publish(
        original,
        Middlewares::from_str("delay_debug.delay").unwrap(),
    )
    .await
    .unwrap();

    let msg = consumer.next().await.unwrap().unwrap();
    let received_time = SystemTime::now();
    assert_eq!(msg, (Middlewares::default(), expected));
    let delta = match received_time.duration_since(delay_at) {
        Ok(delta) => delta,
        Err(e) => e.duration(),
    };
    assert!(dbg!(delta) < Duration::from_millis(1500));

    // There's only one message.
    assert!(timeout(Duration::from_millis(500), consumer.next())
        .await
        .is_err());

    program.kill().unwrap();
}
