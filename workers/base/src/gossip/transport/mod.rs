//! Transport implementations for Foca runtime.
use std::collections::HashMap;
use std::sync::{Arc, Mutex as StdMutex};

use async_trait::async_trait;
use eyre::Result;
use futures::stream::{SplitSink, Stream};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio_rustls::TlsStream;
use tokio_tungstenite::tungstenite::{http::Uri, Message};
use tokio_tungstenite::WebSocketStream;

pub use certificate::Certificates;
pub use websocket::ws_transport;

pub mod certificate;
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
