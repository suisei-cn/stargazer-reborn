//! API Client, with both `blocking` and `non_blocking` implementation.
//!
//! This module requires either or both of `client` and `client_blocking` feature to use.

use serde::{Deserialize, Serialize};

use crate::rpc::{ApiError, ApiResult};

mod_use::mod_use![error];

#[cfg(feature = "client")]
mod non_blocking;
#[cfg(feature = "client")]
pub use non_blocking::*;

#[cfg(feature = "client_blocking")]
pub mod blocking;

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum Shim<R> {
    Ok(R),
    Err(ApiError),
}

impl<T> From<Shim<T>> for ApiResult<T> {
    fn from(shim: Shim<T>) -> Self {
        match shim {
            Shim::Ok(res) => Self::Ok(res),
            Shim::Err(err) => Self::Err(err),
        }
    }
}
