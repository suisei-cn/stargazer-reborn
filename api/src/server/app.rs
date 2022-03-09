use std::sync::Arc;

use axum::{extract::Extension, routing::post, Json, Router};
use color_eyre::Result;

use crate::{
    rpc::models::Requests,
    server::{Context, DB},
};

pub async fn get_app() -> Result<Router> {
    let db = DB::new().await?;
    let ctx = Context { db: Arc::new(db) };

    Ok(Router::new()
        .route(
            "/v1",
            post(|Json(req): Json<Requests>, Extension(ctx): Extension<Context>| req.handle(ctx)),
        )
        .layer(Extension(ctx)))
}
