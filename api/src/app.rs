use std::sync::Arc;

use axum::{extract::Extension, routing::get, Router};
use color_eyre::Result;

use crate::{get_user_settings, DB};

pub(crate) async fn get_app() -> Result<Router> {
    let db = DB::new().await?;

    Ok(Router::new()
        .route("/user/settings", get(get_user_settings))
        .layer(Extension(Arc::new(db))))
}
