use axum::{http::StatusCode, response::Response as AxumResponse, Json};
use serde::{Deserialize, Serialize};

use crate::{
    rpc::{model::GetUser, Response},
    timestamp,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", content = "params")]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub enum Requests {
    GetUser(GetUser),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseObject<T> {
    pub data: T,
    pub success: bool,
    pub time: i64,
}

impl<T> ResponseObject<T> {
    #[inline]
    #[must_use]
    pub fn new(data: T, success: bool) -> Self {
        Self::with_time(data, success, timestamp())
    }

    #[inline]
    #[must_use]
    pub fn with_time(data: T, success: bool, time: i64) -> Self {
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
        serde_json::to_string(&self).expect("Failed to serialize response object")
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
