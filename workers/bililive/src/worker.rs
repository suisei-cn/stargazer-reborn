use std::{collections::HashMap, sync::Arc, time::Duration};

use bililive::RetryConfig;
use eyre::{Result, WrapErr};
use futures_util::StreamExt;
use parking_lot::Mutex;
use serde::Deserialize;
use sg_core::{
    models::{Event, Task},
    mq::{MessageQueue, Middlewares},
    protocol::WorkerRpc,
    utils::ScopedJoinHandle,
};
use tap::TapOptional;
use tarpc::context::Context;
use tokio::time::sleep;
use tracing::{error, info, trace};
use uuid::Uuid;

use crate::bililive::LiveRoom;

#[derive(Clone)]
pub struct BililiveWorker {
    mq: Arc<dyn MessageQueue>,

    #[allow(clippy::type_complexity)]
    tasks: Arc<Mutex<HashMap<Uuid, (Task, ScopedJoinHandle<()>)>>>,
}

impl BililiveWorker {
    /// Creates a new worker.
    #[must_use]
    pub fn new(mq: impl MessageQueue + 'static) -> Self {
        Self {
            mq: Arc::new(mq),
            tasks: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

#[tarpc::server]
impl WorkerRpc for BililiveWorker {
    async fn ping(self, _: Context, id: u64) -> u64 {
        id
    }

    async fn add_task(self, _: Context, task: Task) -> bool {
        let mut tasks = self.tasks.lock();
        if tasks.contains_key(&task.id.into()) {
            // If the task is already running, do nothing.
            return false;
        }

        info!(task_id = ?task.id, "Adding task");

        // Extract uid from the task.
        let uid = match task.params.get("uid") {
            Some(v) if v.is_u64() => v.as_u64().unwrap(),
            Some(_) => {
                error!("UID field: type mismatch. Expected: u64");
                return false;
            }
            None => {
                error!("UID field: missing");
                return false;
            }
        };

        let fut = async move {
            loop {
                info!(?uid, "Spawning bililive task");
                if let Err(error) = bililive_task(uid, task.entity.into(), &*self.mq).await {
                    error!(?error, "Bililive task failed");

                    // Sleep to avoid looping if the task always fails.
                    sleep(Duration::from_secs(60)).await;
                }
            }
        };

        // Spawn the worker and insert it into the tasks map.
        tasks.insert(task.id.into(), (task, ScopedJoinHandle(tokio::spawn(fut))));

        true
    }

    async fn remove_task(self, _: Context, id: Uuid) -> bool {
        self.tasks
            .lock()
            .remove(&id)
            .tap_some(|_| info!(task_id=?id, "Removing task"))
            .is_some()
    }

    async fn tasks(self, _: Context) -> Vec<Task> {
        self.tasks
            .lock()
            .values()
            .map(|(task, _)| task)
            .cloned()
            .collect()
    }
}

#[derive(Debug, Eq, PartialEq, Deserialize)]
struct Command {
    cmd: String,
}

async fn bililive_task(uid: u64, entity_id: Uuid, mq: impl MessageQueue) -> Result<()> {
    let config = bililive::ConfigBuilder::new()
        .fetch_conf()
        .await
        .wrap_err("Unable to fetch bilibili server config")?
        .by_uid(uid)
        .await
        .wrap_err("Unable to fetch live room id by uid")?
        .build();
    let room_id = config.room_id();
    let mut stream = bililive::connect::tokio::connect_with_retry(config, RetryConfig::default())
        .await
        .wrap_err("Unable to connect to bilibili live server")?;

    while let Some(msg) = stream.next().await {
        match msg {
            Ok(msg) => {
                trace!(msg = ?msg, "Received message");
                if msg.json().ok()
                    == Some(Command {
                        cmd: String::from("LIVE"),
                    })
                {
                    info!(uid = uid, "Live started");

                    match LiveRoom::new(room_id).await {
                        Ok(room) => {
                            let event = Event::from_serializable("bililive", entity_id, room)?;
                            if let Err(error) = mq.publish(event, Middlewares::default()).await {
                                error!(?error, "Failed to publish bililive event");
                            };
                        }
                        Err(error) => {
                            error!(?error, "Unable to get live room");
                        }
                    }
                }
            }
            Err(err) => {
                error!(err = ?err, "Error receiving message");
            }
        }
    }

    Ok(())
}
