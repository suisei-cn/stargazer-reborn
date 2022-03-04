use serde::{Deserialize, Serialize};

use crate::{model::GetUser, timestamp};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", content = "params")]
#[serde(rename_all = "camelCase")]
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
    pub fn to_json(&self) -> serde_json::error::Result<String> {
        serde_json::to_string(&self)
    }
}
