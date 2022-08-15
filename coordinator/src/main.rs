//! Coordinator binary.
#![allow(
    clippy::module_name_repetitions,
    clippy::default_trait_access,
    clippy::redundant_pub_crate
)]
#![deny(missing_docs)]

use eyre::Result;
use tracing::level_filters::LevelFilter;

use crate::{app::App, config::Config, db::DB};

pub mod app;
pub mod config;
pub mod db;
pub mod worker;

#[cfg(test)]
mod tests;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    color_eyre::install()?;
    tracing_subscriber::fmt()
        .with_max_level(LevelFilter::DEBUG)
        .init();

    let config = Config::from_env()?;

    let app = App::new(config.clone());
    let mut db = DB::new(app.clone(), config).await?;

    db.init_tasks().await?;

    tokio::select! {
        r = app.serve() => r?,
        r = db.watch_tasks() => r?,
    };

    Ok(())
}
