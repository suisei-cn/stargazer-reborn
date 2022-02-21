//! Worker node and worker group.
use std::collections::HashMap;
use std::sync::{Arc, Weak};
use std::time::Duration;

use futures_util::{Sink, Stream};
use parking_lot::Mutex;
use tarpc::client::Config;
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::{Error as WsError, Message};
use tracing::{debug, error};
use uuid::Uuid;

use sg_core::adapter::WsTransport;
use sg_core::protocol::WorkerRpcClient;

/// Worker group for homogeneous workers.
#[derive(Debug, Default)]
pub struct WorkerGroup(Arc<Mutex<WorkerGroupImpl>>);

impl WorkerGroup {
    /// Create a new worker group.
    #[must_use]
    pub fn new() -> Self {
        Default::default()
    }
    /// Get a weak reference to the worker group.
    #[must_use]
    pub fn weak(&self) -> WeakWorkerGroup {
        WeakWorkerGroup(Arc::downgrade(&self.0))
    }
    /// Lock the worker group and mutate its state.
    pub fn with<O>(&self, f: impl FnOnce(&mut WorkerGroupImpl) -> O) -> O {
        let mut lock = self.0.lock();
        f(&mut *lock)
    }
}

/// Weak reference to a worker group.
#[derive(Debug)]
pub struct WeakWorkerGroup(Weak<Mutex<WorkerGroupImpl>>);

impl WeakWorkerGroup {
    /// Upgrade the weak reference to a strong reference.
    pub fn upgrade(&self) -> Option<WorkerGroup> {
        self.0.upgrade().map(WorkerGroup)
    }
}

#[derive(Debug, Default)]
/// Worker group implementation.
pub struct WorkerGroupImpl {
    workers: HashMap<Uuid, Arc<Worker>>,
}

impl WorkerGroupImpl {
    /// Add a new worker to the group.
    pub fn add_worker(&mut self, worker: Arc<Worker>) {
        debug!(worker_id = %worker.id, "Worker added to group");
        self.workers.insert(worker.id, worker);
        // TODO rebalance
    }
    /// Remove a worker to the group.
    pub fn remove_worker(&mut self, id: Uuid) {
        debug!(worker_id = %id, "Worker removed from group");
        self.workers.remove(&id);
        // TODO rebalance
    }
}

/// Task worker node.
#[derive(Debug)]
pub struct Worker {
    /// Worker ID.
    id: Uuid,
    /// Reference to the worker group.
    parent: WeakWorkerGroup,
    /// RPC client to the worker.
    client: WorkerRpcClient,
    /// Watchdog task.
    watchdog_job: JoinHandle<()>,
}

impl Worker {
    /// Create a new worker from given stream and worker group.
    pub fn new<S>(id: Uuid, stream: S, parent: WeakWorkerGroup) -> Arc<Self>
    where
        S: Stream<Item = Result<Message, WsError>>
            + Sink<Message, Error = WsError>
            + Unpin
            + Send
            + 'static,
    {
        Arc::new_cyclic(|this: &Weak<Self>| {
            let this = this.clone();
            let watchdog_job = tokio::spawn(async move {
                let mut check_interval = tokio::time::interval(Duration::from_secs(10));
                loop {
                    check_interval.tick().await;

                    if let Some(this) = this.upgrade() {
                        let tag = rand::random();
                        let resp = this.client.ping(tarpc::context::current(), tag).await;

                        if !matches!(resp, Ok(_tag)) {
                            // ping failed, remove node from worker group.
                            error!("Worker {}: ping failed", this.id);
                            this.remove_self();

                            break;
                        }
                    } else {
                        // self is dropped, so we can stop the watchdog.
                        break;
                    }
                }
            });

            Self {
                id,
                parent,
                client: WorkerRpcClient::new(Config::default(), WsTransport::new(stream)).spawn(),
                watchdog_job,
            }
        })
    }
    /// Remove self from worker group.
    pub fn remove_self(&self) {
        if let Some(parent) = self.parent.upgrade() {
            parent.with(|parent| parent.remove_worker(self.id));
        }
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.watchdog_job.abort();
    }
}
