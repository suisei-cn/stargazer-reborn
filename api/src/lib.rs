#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(clippy::all)]

mod_use::mod_use![utils];

pub mod rpc;

#[cfg(any(feature = "client", feature = "client_blocking"))]
pub mod client;

#[cfg(feature = "server")]
pub mod server;

pub use rpc::*;
