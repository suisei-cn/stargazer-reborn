use axum::{http::StatusCode, response::Response as AxumResponse, Json};
use serde::{Deserialize, Serialize};
use tracing::log::error;

use crate::{
    rpc::{ApiError, Response},
    timestamp,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseObject<T> {
    pub data: T,
    pub success: bool,
    pub time: String,
}

impl<T> ResponseObject<T> {
    #[inline]
    #[must_use]
    pub fn new(data: T, success: bool) -> Self {
        Self::with_time(data, success, timestamp())
    }

    #[inline]
    #[must_use]
    pub fn with_time(data: T, success: bool, time: String) -> Self {
        Self {
            data,
            success,
            time,
        }
    }

    #[inline]
    #[must_use]
    pub fn success(data: T) -> Self {
        Self::new(data, true)
    }

    #[inline]
    #[must_use]
    pub fn error(data: T) -> Self {
        Self::new(data, false)
    }
}

impl<T: Serialize> ResponseObject<T> {
    #[inline]
    pub fn to_json(&self) -> String {
        match serde_json::to_string(&self) {
            Ok(res) => res,
            Err(err) => {
                error!("Failed to serialize response object: {}", err);
                ApiError::internal_error().packed().to_json()
            }
        }
    }
}

impl<'a, T: Deserialize<'a>> ResponseObject<T> {
    #[inline]
    pub fn try_from_json(json: &'a str) -> serde_json::error::Result<T> {
        serde_json::from_str(json)
    }
}

impl<R: Response> axum::response::IntoResponse for ResponseObject<R> {
    fn into_response(self) -> AxumResponse {
        let status = if self.success {
            StatusCode::OK
        } else {
            StatusCode::BAD_REQUEST
        };

        let mut body = Json(self).into_response();
        *body.status_mut() = status;
        body
    }
}
