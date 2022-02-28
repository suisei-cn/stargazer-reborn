//! Worker node and worker group.
use std::collections::{HashMap, HashSet};
use std::fmt::{Debug, Formatter};
use std::sync::{Arc, Weak};

use consistent_hash_ring::Ring;
use futures_util::{Sink, Stream};
use tap::TapFallible;
use tarpc::client::{Config as ClientConfig, RpcError};
use tarpc::context::Context;
use tokio::sync::Mutex;
use tokio::sync::Notify;
use tokio_tungstenite::tungstenite::{Error as WsError, Message};
use tracing::{debug, error, warn};
use uuid::Uuid;

use sg_core::adapter::WsTransport;
use sg_core::models::Task;
use sg_core::protocol::WorkerRpcClient;

use crate::config::Config;
use crate::utils::ScopedJoinHandle;

/// Worker group for homogeneous workers.
#[derive(Debug)]
pub struct WorkerGroup {
    inner: Arc<Mutex<WorkerGroupImpl>>,
    balance_job: Arc<ScopedJoinHandle<()>>,
}

impl Default for WorkerGroup {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkerGroup {
    /// Create a new worker group.
    #[must_use]
    pub fn new() -> Self {
        let balance_notify = Arc::new(Notify::new());
        let inner = Arc::new(Mutex::new(WorkerGroupImpl::new(balance_notify.clone())));

        let task = {
            let inner = inner.clone();
            async move {
                loop {
                    balance_notify.notified().await;

                    if !inner.lock().await.balance().await {
                        // Balance failed, schedule a balance immediately.
                        balance_notify.notify_one();
                    }
                }
            }
        };
        let balance_job = Arc::new(ScopedJoinHandle(tokio::spawn(task)));

        Self { inner, balance_job }
    }
    /// Get a weak reference to the worker group.
    #[must_use]
    pub fn weak(&self) -> WeakWorkerGroup {
        WeakWorkerGroup {
            inner: Arc::downgrade(&self.inner),
            balance_job: Arc::downgrade(&self.balance_job),
        }
    }
    /// Lock the worker group and mutate its state.
    pub async fn with<O>(&self, f: impl FnOnce(&mut WorkerGroupImpl) -> O + Send) -> O {
        let mut lock = self.inner.lock().await;
        let output = f(&mut *lock);
        drop(lock);
        output
    }
}

/// Weak reference to a worker group.
#[derive(Debug)]
pub struct WeakWorkerGroup {
    inner: Weak<Mutex<WorkerGroupImpl>>,
    balance_job: Weak<ScopedJoinHandle<()>>,
}

impl WeakWorkerGroup {
    /// Upgrade the weak reference to a strong reference.
    #[must_use]
    pub fn upgrade(&self) -> Option<WorkerGroup> {
        Some(WorkerGroup {
            inner: self.inner.upgrade()?,
            balance_job: self.balance_job.upgrade()?,
        })
    }
}

#[derive(Debug)]
pub(crate) struct BoundTask {
    /// Task struct.
    task: Task,
    /// The worker that is currently executing the task.
    pub(crate) worker: Option<Uuid>,
}

/// Worker group implementation.
pub struct WorkerGroupImpl {
    pub(crate) workers: HashMap<Uuid, Arc<Worker>>,
    pub(crate) tasks: HashMap<Uuid, BoundTask>,
    ring: Ring</* worker */ Uuid>,
    balance_notify: Arc<Notify>,
}

impl Debug for WorkerGroupImpl {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let ring_debug: Vec<_> = self
            .ring
            .resident_ranges()
            .map(|resident| resident.node())
            .collect();

        f.debug_struct("WorkerGroupImpl")
            .field("workers", &self.workers)
            .field("tasks", &self.tasks)
            .field("ring", &ring_debug)
            .finish()
    }
}

fn check_resp(
    resp: Result<bool, RpcError>,
    task_id: Uuid,
    worker_id: Uuid,
    false_msg: &str,
    err_msg: &str,
) -> Result<(), Uuid> {
    match resp {
        Ok(true) => Ok(()),
        Ok(false) => {
            error!(%task_id, %worker_id, false_msg);
            Err(worker_id)
        }
        Err(e) => {
            error!(%task_id, %worker_id, "{}: {}", err_msg, e);
            Err(worker_id)
        }
    }
}

