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
mod ident;
mod resolver;
mod runtime;
#[cfg(test)]
mod tests;
mod transport;
