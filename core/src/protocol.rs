//! RPC protocol.

/// RPC protocol for worker-coordinator communication.
#[tarpc::service]
pub trait WorkerRpc {
    /// Ping the worker.
    async fn ping(id: u64) -> u64;
}
