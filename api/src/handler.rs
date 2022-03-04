use std::sync::Arc;

use axum::{extract::Extension, http::StatusCode, response::IntoResponse};

use crate::DB;

pub async fn get_user_settings(Extension(_db): Extension<Arc<DB>>) -> impl IntoResponse {
    (StatusCode::OK, "OK")
}
