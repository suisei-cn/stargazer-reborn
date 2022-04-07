//! Core definitions and protocols for Stargazer.
#![allow(clippy::module_name_repetitions)]
#![deny(missing_docs)]

pub mod adapter;
pub mod error;
pub mod models;
#[cfg(feature = "mq")]
pub mod mq;
pub mod protocol;
pub mod utils;

pub use async_trait;
