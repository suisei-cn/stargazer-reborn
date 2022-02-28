use std::collections::{HashMap, HashSet};
use std::fmt::Display;
use std::net::UdpSocket;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use educe::Educe;
use eyre::Result;
use mongodb::bson::doc;
use mongodb::{Client, Collection};
use tarpc::context::Context;
use tokio::sync::oneshot::{channel, Sender};
use tokio::task::JoinHandle;
use tokio::time::sleep;
use uuid::Uuid;

use sg_core::models::Task;
use sg_core::protocol::{WorkerRpc, WorkerRpcExt};
use sg_core::utils::ScopedJoinHandle;
use sg_core::value::Value;

use crate::config::Config;
use crate::db::DB;
use crate::App;

#[derive(Clone, Educe)]
#[educe(Hash, Eq, PartialEq)]
struct DummyWorker {
    #[educe(Hash(ignore), Eq(ignore), PartialEq(ignore))]
    ws: String,
    id: Uuid,
    #[educe(Hash(ignore), Eq(ignore), PartialEq(ignore))]
    kind: String,
    #[educe(Hash(ignore), Eq(ignore), PartialEq(ignore))]
    tasks: Arc<Mutex<HashMap<Uuid, Task>>>,
}

impl DummyWorker {
    pub fn new(ws: impl Display, kind: impl Display) -> Self {
        Self {
            ws: ws.to_string(),
            id: Uuid::new_v4(),
            kind: kind.to_string(),
            tasks: Default::default(),
        }
    }
    pub async fn join_remote(self) -> Result<()> {
        self.clone().join(self.ws, self.id, self.kind).await
    }
}

