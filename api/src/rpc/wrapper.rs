//! Wrapper for RPC calls. Includes wrapper for [`Request`](crate::rpc::Request) types and [`Response`](crate::rpc::Request) types.

use std::ops::{Deref, DerefMut};

use serde::{Deserialize, Serialize};

use crate::{rpc::ApiError, timestamp};

/// Wrapper for RPC response. Contains processed time, success indicator and payload. For more information, see [module doc](index.html#response).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[must_use]
pub struct ResponseObject<T> {
    pub data: T,
    pub success: bool,
    pub time: String,
}

impl<T> ResponseObject<T> {
    #[inline]
    pub fn new(data: T, success: bool) -> Self {
        Self::with_time(data, success, timestamp())
    }

    #[inline]
    pub fn with_time(data: T, success: bool, time: String) -> Self {
        Self {
            data,
            success,
            time,
        }
    }

    #[inline]
    pub fn success(data: T) -> Self {
        Self::new(data, true)
    }

    #[inline]
    pub fn error(data: T) -> Self {
        Self::new(data, false)
    }

    #[inline]
    pub fn is_success(&self) -> bool {
        self.success
    }

    #[inline]
    pub fn is_error(&self) -> bool {
        !self.success
    }
}

impl<T: Serialize> ResponseObject<T> {
    #[inline]
    pub fn to_json(&self) -> String {
        match serde_json::to_string(&self) {
            Ok(res) => res,
            Err(err) => {
                tracing::error!("Failed to serialize response object: {}", err);
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

impl<T> Deref for ResponseObject<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<T> DerefMut for ResponseObject<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}
