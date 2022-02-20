use std::error::Error;
use std::fmt::{Display, Formatter, Write};

use eyre::Report;
use serde::{Deserialize, Serialize};

/// Represents a end-user friendly serializable error.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializedError {
    desc: String,
    cause: Option<Box<SerializedError>>,
    suggestion: Option<String>,
}

impl SerializedError {
    /// Convert `SerializedError` into `eyre::Report`.
    #[must_use]
    pub fn into_report(self) -> Report {
        let suggestion: Value = self.suggestion.clone().map(Into::into).unwrap_or_default();
        Report::new(self).section("suggestion", suggestion)
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
                            suggestion: None,
                        })
                    },
                    |acc| {
                        Some(Self {
                            desc: x.to_string(),
                            cause: Some(Box::new(acc)),
                            suggestion: None,
                        })
                    },
                )
            })
            .expect("must have one error");
        Self {
            desc: e.desc,
            cause: e.cause,
            suggestion: report.get_suggestion().map(ToString::to_string),
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
