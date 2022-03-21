use axum::{extract::Extension, routing::post, Json, Router};
use color_eyre::Result;

use api::{
    rpc::models::Requests,
    server::{Config, Context, DB},
};

pub async fn serve() -> Result<()> {
    let config = Config::from_env()?;
    let db = DB::new(&config).await?;
    let ctx = Context { db };

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

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    serve().await?;

    Ok(())
}
