//! Errors for the core library.
use std::error::Error;
use std::fmt::{Display, Formatter};

use eyre::Report;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors that may occur during transport.
#[derive(Debug, Error)]
pub enum TransportError {
    #[error("Bincode error")]
    Serialize(#[from] bincode::Error),
    #[error("Websocket error")]
    Websocket(#[from] tokio_tungstenite::tungstenite::Error),
}

/// Represents a end-user friendly serializable error.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializedError {
    desc: String,
    cause: Option<Box<SerializedError>>,
}

impl SerializedError {
    /// Convert `SerializedError` into `eyre::Report`.
    #[must_use]
    pub fn into_report(self) -> Report {
        Report::new(self)
    }

    /// Convert an error into `SerializedError`.
    #[must_use]
    pub fn from_error(e: impl Into<Report>) -> Self {
        let report = e.into();
        let e = report
            .chain()
            .rfold(None, |acc, x| {
                acc.map_or_else(
                    || {
                        Some(Self {
                            desc: x.to_string(),
                            cause: None,
                        })
                    },
                    |acc| {
                        Some(Self {
                            desc: x.to_string(),
                            cause: Some(Box::new(acc)),
                        })
                    },
                )
            })
            .expect("must have one error");
        Self {
            desc: e.desc,
            cause: e.cause,
        }
    }
}

impl Display for SerializedError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.desc.as_str())
    }
}

impl Error for SerializedError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.cause.as_ref().map(|e| &**e as &dyn Error)
    }
}
