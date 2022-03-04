#![allow(macro_expanded_macro_exports_accessed_by_absolute_paths)]

mod_use::mod_use![rpc, app, config, db, handler, utils];

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