#[tarpc::server]
impl WorkerRpc for DummyWorker {
    async fn ping(self, _: Context, id: u64) -> u64 {
        id
    }
    async fn add_task(self, _: Context, task: Task) -> bool {
        self.tasks
            .lock()
            .unwrap()
            .insert(task.id.into(), task)
            .is_none()
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

struct Tester {
    server: App,
    server_stop: Sender<()>,
    server_handle: JoinHandle<Result<()>>,
    port: u16,

    tasks: HashMap<String, HashSet<Uuid>>,
    clients: HashMap<String, HashMap<DummyWorker, ScopedJoinHandle<()>>>,
}

impl Tester {
    pub async fn new() -> Self {
        let port = free_port();
        let server = App::new(Config {
            bind: format!("127.0.0.1:{}", port).parse().unwrap(),
            ping_interval: Duration::from_millis(100),
            ..Default::default()
        });
        let (tx, rx) = channel();
        let server_handle = {
            let server = server.clone();
            tokio::spawn(async move {
                tokio::select! {
                    r = server.serve() => r,
                    _ = rx => Ok(())
                }
            })
        };
        sleep(Duration::from_millis(100)).await;

        Self {
            server,
            server_stop: tx,
            server_handle,
            port,
            tasks: Default::default(),
            clients: Default::default(),
        }
    }

    pub async fn finish(self) {
        self.server_stop.send(()).unwrap();
        self.server_handle.await.unwrap().unwrap();
    }

    async fn validate(&self) {
        let mut server_side: HashMap<String, HashMap<Uuid, Uuid>> = HashMap::new();
        let mut remote_tasks: HashMap<String, HashSet<Uuid>> = HashMap::new();
        for (kind, workers) in &*self.server.worker_groups.lock().await {
            workers
                .with(|workers| {
                    for (id, bound_task) in &workers.tasks {
                        remote_tasks.entry(kind.clone()).or_default().insert(*id);
                        if let Some(bound_worker) = bound_task.worker {
                            server_side
                                .entry(kind.clone())
                                .or_default()
                                .insert(*id, bound_worker);
                        }
                    }
                })
                .await;
        }

        assert_eq!(
            self.tasks, remote_tasks,
            "Server and local tasks do not match"
        );

        let mut client_side: HashMap<String, HashMap<Uuid, Uuid>> = HashMap::new();
        for (kind, workers) in &self.clients {
            for worker in workers.keys() {
                for task in worker.tasks.lock().unwrap().values() {
                    client_side
                        .entry(kind.clone())
                        .or_default()
                        .insert(task.id.into(), worker.id);
                }
            }
        }

        assert_eq!(
            server_side, client_side,
            "Server and client task distribution don't match"
        );
    }

    pub async fn increase_workers(&mut self, kind: impl Display + Send, count: usize) {
        let kind = kind.to_string();
        eprintln!("Increase {} {} workers", count, kind);

        for _ in 0..count {
            let ws = format!("ws://127.0.0.1:{}", self.port);
            let worker = DummyWorker::new(ws, kind.clone());

            let handle = {
                let worker = worker.clone();
                ScopedJoinHandle(tokio::spawn(async move {
                    worker.join_remote().await.unwrap();
                }))
            };
            self.clients
                .entry(kind.clone())
                .or_default()
                .insert(worker, handle);
        }

        sleep(Duration::from_millis(150)).await;
        self.validate().await;
    }

    pub async fn decrease_workers(&mut self, kind: impl Display + Send, count: usize) {
        let kind = kind.to_string();
        eprintln!("Decrease {} {} workers", count, kind);

        for _ in 0..count {
            if let Some(map) = self.clients.get_mut(&kind) {
                if let Some((client, handle)) = map
                    .iter()
                    .map(|(client, handle)| (client.clone(), handle))
                    .next()
                {
                    handle.abort();
                    map.remove(&client);
                }
            }
        }

        sleep(Duration::from_millis(150)).await;
        self.validate().await;
    }

    pub async fn increase_tasks(&mut self, kind: impl Display + Send, count: usize) {
        let kind = kind.to_string();
        eprintln!("Increase {} {} tasks", count, kind);

        for _ in 0..count {
            let task = Task {
                id: Uuid::new_v4().into(),
                entity: Uuid::new_v4().into(),
                kind: kind.clone(),
                params: Default::default(),
            };

            self.tasks
                .entry(kind.clone())
                .or_default()
                .insert(task.id.into());
            self.server.add_task(task).await;
        }

        sleep(Duration::from_millis(250)).await;
        self.validate().await;
    }

    pub async fn decrease_tasks(&mut self, kind: impl Display + Send, count: usize) {
        let kind = kind.to_string();
        eprintln!("Decrease {} {} tasks", count, kind);

        for _ in 0..count {
            if let Some(tasks) = self.tasks.get_mut(&kind) {
                if let Some(id) = tasks.iter().copied().next() {
                    tasks.remove(&id);
                    self.server.remove_task(id).await;
                }
            }
        }

        sleep(Duration::from_millis(150)).await;
        self.validate().await;
    }
}

#[tokio::test]
async fn must_consistent() {
    let mut tester = Tester::new().await;

    tester.increase_tasks("test", 100).await;
    tester.increase_workers("test", 5).await;
    tester.decrease_tasks("test", 20).await;
    tester.increase_workers("test", 5).await;
    tester.increase_tasks("test", 50).await;
    tester.increase_workers("test", 10).await;
    tester.increase_tasks("test", 50).await;
    tester.decrease_workers("test", 7).await;
    tester.decrease_tasks("test", 20).await;
    tester.increase_tasks("test", 50).await;

    tester.finish().await;
}

#[tokio::test]
async fn must_db() {
    let client = Client::with_uri_str("mongodb://localhost:27017/")
        .await
        .unwrap();
    let db = client.database("test");
    let collection: Collection<Task> = db.collection("coordinator");
    let config = Config {
        mongo_uri: String::from("mongodb://localhost:27017/"),
        mongo_db: String::from("test"),
        mongo_collection: String::from("coordinator"),
        ..Default::default()
    };

    // Clear test collection before test.
    collection.drop(None).await.unwrap();

    // Add some initial tasks.
    let mut tasks: Vec<_> = (0..5)
        .into_iter()
        .map(|_| Task {
            id: Uuid::new_v4().into(),
            entity: Uuid::new_v4().into(),
            kind: String::from("test"),
            params: Default::default(),
        })
        .collect();
    collection.insert_many(&tasks, None).await.unwrap();

    // Create app and db instance.
    let app = App::new(config.clone());
    let mut db = DB::new(app.clone(), config).await.unwrap();

    // Initial tasks must be added.
    db.init_tasks().await.unwrap();
    assert_task_ids(&app, &tasks).await;

    // Spawn change stream task.
    tokio::spawn(async move {
        db.watch_tasks().await.unwrap();
    });

    let new_task = Task {
        id: Uuid::new_v4().into(),
        entity: Uuid::new_v4().into(),
        kind: String::from("test"),
        params: Default::default(),
    };

    // Insert a new task.
    tasks.push(new_task.clone());
    collection.insert_one(new_task, None).await.unwrap();
    sleep(Duration::from_millis(200)).await;
    assert_task_ids(&app, &tasks).await;

    // Update a task.
    let mut task = tasks.pop().unwrap();
    task.params
        .insert("test".into(), Value::String("test".into()));
    tasks.push(task.clone());
    collection
        .update_one(
            doc! { "id": task.id },
            doc! { "$set": { "params": { "test": "test" } } },
            None,
        )
        .await
        .unwrap();
    sleep(Duration::from_millis(200)).await;
    assert_task_ids(&app, &tasks).await;

    // Replace a task.
    let mut task = tasks.pop().unwrap();
    task.params.clear();
    tasks.push(task.clone());
    collection
        .replace_one(doc! { "id": task.id }, task, None)
        .await
        .unwrap();
    sleep(Duration::from_millis(200)).await;
    assert_task_ids(&app, &tasks).await;

    // Delete a task.
    let task = tasks.pop().unwrap();
    collection
        .delete_one(doc! { "id": task.id }, None)
        .await
        .unwrap();
    sleep(Duration::from_millis(200)).await;
    assert_task_ids(&app, &tasks).await;
}

async fn assert_task_ids(app: &App, expected: &[Task]) {
    app.worker_groups.lock().await["test"]
        .with(|group| {
            let group_task_ids: HashSet<_> = group.tasks.keys().copied().collect();
            let expected_task_ids: HashSet<_> =
                expected.iter().map(|task| task.id.into()).collect();
            assert_eq!(group_task_ids, expected_task_ids);
        })
        .await;
}
