//! Errors for the core library.
use thiserror::Error;

/// Errors that may occur during transport.
#[derive(Debug, Error)]
pub enum TransportError {
    /// Bincode can't (de)serialize the message.
    #[error("Json error: {0}")]
    Serialize(#[from] serde_json::Error),
    /// An error occurred on the websocket stream.
    #[error("Websocket error")]
    Websocket(#[from] tokio_tungstenite::tungstenite::Error),
}
