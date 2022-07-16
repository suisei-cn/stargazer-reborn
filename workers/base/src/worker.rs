//! Worker trait and manager logic.

use consistent_hash_ring::Ring;
use tokio::net::{TcpListener, ToSocketAddrs};
use uuid::Uuid;
use eyre::Result;
use tokio_tungstenite::tungstenite::http::Uri;
use sg_core::models::Task;
use crate::{Certificates, ID, start_foca, StdResolver, ws_transport};

pub trait Worker {
    // TODO &self or &mut self?
    fn add_task(&self, task: Task) -> bool;
    fn remove_task(&self, id: Uuid) -> bool;
}

pub struct NodeConfig<A> {
    bind: A,
    base_uri: Uri,
    certificates: Certificates,
    ident: ID
}

pub async fn start_worker<A: ToSocketAddrs>(worker: impl Worker, config: NodeConfig<A>) -> Result<()> {
    let listener = TcpListener::bind(config.bind).await?;
    let (stream, sink) = ws_transport(listener, config.certificates, config.base_uri, StdResolver).await;
    let foca = start_foca(config.ident, stream, sink, None);

    let ring = Ring::default();
}