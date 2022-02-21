//! Core definitions and protocols for Stargazer.
#![allow(clippy::module_name_repetitions)]
#![deny(missing_docs)]

pub use error::SerializedError;

pub mod adapter;
mod error;
pub mod models;
pub mod protocol;
