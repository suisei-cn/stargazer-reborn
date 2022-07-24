//! Support library for all workers. Mainly contains gossip protocol for auto discovery.
#![allow(
    clippy::module_name_repetitions,
    clippy::default_trait_access,
    clippy::cast_possible_truncation
)]

pub use ident::ID;
pub use transport::Certificates;
pub use worker::{start_worker, DBConfig, NodeConfig, Worker};

mod compression;
mod db;
mod ident;
mod resolver;
#[cfg(not(feature = "fuzzing"))]
mod ring;
#[cfg(feature = "fuzzing")]
pub mod ring;
mod runtime;
#[cfg(test)]
mod tests;
mod transport;
mod worker;
