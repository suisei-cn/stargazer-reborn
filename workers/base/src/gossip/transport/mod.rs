//! Transport implementations for Foca runtime.
use std::{
    collections::HashMap,
    sync::{Arc, Mutex as StdMutex},
};

use async_trait::async_trait;
pub use certificate::Certificates;
use eyre::Result;
use futures::stream::{SplitSink, Stream};
use tokio::{net::TcpStream, sync::Mutex};
use tokio_rustls::TlsStream;
use tokio_tungstenite::{
    tungstenite::{http::Uri, Message},
    WebSocketStream,
};
pub use websocket::ws_transport;

mod certificate;
#[cfg(test)]
mod tests;
mod websocket;

type Ws = WebSocketStream<TlsStream<TcpStream>>;
type ConnPool = StdMutex<HashMap<Uri, Arc<Mutex<Option<SplitSink<Ws, Message>>>>>>;

/// Stream of gossip messages from other nodes.
pub trait GossipStream: Send + Stream<Item = Vec<u8>> + Unpin + 'static {}

/// Sink of gossip messages to other nodes.
#[async_trait]
pub trait GossipSink<Ident>: Send + Sync + Clone + 'static {
    /// Send a message to a node.
    async fn send(&self, target: Ident, payload: Vec<u8>) -> Result<()>;
}
