use serde::{Deserialize, Serialize};

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

impl From<jsonwebtoken::errors::Error> for ApiError {
    fn from(e: jsonwebtoken::errors::Error) -> Self {
        tracing::warn!("{}", e);
        Self::bad_token()
    }
}

impl From<mongodb::error::Error> for ApiError {
    fn from(err: mongodb::error::Error) -> Self {
        let err_str = err.to_string();
        tracing::error!(error = err_str.as_str(), "Mongo error");
        ApiError::internal_error()
    }
}

/// Represents an API Error. Implemented [`IntoResponse`] trait.
///
/// # Examples
/// ## Format into JSON
/// ```rust
/// # use api::rpc::ApiError; fn main() {
/// let resp = r#"{"data":{"error":["Cannot find user with ID `26721d57-37f5-458c-afea-2b18baf34925`"]},"success":false,"time":"2022-01-01T00:00:00.000000000Z"}"#;
///
/// let mut resp_obj = ApiError::user_not_found(
///     &"26721d57-37f5-458c-afea-2b18baf34925".parse().unwrap()
/// ).packed();
/// # resp_obj.time = "2022-01-01T00:00:00.000000000Z".to_owned();
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

    pub fn bad_token() -> Self {
        Self::new(vec![
            "Bad token".to_owned(),
            "It's either expired or in bad shape".to_owned(),
        ])
    }

    pub fn unauthorized() -> Self {
        Self::new(vec![
            "Unauthorized".to_owned(),
            "Token is valid but cannot be used with this user_id".to_owned(),
        ])
    }

    pub fn wrong_password() -> Self {
        Self::new(vec!["Wrong password".to_owned()])
    }

    pub fn user_not_found(user_id: impl AsRef<str>) -> Self {
        Self::new(vec![format!(
            "Cannot find user with ID `{}`",
            user_id.as_ref()
        )])
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