impl WorkerGroupImpl {
    /// Create a new worker group implementation.
    #[must_use]
    pub fn new(balance_notify: Arc<Notify>) -> Self {
        Self {
            workers: HashMap::new(),
            tasks: HashMap::new(),
            ring: Ring::default(),
            balance_notify,
        }
    }
    /// Add a new worker to the group.
    pub fn add_worker(&mut self, worker: Arc<Worker>) {
        debug!(worker_id = %worker.id, "Add worker to group");
        self.ring.insert(worker.id);
        self.workers.insert(worker.id, worker);

        self.balance_notify.notify_one();
    }
    /// Remove a worker from the group.
    pub fn remove_worker(&mut self, id: Uuid) {
        debug!(worker_id = %id, "Remove worker from group");
        self.ring.remove(&id);
        self.workers.remove(&id);

        self.balance_notify.notify_one();
    }
    /// Add a task to the group.
    pub fn add_task(&mut self, task: Task) {
        let id = task.id;
        debug!(task_id = %id, "Add task to group");
        let bound_task = BoundTask { task, worker: None };
        self.tasks.insert(id.into(), bound_task);

        self.balance_notify.notify_one();
    }
    /// Remove a task from the group.
    pub fn remove_task(&mut self, id: Uuid) {
        debug!(task_id = %id, "Remove task from group");
        self.tasks.remove(&id);

        self.balance_notify.notify_one();
    }

    /// Balance the group.
    ///
    /// Workers not responding or inconsistent will be removed. Return `false` if there's a worker removed.
    /// Balance should be called again in this case.
    pub async fn balance(&mut self) -> bool {
        self.balance_impl()
            .await
            .tap_err(|bad_worker| {
                warn!(worker_id=%bad_worker, "Balance: remove bad worker");
                self.remove_worker(*bad_worker);
            })
            .is_ok()
    }

    /// Core implementation to balance the group.
    ///
    /// # Errors
    /// If a worker is not responding or inconsistent, return id of that worker.
    ///
    /// Beware that if an error is returned, the tasks field of the worker is poisoned.
    async fn balance_impl(&mut self) -> Result<(), Uuid> {
        // TODO instrument this future

        if self.ring.is_empty() {
            error!("Balance: No worker in worker group");
            return Ok(());
        }

        // Remove gone tasks.
        for worker in self.workers.values_mut() {
            // Note that we collect tasks_gone first to avoid holding the lock across awaits.

            // Do RPC to remove tasks from remote worker.
            let tasks_gone: Vec<_> = worker
                .tasks
                .lock()
                .await
                .iter()
                .filter(|task| !self.tasks.contains_key(task))
                .copied()
                .collect();
            for task in tasks_gone {
                // This task is gone, we remove it from the worker.
                debug!(task_id=%task, worker_id=%worker.id, "Task is gone, remove from worker");
                let resp = worker.client.remove_task(Context::current(), task).await;
                check_resp(
                    resp,
                    task,
                    worker.id,
                    "Task not found on worker",
                    "Error removing task from worker",
                )?;
            }

            // Remove tasks from local map.
            worker
                .tasks
                .lock()
                .await
                .retain(|task| self.tasks.contains_key(task));
        }

        // Migrate tasks to new workers.
        for (task_id, bound_task) in &mut self.tasks {
            // Calculate expected worker using the ring.
            let expected_worker_id = self.ring.get(&task_id);
            // Currently assigned worker.
            let bound_worker_id = &mut bound_task.worker;

            debug!(%task_id, worker_id=%expected_worker_id, "Migrating task");

            if *bound_worker_id != Some(*expected_worker_id) {
                // If task is not assigned to the expected worker ...

                // If the task has already assigned to a worker, remove it.
                if let Some(old_worker) = bound_worker_id.and_then(|id| self.workers.get_mut(&id)) {
                    // Do RPC to remove tasks from remote worker.
                    let resp = old_worker
                        .client
                        .remove_task(Context::current(), *task_id)
                        .await;
                    check_resp(
                        resp,
                        *task_id,
                        old_worker.id,
                        "Task not found on worker",
                        "Error removing task from worker",
                    )?;

                    // Remove tasks from local map.
                    old_worker.tasks.lock().await.remove(task_id);
                }

                // Assign the task to the expected worker.
                let expected_worker = self
                    .workers
                    .get_mut(expected_worker_id)
                    .expect("Migration target worker must exist");
                // Do RPC to add tasks to remote worker.
                let resp = expected_worker
                    .client
                    .add_task(Context::current(), bound_task.task.clone())
                    .await;
                check_resp(
                    resp,
                    *task_id,
                    *expected_worker_id,
                    "Task already exists on worker",
                    "Error adding task to worker",
                )?;

                // Add tasks to local map.
                expected_worker.tasks.lock().await.insert(*task_id);

                // Update the task's bound info.
                *bound_worker_id = Some(*expected_worker_id);
            }
        }

        if cfg!(debug_assertions) {
            self.validate().await;
        }

        Ok(())
    }

