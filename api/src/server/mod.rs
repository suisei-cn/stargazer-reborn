use std::sync::Arc;

use axum::{extract::Extension, routing::post, Json, Router};
use color_eyre::Result;

use crate::rpc::models::Requests;

#[cfg(test)]
mod test;

mod_use::mod_use![db, config, handler, jwt, context];

pub async fn serve_with_config(config: Config) -> Result<()> {
    let config = Arc::new(config);
    tracing::debug!(config = ?config);
    let db = DB::new(&config).await?;
    let jwt = Arc::new(JWTContext::new(&config));
    let server = axum::Server::bind(&config.bind);

    let app = Router::new()
        .route(
            "/v1",
            post(|Json(req): Json<Requests>, Extension(ctx): Extension<Context>| req.handle(ctx)),
        )
        .layer(Extension(Context { db, jwt, config }))
        .into_make_service();

    tracing::info!("Server starting");

    server.serve(app).await?;

    tracing::info!("Server stopped");

    Ok(())
}

pub async fn serve() -> Result<()> {
    serve_with_config(Config::from_env()?).await
}
