//! Coordinator binary.
#![allow(clippy::module_name_repetitions, clippy::default_trait_access)]
#![deny(missing_docs)]

use std::net::SocketAddr;
use std::str::FromStr;

use eyre::Result;
use tracing::level_filters::LevelFilter;

use crate::app::App;

pub mod app;
pub mod config;
pub mod worker;

#[cfg(test)]
mod tests;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(LevelFilter::DEBUG)
        .init();
    let app = App::default();
    app.serve(SocketAddr::from_str("127.0.0.1:7000").unwrap())
        .await?;
    Ok(())
}
