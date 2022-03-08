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

impl ApiError {
    pub fn new(error: Vec<String>) -> Self {
        Self { error }
    }

    pub fn owned(error: Vec<String>) -> Self {
        Self { error }
    }

    pub fn packed(self) -> ResponseObject<Self> {
        ResponseObject::error(self)
    }

    pub fn unauthorized() -> Self {
        ApiError::new(vec!["Unauthorized".to_owned()])
    }

    pub fn user_not_found<'a>(user_id: impl Into<&'a str>) -> Self {
        ApiError::new(vec![format!("User `{}` not found", user_id.into())])
    }

    pub fn bad_request(error: impl Into<String>) -> Self {
        ApiError::new(vec!["Bad request".to_owned(), error.into()])
    }
}

impl Response for ApiError {
    fn is_successful(&self) -> bool {
        false
    }
}
