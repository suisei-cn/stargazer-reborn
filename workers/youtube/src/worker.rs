use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::{Mutex, RwLock};
use serde_json::Value;
use tap::Tap;
use tarpc::context::Context;
use tracing::{error, info};
use uuid::Uuid;

use sg_core::models::Task;
use sg_core::protocol::WorkerRpc;

use crate::registry::Registry;
use crate::Config;

#[derive(Clone)]
pub struct YoutubeWorker {
    config: Config,
    registry: Arc<RwLock<Registry>>,
    tasks: Arc<Mutex<HashMap<Uuid, Task>>>,
}

impl YoutubeWorker {
    pub fn new(config: Config, registry: Arc<RwLock<Registry>>) -> Self {
        Self {
            config,
            registry,
            tasks: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

#[tarpc::server]
impl WorkerRpc for YoutubeWorker {
    async fn ping(self, _: Context, id: u64) -> u64 {
        id
    }

    async fn add_task(self, _: Context, task: Task) -> bool {
        let mut registry = self.registry.write();
        if registry.contains_id(task.id.into()) {
            // If the task is already running, do nothing.
            return false;
        }

        info!(task_id = ?task.id, "Adding task");

        // Extract the channel id from the task.
        let channel_id = match task.params.get("channel_id") {
            Some(Value::String(channel_id)) => channel_id.clone(),
            Some(_) => {
                error!("channel_id field: type mismatch. Expected: String");
                return false;
            }
            None => {
                error!("channel_id field: missing");
                return false;
            }
        };

        registry
            .add_channel(task.id.into(), channel_id)
            .tap(|succ| {
                if *succ {
                    self.tasks.lock().insert(task.id.into(), task);
                }
            })
    }

    async fn remove_task(self, _: Context, id: Uuid) -> bool {
        self.registry.write().remove_channel(id).tap(|succ| {
            if *succ {
                self.tasks.lock().remove(&id);
            }
        })
    }

    async fn tasks(self, _: Context) -> Vec<Task> {
        self.tasks.lock().values().cloned().collect()
    }
}