    /// Validate if the internal state of the group is consistent.
    ///
    /// This method is quite expensive due to locking, and should be used only for debugging.
    ///
    /// # Panics
    /// Panics if the group is not consistent.
    pub async fn validate(&self) {
        // Task must only be assigned to one worker.
        let mut tasks = HashSet::new();
        for worker in self.workers.values() {
            for task in &*worker.tasks.lock().await {
                assert!(tasks.insert(*task), "multiple task {} present", task);
            }
        }

        // Worker-task and task-worker map must have the same tasks.
        assert_eq!(
            tasks,
            self.tasks.keys().copied().collect(),
            "tasks are not synchronized between worker-task and task-worker maps"
        );

        // Task can't be assigned to unknown workers.
        let workers: HashSet<_> = self.workers.keys().copied().collect();
        let assigned_to: HashSet<_> = self.tasks.values().filter_map(|task| task.worker).collect();
        let unknown_workers = &assigned_to - &workers;
        assert!(
            unknown_workers.is_empty(),
            "task assigned to unknown workers: {:?}",
            unknown_workers
        );

        // Ring must have the same workers as the workers map.
        let ring_nodes: HashSet<_> = self
            .ring
            .resident_ranges()
            .map(|resident| resident.node())
            .copied()
            .collect();
        assert_eq!(
            ring_nodes, workers,
            "ring nodes are not the same as workers"
        );
    }

    /// Returns the number of workers in the worker group.
    #[allow(clippy::must_use_candidate)]
    pub fn worker_len(&self) -> usize {
        self.workers.len()
    }
    /// Returns `true` if the group contains no workers.
    #[allow(clippy::must_use_candidate)]
    pub fn worker_is_empty(&self) -> bool {
        self.workers.is_empty()
    }
    /// Returns the number of tasks in the worker group.
    #[allow(clippy::must_use_candidate)]
    pub fn task_len(&self) -> usize {
        self.tasks.len()
    }
    /// Returns `true` if the group contains no tasks.
    #[allow(clippy::must_use_candidate)]
    pub fn task_is_empty(&self) -> bool {
        self.tasks.is_empty()
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
    #[allow(dead_code)]
    watchdog_job: ScopedJoinHandle<()>,
    /// Tasks assigned to the worker.
    tasks: Mutex<HashSet<Uuid>>,
}

impl Worker {
    /// Create a new worker from given stream and worker group.
    pub fn new<S>(id: Uuid, stream: S, parent: WeakWorkerGroup, config: &Config) -> Arc<Self>
    where
        S: Stream<Item = Result<Message, WsError>>
            + Sink<Message, Error = WsError>
            + Unpin
            + Send
            + 'static,
    {
        Arc::new_cyclic(|this: &Weak<Self>| {
            let this = this.clone();
            let ping_interval = config.ping_interval;
            let watchdog_job = tokio::spawn(async move {
                let mut check_interval = tokio::time::interval(ping_interval);
                loop {
                    check_interval.tick().await;

                    if let Some(this) = this.upgrade() {
                        let tag = rand::random();
                        let resp = this.client.ping(tarpc::context::current(), tag).await;

                        if !matches!(resp, Ok(_tag)) {
                            // ping failed, remove node from worker group.
                            error!(worker_id = %this.id, "Ping failed");
                            this.remove_self().await;

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
                client: WorkerRpcClient::new(ClientConfig::default(), WsTransport::new(stream))
                    .spawn(),
                watchdog_job: ScopedJoinHandle(watchdog_job),
                tasks: Default::default(),
            }
        })
    }
    /// Remove self from worker group.
    pub async fn remove_self(&self) {
        if let Some(parent) = self.parent.upgrade() {
            parent.with(|parent| parent.remove_worker(self.id)).await;
        }
    }
}
