use color_eyre::Result;

#[cfg(test)]
mod test;

mod_use::mod_use![config, handler, jwt, context, ext];

pub async fn serve_with_config(config: Config) -> Result<()> {
    tracing::debug!(config = ?config);

    let server = axum::Server::bind(&config.bind);

    let app = make_app(config).await?.into_make_service();

    tracing::info!("Server starting");

    server.serve(app).await?;

    tracing::info!("Server stopped");

    Ok(())
}

pub async fn serve() -> Result<()> {
    serve_with_config(Config::from_env()?).await
}
