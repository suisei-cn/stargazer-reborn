#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(clippy::all)]
#![allow(clippy::redundant_pub_crate)]
#![allow(clippy::missing_errors_doc)]

use tracing::level_filters::LevelFilter;

mod_use::mod_use![bot, command, config, ext, util];

#[tokio::main]
async fn main() {
    color_eyre::install().unwrap();

    tracing_subscriber::fmt()
        .with_max_level(
            std::env::var("BOT_LOG")
                .as_deref()
                .unwrap_or("info")
                .parse::<LevelFilter>()
                .unwrap(),
        )
        .init();

    init_from_env().await;
    start_bot().await.unwrap();
}
