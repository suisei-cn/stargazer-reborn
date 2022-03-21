use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::rpc::{Response, ResponseObject};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiError {
    pub error: Vec<String>,
}

impl axum::response::IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        self.packed().into_response()
    }
}

impl From<mongodb::error::Error> for ApiError {
    fn from(err: mongodb::error::Error) -> Self {
        let err_str = err.to_string();
        warn!(target: "mongo error", error = err_str.as_str());
        ApiError::internal_error()
    }
}

/// Represents an API Error. Implemented [`IntoResponse`] trait.
///
/// # Examples
/// ## Format into JSON
/// ```rust
/// # use api::rpc::ApiError; fn main() {
/// let resp = r#"{"data":{"error":["User `foo` not found"]},"success":false,"time":0}"#;
///
/// let mut resp_obj = ApiError::user_not_found("foo").packed();
/// # resp_obj.time = 0;
///
/// assert_eq!(resp, resp_obj.to_json());
/// # }
/// ```
///
/// ## Usage with `Axum`
///
/// ```rust
/// # use api::rpc::ApiError; fn main() {
/// use axum::response::IntoResponse;
///
/// let error = ApiError::new(vec!["error1".to_string(), "error2".to_string()]);
/// let response = error.packed().into_response();
/// # }
/// ```
///
/// [`IntoResponse`]: axum::response::IntoResponse
impl ApiError {
    pub fn new(error: Vec<String>) -> Self {
        Self { error }
    }

    pub fn packed(self) -> ResponseObject<Self> {
        ResponseObject::error(self)
    }

    pub fn unauthorized() -> Self {
        Self::new(vec!["Unauthorized".to_owned()])
    }

    pub fn user_not_found(user_id: &str) -> Self {
        Self::new(vec![format!("User `{}` not found", user_id)])
    }

    pub fn bad_request(error: impl Into<String>) -> Self {
        Self::new(vec!["Bad request".to_owned(), error.into()])
    }

    pub fn internal_error() -> Self {
        Self::new(vec!["Internal Error".to_owned()])
    }
}

impl Response for ApiError {
    fn is_successful(&self) -> bool {
        false
    }
}
