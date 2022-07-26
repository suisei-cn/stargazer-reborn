//! Worker config.

use tokio_tungstenite::tungstenite::http::Uri;

use crate::{Certificates, ID};

/// Configuration for worker nodes.
pub struct NodeConfig<A> {
    // TODO this can be a list.
    /// A peer URL to announce to the rest of the cluster.
    ///
    /// This is optional. If not set, the worker will be started at idle state.
    pub announce: Option<Uri>,
    /// Socket address to bind to for gossip protocol.
    pub bind: A,
    /// URI of this node to announce to the rest of the cluster.
    pub base_uri: Uri,
    /// TLS certificates to use for the gossip protocol.
    pub certificates: Certificates,
    /// Identity of this node.
    pub ident: ID,
    /// MongoDB configuration.
    pub db: DBConfig,
}

/// Database configuration.
pub struct DBConfig {
    /// MongoDB connection URI.
    pub uri: String,
    /// Database name.
    pub db: String,
    /// Collection name.
    pub collection: String,
}
