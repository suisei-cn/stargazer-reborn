use api::server::{get_app, get_config};

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    let config = get_config();
    let app = get_app().await?;

    axum::Server::bind(&config.bind)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}
