//! Identity for a worker.

use std::fmt::{Debug, Formatter};

use foca::Identity;
use rand::random;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use tokio_tungstenite::tungstenite::http::Uri;

/// Foca identity.
///
/// Contains its protocol version, address, and worker kind.
///
/// The extra field is for fast rejoining.
#[serde_as]
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct ID {
    version: u16,
    #[serde_as(as = "DisplayFromStr")]
    addr: Uri,
    kind: String,
    extra: u16,
}

impl ID {
    /// Create a new ID.
    pub fn new(addr: Uri, kind: String) -> Self {
        Self {
            version: 0,
            addr,
            kind,
            extra: random(),
        }
    }

    /// Get the address.
    pub const fn addr(&self) -> &Uri {
        &self.addr
    }

    /// Get the kind.
    pub fn kind(&self) -> &str {
        &self.kind
    }
}

impl Debug for ID {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("ID")
            .field(&self.addr)
            .field(&self.kind)
            .finish()
    }
}

impl Identity for ID {
    fn renew(&self) -> Option<Self> {
        Some(Self {
            extra: self.extra + 1, // for fast rejoining
            ..self.clone()
        })
    }

    fn has_same_prefix(&self, other: &Self) -> bool {
        // Extra field is ignored.
        self.version == other.version && self.addr == other.addr && self.kind == other.kind
    }
}
