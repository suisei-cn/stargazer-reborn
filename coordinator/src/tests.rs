use std::net::UdpSocket;
use std::time::Duration;

use tarpc::context::Context;
use tokio::time::{sleep, timeout};
use uuid::Uuid;

use sg_core::protocol::{WorkerRpc, WorkerRpcExt};

use crate::config::Config;
use crate::App;

#[derive(Clone)]
struct DummyWorker;

#[tarpc::server]
impl WorkerRpc for DummyWorker {
    async fn ping(self, _: Context, id: u64) -> u64 {
        id
    }
}

fn free_port() -> u16 {
    let sock = UdpSocket::bind("127.0.0.1:0").unwrap();
    sock.local_addr().unwrap().port()
}

#[tokio::test]
async fn must_join_disconnect() {
    let port = free_port();

    let server = tokio::spawn(async move {
        let app = App::new(Config {
            ping_interval: Duration::from_millis(100),
        });
        let app_2 = app.clone();
        let handle = tokio::spawn(async move {
            app_2
                .serve(format!("127.0.0.1:{}", port).parse().unwrap())
                .await
        });

        sleep(Duration::from_millis(200)).await;
        assert_eq!(
            app.inner().worker_groups.lock()["dummy"].with(|this| this.len()),
            1
        );

        sleep(Duration::from_millis(200)).await;
        assert_eq!(
            app.inner().worker_groups.lock()["dummy"].with(|this| this.len()),
            0
        );

        handle.abort();
    });

    sleep(Duration::from_millis(100)).await;
    assert!(timeout(
        Duration::from_millis(200),
        DummyWorker.join(format!("ws://127.0.0.1:{}", port), Uuid::new_v4(), "dummy"),
    )
    .await
    .is_err());

    server.await.unwrap();
}
