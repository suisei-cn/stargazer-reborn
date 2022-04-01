use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Reqwest error: {0}")]
    Reqwest(#[from] reqwest::Error),
    #[error("Serde Json error: {0}")]
    SerdeJson(#[from] serde_json::Error),
    #[error("Unable to parse url: {0}")]
    Url(#[from] url::ParseError),
}

pub type Result<T> = std::result::Result<T, Error>;
