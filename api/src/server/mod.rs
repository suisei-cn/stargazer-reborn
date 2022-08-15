//! Server implementation of the RPC API.

use color_eyre::Result;
use sg_core::utils::FigmentExt;

mod_use::mod_use![config, handler, jwt, context, ext];

#[allow(clippy::missing_errors_doc)]
pub async fn serve_with_config(config: Config) -> Result<()> {
    tracing::debug!(config = ?config);

    let server = axum::Server::bind(&config.bind);

    let app = make_app(config).await?.into_make_service();

    tracing::info!("Server starting");

    server.serve(app).await?;

    tracing::info!("Server stopped");

    Ok(())
}

#[allow(clippy::missing_errors_doc)]
pub async fn serve() -> Result<()> {
    serve_with_config(Config::from_env("API_")?).await
}
