use std::sync::Arc;

use axum::extract::Extension;
use color_eyre::Result;
use http::Method;

#[cfg(test)]
mod test;

mod_use::mod_use![config, handler, jwt, context, ext];

pub async fn serve_with_config(config: Config) -> Result<()> {
    let config = Arc::new(config);
    tracing::debug!(config = ?config);

    let server = axum::Server::bind(&config.bind);
    let jwt = Arc::new(JWTContext::new(&config));

    let ctx = Context::new(jwt, config).await?;
    let cors_layer = tower_http::cors::CorsLayer::new()
        // Allow `POST` when accessing the resource
        .allow_methods(vec![Method::POST])
        // Credentials should be passed in as parameter of the request(rpc) body
        .allow_credentials(false)
        // Allow requests from any origin
        .allow_origin(tower_http::cors::Any);

    let trace_layer = tower_http::trace::TraceLayer::new_for_http();

    let app = get_router()
        .layer(Extension(ctx))
        .layer(cors_layer)
        .layer(trace_layer)
        .into_make_service();

    tracing::info!("Server starting");

    server.serve(app).await?;

    tracing::info!("Server stopped");

    Ok(())
}

pub async fn serve() -> Result<()> {
    serve_with_config(Config::from_env()?).await
}
