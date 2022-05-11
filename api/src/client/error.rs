use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Reqwest error: {0}")]
    Reqwest(#[from] reqwest::Error),
    #[error("Serde Json error: {0}")]
    SerdeJson(#[from] serde_json::Error),
    #[error("Unable to parse url: {0}")]
    Url(#[from] url::ParseError),
    #[error("API error: {0}")]
    Api(#[from] crate::rpc::ApiError),
}

impl Error {
    #[must_use]
    pub const fn is_api(&self) -> bool {
        matches!(self, Error::Api(_))
    }

    #[must_use]
    pub const fn as_api(&self) -> Option<&crate::rpc::ApiError> {
        if let Error::Api(api_error) = self {
            Some(api_error)
        } else {
            None
        }
    }

    // Allow b/c destructor cannot be evaluated at compile time
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn into_api(self) -> Option<crate::rpc::ApiError> {
        if let Error::Api(api_error) = self {
            Some(api_error)
        } else {
            None
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;
