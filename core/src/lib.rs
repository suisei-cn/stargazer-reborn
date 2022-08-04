//! Core definitions and protocols for Stargazer.
#![allow(clippy::module_name_repetitions, clippy::default_trait_access)]
#![deny(missing_docs)]

pub use async_trait;

pub mod adapter;
pub mod error;
pub mod models;
#[cfg(feature = "mq")]
pub mod mq;
pub mod protocol;
pub mod utils;
