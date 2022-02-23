use eyre::Result;
use tarpc::context::Context;
use tracing::level_filters::LevelFilter;
use tracing::warn;
use uuid::Uuid;

use sg_core::models::Task;
use sg_core::protocol::{WorkerRpc, WorkerRpcExt};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(LevelFilter::WARN)
        .init();
    Worker
        .join("ws://127.0.0.1:7000", Uuid::new_v4(), "dummy")
        .await?;
    Ok(())
}

#[derive(Debug, Clone)]
struct Worker;

#[tarpc::server]
impl WorkerRpc for Worker {
    async fn ping(self, _: Context, id: u64) -> u64 {
        id
    }
    async fn add_task(self, _: Context, task: Task) -> bool {
        warn!("adding task {}", task.id);
        true
    }
    async fn remove_task(self, _: Context, id: Uuid) -> bool {
        warn!("removing task {}", id);
        true
    }
    async fn tasks(self, _: Context) -> Vec<Task> {
        Vec::new()
    }
}
