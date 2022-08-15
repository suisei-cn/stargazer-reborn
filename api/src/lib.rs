#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(clippy::all)]
#![allow(clippy::module_name_repetitions)]

pub use rpc::*;

mod_use::mod_use![utils];

pub mod rpc;

#[cfg(any(feature = "client", feature = "client_blocking"))]
pub mod client;

#[cfg(feature = "server")]
pub mod server;

#[cfg(all(feature = "server", feature = "client_blocking"))]
#[cfg(test)]
mod test;
