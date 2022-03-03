//! RPC protocol.

use std::fmt::Display;
use std::future::Future;
use std::pin::Pin;

use eyre::Result;
use tarpc::server::{BaseChannel, Channel, Serve};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tracing::{debug, info};
use uuid::Uuid;

use crate::adapter::WsTransport;
use crate::models::Task;

/// RPC protocol for worker-coordinator communication.
#[tarpc::service]
pub trait WorkerRpc {
    /// Ping the worker.
    async fn ping(id: u64) -> u64;
    /// Add a task to the worker. Return `false` if the task already exists.
    async fn add_task(task: Task) -> bool;
    /// Remove a task from the worker. Return `false` if the task was not found.
    async fn remove_task(id: Uuid) -> bool;
    /// Get the list of tasks running on the worker.
    async fn tasks() -> Vec<Task>;
}

/// Extension trait for `WorkerRpc`.
pub trait WorkerRpcExt {
    /// Join a coordinator.
    fn join(
        self,
        addr: impl IntoClientRequest + Unpin + Send + 'static,
        id: Uuid,
        ty: impl Display + Send + 'static,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send>>;
}

impl<T> WorkerRpcExt for T
where
    T: WorkerRpc + Clone + Send,
    ServeWorkerRpc<T>: Serve<WorkerRpcRequest, Resp = WorkerRpcResponse, Fut = WorkerRpcResponseFut<Self>>
        + Send
        + 'static,
    WorkerRpcResponseFut<Self>: Send + 'static,
{
    fn join(
        self,
        addr: impl IntoClientRequest + Unpin + Send + 'static,
        id: Uuid,
        ty: impl Display + Send + 'static,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send>> {
        Box::pin(async move {
            let mut req = addr.into_client_request()?;

            req.headers_mut()
                .insert("Sg-Worker-Kind", ty.to_string().parse()?);
            req.headers_mut()
                .insert("Sg-Worker-ID", id.to_string().parse()?);

            debug!("Connecting to coordinator");
            let (stream, _) = tokio_tungstenite::connect_async(req).await?;
            let channel = BaseChannel::with_defaults(WsTransport::new(stream));

            info!("Coordinator connected, ready to receive tasks.");
            channel.execute(self.serve()).await;
            Ok(())
        })
    }
}
