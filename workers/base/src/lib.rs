//! Support library for all workers.
//!
//! Contains gossip protocol for auto discovery.
#![allow(
    clippy::module_name_repetitions,
    clippy::default_trait_access,
    clippy::cast_possible_truncation
)]
#![warn(missing_docs)]

pub use common::Worker;
pub use config::{DBConfig, NodeConfig};
pub use gossip::{Certificates, ID};
pub use worker::start_worker;

mod change_events;
mod common;
mod config;
mod gossip;
pub mod ring;
mod worker;
