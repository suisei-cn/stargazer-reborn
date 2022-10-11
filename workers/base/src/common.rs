//! Common traits and types for workers.
use sg_core::models::Task;
use tokio_tungstenite::tungstenite::http::Uri;
use tracing::{debug, error};
use uuid::Uuid;

/// A worker that perform tasks.
pub trait Worker: Send {
    // TODO &self or &mut self?
    /// Add a task to the worker.
    fn add_task(&self, task: Task) -> bool;
    /// Remove a task from the worker.
    fn remove_task(&self, id: Uuid) -> bool;
}

/// An event represents a cluster member change or a task change.
pub enum Event {
    NodeUp(Uri),
    NodeDown(Uri),
    TaskAdd(Task),
    TaskRemove(Uuid),
}

/// A helper trait for logging.
pub trait WorkerLogExt {
    fn add_task_logged(&self, task: Task);
    fn remove_task_logged(&self, id: Uuid);
}

impl<W: Worker> WorkerLogExt for W {
    fn add_task_logged(&self, task: Task) {
        let task_id = task.id;
        if self.add_task(task) {
            debug!(%task_id, "Task added.");
        } else {
            error!(%task_id, "Task already exists.");
        }
    }

    fn remove_task_logged(&self, id: Uuid) {
        if self.remove_task(id) {
            debug!(task_id = %id, "Task removed.");
        } else {
            error!(task_id = %id, "Task does not exist.");
        }
    }
}
