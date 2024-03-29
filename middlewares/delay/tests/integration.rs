use std::{
    process::Command,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use assert_cmd::cargo::CommandCargoExt;
use futures_util::StreamExt;
use rstest::rstest;
use serde_json::{json, Value};
use sg_core::{
    models::Event,
    mq::{MessageQueue, Middlewares, RabbitMQ},
};
use tokio::time::{sleep, timeout};
use uuid::Uuid;

#[rstest]
#[case(json ! ({"a": "b"}), json ! ({"a": "b"}))]
#[case(json ! ({"a": "b", "x-delay-cancel": false}), json ! ({"a": "b"}))]
#[tokio::test(flavor = "multi_thread")]
async fn must_delay_and_send(#[case] mut event: Value, #[case] expected_event: Value) {
    let exchange_name = format!("test_{}", rand::random::<usize>());

    // Initialize messages to send and expect.
    let delay_at = SystemTime::now() + Duration::from_secs(5);
    let ts = delay_at.duration_since(UNIX_EPOCH).unwrap().as_secs();
    event["x-delay-id"] = json!(114_514);
    event["x-delay-at"] = json!(ts);
    let original = Event::from_serializable_with_id(Uuid::nil(), "", Uuid::nil(), event).unwrap();
    let expected =
        Event::from_serializable_with_id(Uuid::nil(), "", Uuid::nil(), expected_event).unwrap();

    // Connect to MQ.
    let mq = RabbitMQ::new("amqp://guest:guest@localhost:5672", &exchange_name)
        .await
        .unwrap();
    let mut consumer = mq.consume(Some("delay_debug")).await;

    // Start delay middleware.
    let mut program = Command::cargo_bin("delay")
        .unwrap()
        .env("MIDDLEWARE_AMQP_URL", "amqp://guest:guest@localhost:5672")
        .env("MIDDLEWARE_AMQP_EXCHANGE", &exchange_name)
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
    assert!(
        timeout(Duration::from_secs(1), consumer.next())
            .await
            .is_err()
    );

    // Shutdown the middleware.
    program.kill().unwrap();
}

#[rstest]
#[case(true)]
#[case(false)]
#[tokio::test(flavor = "multi_thread")]
async fn must_reschedule(#[case] earlier_than_now: bool) {
    let exchange_name = format!("test_{}", rand::random::<usize>());

    // Initialize messages to send and expect.

    // The delivery time of the second request.
    let second_delay_at = if earlier_than_now {
        // This should be rejected
        SystemTime::now() - Duration::from_secs(5)
    } else {
        SystemTime::now() + Duration::from_secs(5)
    };
    let second_ts = second_delay_at
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    // The delivery time of the first request.
    let first_delay_at = SystemTime::now() + Duration::from_secs(2);
    let first_ts = first_delay_at.duration_since(UNIX_EPOCH).unwrap().as_secs();

    let first_request = Event::from_serializable_with_id(
        Uuid::nil(),
        "",
        Uuid::nil(),
        json!({
            "c": "d",
            "x-delay-id": 114_514,
            "x-delay-at": first_ts
        }),
    )
    .unwrap();
    let second_request = Event::from_serializable_with_id(
        Uuid::nil(),
        "",
        Uuid::nil(),
        json!({
            "a": "b",
            "x-delay-id": 114_514,
            "x-delay-at": second_ts
        }),
    )
    .unwrap();
    let expected = if earlier_than_now {
        Event::from_serializable_with_id(Uuid::nil(), "", Uuid::nil(), json!({"c": "d"})).unwrap()
    } else {
        Event::from_serializable_with_id(Uuid::nil(), "", Uuid::nil(), json!({"a": "b"})).unwrap()
    };

    // Connect to MQ.
    let mq = RabbitMQ::new("amqp://guest:guest@localhost:5672", &exchange_name)
        .await
        .unwrap();
    let mut consumer = mq.consume(Some("delay_reschedule_debug")).await;

    // Start delay middleware.
    let mut program = Command::cargo_bin("delay")
        .unwrap()
        .env("MIDDLEWARE_AMQP_URL", "amqp://guest:guest@localhost:5672")
        .env("MIDDLEWARE_AMQP_EXCHANGE", &exchange_name)
        .env("MIDDLEWARE_DATABASE_URL", ":memory:")
        .spawn()
        .unwrap();
    sleep(Duration::from_secs(1)).await;

    // Publish requests.
    mq.publish(
        first_request,
        "delay_reschedule_debug.delay".parse().unwrap(),
    )
    .await
    .unwrap();
    mq.publish(
        second_request,
        "delay_reschedule_debug.delay".parse().unwrap(),
    )
    .await
    .unwrap();

    // Receive the delayed message and check its content & deliver time.
    let expected_receive_time = if earlier_than_now {
        first_delay_at
    } else {
        second_delay_at
    };
    let msg = consumer.next().await.unwrap().unwrap();
    let received_time = SystemTime::now();
    assert_eq!(msg, (Middlewares::default(), expected));
    let delta = time_diff_abs(expected_receive_time, received_time);
    assert!(delta < Duration::from_millis(1500));

    // There must be only one message.
    assert!(
        timeout(Duration::from_secs(4), consumer.next())
            .await
            .is_err()
    );

    // Shutdown the middleware.
    program.kill().unwrap();
}

#[rstest]
#[case(json ! ({}))]
#[case(json ! ({"x-delay-at": 1_919_810}))]
#[case(json ! ({"matchy": "cute"}))]
#[tokio::test(flavor = "multi_thread")]
async fn must_cancel(#[case] mut event: Value) {
    let exchange_name = format!("test_{}", rand::random::<usize>());

    // Initialize messages to send and expect.
    let delay_at = SystemTime::now() + Duration::from_secs(5);
    let ts = delay_at.duration_since(UNIX_EPOCH).unwrap().as_secs();
    let original = Event::from_serializable(
        "",
        Uuid::nil(),
        json!({
            "a": "b",
            "x-delay-id": 114_514,
            "x-delay-at": ts
        }),
    )
    .unwrap();
    event["x-delay-id"] = json!(114_514);
    event["x-delay-cancel"] = json!(true);
    let cancel = Event::from_serializable("", Uuid::nil(), event).unwrap();

    // Connect to MQ.
    let mq = RabbitMQ::new("amqp://guest:guest@localhost:5672", &exchange_name)
        .await
        .unwrap();
    let mut consumer = mq.consume(Some("delay_cancel_debug")).await;

    // Start delay middleware.
    let mut program = Command::cargo_bin("delay")
        .unwrap()
        .env("MIDDLEWARE_AMQP_URL", "amqp://guest:guest@localhost:5672")
        .env("MIDDLEWARE_AMQP_EXCHANGE", &exchange_name)
        .env("MIDDLEWARE_DATABASE_URL", ":memory:")
        .spawn()
        .unwrap();
    sleep(Duration::from_secs(1)).await;

    // Publish a test message.
    mq.publish(original, "delay_cancel_debug.delay".parse().unwrap())
        .await
        .unwrap();
    // And then cancel it.
    mq.publish(cancel, "delay_cancel_debug.delay".parse().unwrap())
        .await
        .unwrap();

    // Should not receive any message.
    assert!(
        timeout(Duration::from_secs(6), consumer.next())
            .await
            .is_err()
    );

    // Shutdown the middleware.
    program.kill().unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn must_delay_and_send_across_restart() {
    let exchange_name = format!("test_{}", rand::random::<usize>());

    // Prepare temp file.
    let temp_file = tempfile::NamedTempFile::new().unwrap();
    let db_path = temp_file.path();

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
    let mq = RabbitMQ::new("amqp://guest:guest@localhost:5672", &exchange_name)
        .await
        .unwrap();
    let mut consumer = mq.consume(Some("delay_persist_debug")).await;

    // Start delay middleware.
    let mut program = Command::cargo_bin("delay")
        .unwrap()
        .env("MIDDLEWARE_AMQP_URL", "amqp://guest:guest@localhost:5672")
        .env("MIDDLEWARE_AMQP_EXCHANGE", &exchange_name)
        .env("MIDDLEWARE_DATABASE_URL", db_path)
        .env("RUST_LOG", "info")
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
        .env("MIDDLEWARE_AMQP_EXCHANGE", &exchange_name)
        .env("MIDDLEWARE_DATABASE_URL", db_path)
        .spawn()
        .unwrap();

    // Receive the delayed message and check its content & deliver time.
    let msg = consumer.next().await.unwrap().unwrap();
    let received_time = SystemTime::now();
    assert_eq!(msg, (Middlewares::default(), expected));
    let delta = time_diff_abs(delay_at, received_time);
    assert!(delta < Duration::from_millis(1500));

    // There must be only one message.
    assert!(
        timeout(Duration::from_millis(500), consumer.next())
            .await
            .is_err()
    );

    // Shutdown the middleware.
    program.kill().unwrap();
}

fn time_diff_abs(a: SystemTime, b: SystemTime) -> Duration {
    match a.duration_since(b) {
        Ok(delta) => delta,
        Err(e) => e.duration(),
    }
}
