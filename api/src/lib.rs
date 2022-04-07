#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(clippy::all)]
#![allow(clippy::module_name_repetitions)]

mod_use::mod_use![utils];

pub mod rpc;
pub use rpc::*;

#[cfg(any(feature = "client", feature = "client_blocking"))]
pub mod client;

#[cfg(feature = "server")]
pub mod server;

#[cfg(test)]
#[cfg(all(feature = "server", feature = "client_blocking"))]
mod test;
