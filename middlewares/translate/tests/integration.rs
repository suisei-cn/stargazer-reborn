use std::process::Command;
use std::time::Duration;

use assert_cmd::cargo::CommandCargoExt;
use futures_util::StreamExt;
use serde_json::json;
use tokio::time::{sleep, timeout};
use uuid::Uuid;

use sg_core::models::Event;
use sg_core::mq::{MessageQueue, Middlewares, RabbitMQ};

#[tokio::test(flavor = "multi_thread")]
async fn must_translate_and_put_back() {
    let exchange_name = format!("test_{}", rand::random::<usize>());

    let original = Event {
        id: Uuid::nil().into(),
        kind: "".to_string(),
        entity: Uuid::nil().into(),
        fields: json!({
            "a": "a",
            "b": ["b1", "b2"],
            "c": {
                "cc": "d"
            },
            "x-translate-fields": ["/a", "/b/0", "/c/cc"]
        })
        .as_object()
        .unwrap()
        .clone(),
    };
    let translated = Event {
        id: Uuid::nil().into(),
        kind: "".to_string(),
        entity: Uuid::nil().into(),
        fields: json!({
            "a": "testa",
            "b": ["testb1", "b2"],
            "c": {
                "cc": "testd"
            }
        })
        .as_object()
        .unwrap()
        .clone(),
    };

    let mut program = Command::cargo_bin("translate")
        .unwrap()
        .env("MIDDLEWARE_AMQP_URL", "amqp://guest:guest@localhost:5672")
        .env("MIDDLEWARE_AMQP_EXCHANGE", &exchange_name)
        .env("MIDDLEWARE_BAIDU_APP_ID", "0")
        .env("MIDDLEWARE_BAIDU_APP_SECRET", "")
        .env("MIDDLEWARE_DEBUG", "true")
        .spawn()
        .unwrap();

    let mq = RabbitMQ::new("amqp://guest:guest@localhost:5672", &exchange_name)
        .await
        .unwrap();
    let mut consumer = mq.consume(Some("translate_debug")).await;

    sleep(Duration::from_secs(1)).await;

    mq.publish(original, "translate_debug.translate".parse().unwrap())
        .await
        .unwrap();

    let msg = consumer.next().await.unwrap().unwrap();
    assert_eq!(msg, (Middlewares::default(), translated));

    // There's only one message.
    assert!(timeout(Duration::from_millis(500), consumer.next())
        .await
        .is_err());

    program.kill().unwrap();
}
