use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use assert_cmd::cargo::CommandCargoExt;
use futures_util::StreamExt;
use serde_json::json;
use tokio::time::{sleep, timeout};
use uuid::Uuid;

use sg_core::models::Event;
use sg_core::mq::{MessageQueue, Middlewares, RabbitMQ};

#[tokio::test]
async fn must_delay_and_send() {
    // Initialize messages to send and expect.
    let delay_at = SystemTime::now() + Duration::from_secs(5);
    let ts = delay_at.duration_since(UNIX_EPOCH).unwrap().as_secs();
    let original = Event::from_serializable_with_id(
        Uuid::nil(),
        "",
        Uuid::nil(),
        json!({
            "a": "b",
            "x-delay-id": 114_514,
            "x-delay-at": ts
        })).unwrap();
    let expected = Event::from_serializable_with_id(
        Uuid::nil(),
        "",
        Uuid::nil(),
        json!({
            "a": "b",
        })).unwrap();

    // Connect to MQ.
    let mq = RabbitMQ::new("amqp://guest:guest@localhost:5672", "test")
        .await
        .unwrap();
    let mut consumer = mq.consume(Some("delay_debug")).await;

    // Start delay middleware.
    let mut program = Command::cargo_bin("delay")
        .unwrap()
        .env("MIDDLEWARE_AMQP_URL", "amqp://guest:guest@localhost:5672")
        .env("MIDDLEWARE_AMQP_EXCHANGE", "test")
        .env("MIDDLEWARE_DATABASE_URL", ":memory:")
        .spawn()
        .unwrap();
    sleep(Duration::from_secs(1)).await;

    // Publish a test message.
    mq.publish(original, "delay_debug.delay".parse().unwrap())
        .await
        .unwrap();

    // Receive the delayed message and check its content & deliver time.
    let msg = consumer.next().await.unwrap().unwrap();
    let received_time = SystemTime::now();
    assert_eq!(msg, (Middlewares::default(), expected));
    let delta = time_diff_abs(delay_at, received_time);
    assert!(delta < Duration::from_millis(1500));

    // There must be only one message.
    assert!(timeout(Duration::from_secs(1), consumer.next())
        .await
        .is_err());

    // Shutdown the middleware.
    program.kill().unwrap();
}

#[tokio::test]
async fn must_delay_and_send_across_restart() {
    // Prepare temp dir.
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("test.db");

    // Initialize messages to send and expect.
    let delay_at = SystemTime::now() + Duration::from_secs(7);
    let ts = delay_at.duration_since(UNIX_EPOCH).unwrap().as_secs();
    let original = Event::from_serializable_with_id(
        Uuid::nil(),
        "",
        Uuid::nil(),
        json!({
            "a": "b",
            "x-delay-id": 114_514,
            "x-delay-at": ts
        }),
    )
    .unwrap();
    let expected = Event::from_serializable_with_id(
        Uuid::nil(),
        "",
        Uuid::nil(),
        json!({
            "a": "b",
        }),
    )
    .unwrap();

    // Connect to MQ.
    let mq = RabbitMQ::new("amqp://guest:guest@localhost:5672", "test")
        .await
        .unwrap();
    let mut consumer = mq.consume(Some("delay_persist_debug")).await;

    // Start delay middleware.
    let mut program = Command::cargo_bin("delay")
        .unwrap()
        .env("MIDDLEWARE_AMQP_URL", "amqp://guest:guest@localhost:5672")
        .env("MIDDLEWARE_AMQP_EXCHANGE", "test")
        .env("MIDDLEWARE_DATABASE_URL", &db_path)
        .spawn()
        .unwrap();
    sleep(Duration::from_secs(1)).await;

    // Publish a test message.
    mq.publish(original, "delay_persist_debug.delay".parse().unwrap())
        .await
        .unwrap();
    // Ensure the message is received and processed by the middleware.
    sleep(Duration::from_secs(1)).await;

    // Kill the middleware and restart
    program.kill().unwrap();
    let mut program = Command::cargo_bin("delay")
        .unwrap()
        .env("MIDDLEWARE_AMQP_URL", "amqp://guest:guest@localhost:5672")
        .env("MIDDLEWARE_AMQP_EXCHANGE", "test")
        .env("MIDDLEWARE_DATABASE_URL", &db_path)
        .spawn()
        .unwrap();

    // Receive the delayed message and check its content & deliver time.
    let msg = consumer.next().await.unwrap().unwrap();
    let received_time = SystemTime::now();
    assert_eq!(msg, (Middlewares::default(), expected));
    let delta = time_diff_abs(delay_at, received_time);
    assert!(delta < Duration::from_millis(1500));

    // There must be only one message.
    assert!(timeout(Duration::from_millis(500), consumer.next())
        .await
        .is_err());

    // Shutdown the middleware.
    program.kill().unwrap();
}

fn time_diff_abs(a: SystemTime, b: SystemTime) -> Duration {
    match a.duration_since(b) {
        Ok(delta) => delta,
        Err(e) => e.duration(),
    }
}
