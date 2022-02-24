//! Coordinator binary.
#![allow(clippy::module_name_repetitions, clippy::default_trait_access)]
#![deny(missing_docs)]

use eyre::Result;
use tracing::level_filters::LevelFilter;

use crate::app::App;
use crate::config::Config;

pub mod app;
pub mod config;
pub mod utils;
pub mod worker;

#[cfg(test)]
mod tests;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    color_eyre::install()?;
    tracing_subscriber::fmt()
        .with_max_level(LevelFilter::DEBUG)
        .init();
    let app = App::new(Config::from_env()?);

    app.serve().await?;
    Ok(())
}
