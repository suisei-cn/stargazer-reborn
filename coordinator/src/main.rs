//! Coordinator binary.
#![allow(clippy::module_name_repetitions, clippy::default_trait_access)]
#![deny(missing_docs)]

use eyre::Result;
use tracing::level_filters::LevelFilter;
use uuid::Uuid;

use sg_core::models::Task;

use crate::app::App;

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
    let app = App::default();

    // TODO debug code
    for _ in 0..20 {
        app.add_task(Task {
            id: Uuid::new_v4(),
            kind: "dummy".to_string(),
            params: Default::default(),
        })
        .await;
    }

    app.serve("127.0.0.1:7000").await?;
    Ok(())
}
