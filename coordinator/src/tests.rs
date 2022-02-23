use std::collections::HashMap;
use std::net::UdpSocket;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tarpc::context::Context;
use tokio::time::{sleep, timeout};
use uuid::Uuid;

use sg_core::models::Task;
use sg_core::protocol::{WorkerRpc, WorkerRpcExt};

use crate::config::Config;
use crate::App;

#[derive(Clone, Default)]
struct DummyWorker {
    tasks: Arc<Mutex<HashMap<Uuid, Task>>>,
}

#[tarpc::server]
impl WorkerRpc for DummyWorker {
    async fn ping(self, _: Context, id: u64) -> u64 {
        id
    }
    async fn add_task(self, _: Context, task: Task) -> bool {
        self.tasks.lock().unwrap().insert(task.id, task).is_none()
    }
    async fn remove_task(self, _: Context, id: Uuid) -> bool {
        self.tasks.lock().unwrap().remove(&id).is_some()
    }
    async fn tasks(self, _: Context) -> Vec<Task> {
        self.tasks.lock().unwrap().values().cloned().collect()
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
            app.worker_groups.lock().await["dummy"]
                .with(|this| this.worker_len())
                .await,
            1
        );

        sleep(Duration::from_millis(200)).await;
        assert_eq!(
            app.worker_groups.lock().await["dummy"]
                .with(|this| this.worker_len())
                .await,
            0
        );

        handle.abort();
    });

    sleep(Duration::from_millis(100)).await;
    assert!(timeout(
        Duration::from_millis(200),
        DummyWorker::default().join(format!("ws://127.0.0.1:{}", port), Uuid::new_v4(), "dummy"),
    )
    .await
    .is_err());

    server.await.unwrap();
}
