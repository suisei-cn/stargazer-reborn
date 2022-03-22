use std::sync::Arc;

use axum::{extract::Extension, routing::post, Json, Router};
use color_eyre::Result;

use crate::{
    rpc::models::Requests,
    server::{Config, Context, DB},
};

mod_use::mod_use![db, config, handler, jwt, context];

pub async fn serve() -> Result<()> {
    let config = Config::from_env()?;
    let db = DB::new(&config).await?;
    let jwt = Arc::new(JWTContext::new(&config));
    let ctx = Context { db, jwt };

    let server = Router::new()
        .route(
            "/v1",
            post(|Json(req): Json<Requests>, Extension(ctx): Extension<Context>| req.handle(ctx)),
        )
        .layer(Extension(ctx))
        .into_make_service();

    axum::Server::bind(&config.bind).serve(server).await?;

    Ok(())
}
