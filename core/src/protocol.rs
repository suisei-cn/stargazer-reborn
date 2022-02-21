//! RPC protocol.

use std::fmt::Display;
use std::future::Future;
use std::pin::Pin;

use eyre::Result;
use tarpc::server::{BaseChannel, Channel, Serve};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use uuid::Uuid;

use crate::adapter::WsTransport;

/// RPC protocol for worker-coordinator communication.
#[tarpc::service]
pub trait WorkerRpc {
    /// Ping the worker.
    async fn ping(id: u64) -> u64;
}

/// Extension trait for `WorkerRpc`.
pub trait WorkerRpcExt {
    /// Join a coordinator.
    fn join(
        self,
        addr: impl IntoClientRequest + Unpin + 'static,
        id: Uuid,
        ty: impl Display + 'static,
    ) -> Pin<Box<dyn Future<Output = Result<()>>>>;
}

impl<T> WorkerRpcExt for T
where
    T: WorkerRpc + Clone,
    ServeWorkerRpc<T>: Serve<WorkerRpcRequest, Resp = WorkerRpcResponse, Fut = WorkerRpcResponseFut<Self>>
        + Send
        + 'static,
    WorkerRpcResponseFut<Self>: Send + 'static,
{
    fn join(
        self,
        addr: impl IntoClientRequest + Unpin + 'static,
        id: Uuid,
        ty: impl Display + 'static,
    ) -> Pin<Box<dyn Future<Output = Result<()>>>> {
        Box::pin(async move {
            let mut req = addr.into_client_request()?;

            req.headers_mut()
                .insert("sg-worker-ty", ty.to_string().parse()?);
            req.headers_mut()
                .insert("sg-worker-id", id.to_string().parse()?);

            let (stream, _) = tokio_tungstenite::connect_async(req).await?;
            let channel = BaseChannel::with_defaults(WsTransport::new(stream));
            channel.execute(self.serve()).await;
            Ok(())
        })
    }
}
