use std::error::Error;
use std::sync::Arc;

use axum::extract::{Extension, Query};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Router, Server};
use eyre::Result;
use parking_lot::RwLock;
use serde::Deserialize;
use tracing::{error, info, warn};

use sg_core::mq::MessageQueue;

use crate::models::{ChallengeQuery, Mode};
use crate::registry::Registry;
use crate::Config;

struct ChallengeError;

impl<E: Error> From<E> for ChallengeError {
    fn from(_: E) -> Self {
        ChallengeError
    }
}

impl IntoResponse for ChallengeError {
    fn into_response(self) -> Response {
        StatusCode::NOT_FOUND.into_response()
    }
}

#[derive(Deserialize)]
struct TopicQuery {
    topic: String,
}

#[allow(clippy::unused_async)]
async fn challenge(
    Query(query): Query<ChallengeQuery>,
    Extension(registry): Extension<Arc<RwLock<Registry>>>,
) -> Result<String, ChallengeError> {
    let channel_id =
        serde_urlencoded::from_str::<TopicQuery>(query.topic.query().unwrap_or_default())?.topic;
    let has_task = registry.read().contains_channel(&channel_id);
    let mode = query.mode;
    if (mode == Mode::Subscribe && has_task) || (mode == Mode::Unsubscribe && !has_task) {
        info!(?mode, ?channel_id, "Accepting callback challenge.");
        Ok(query.challenge)
    } else {
        warn!(?mode, ?channel_id, "Rejecting callback challenge.");
        Err(ChallengeError)
    }
}

struct EventError;

impl<E: Error> From<E> for EventError {
    fn from(_: E) -> Self {
        error!("Failed to handle event.");
        EventError
    }
}

impl IntoResponse for EventError {
    fn into_response(self) -> Response {
        ().into_response()
    }
}

#[allow(clippy::unused_async)]
async fn event(Extension(_registry): Extension<Arc<RwLock<Registry>>>) -> Result<(), EventError> {
    todo!()
}

pub async fn serve(
    config: &Config,
    registry: Arc<RwLock<Registry>>,
    mq: impl MessageQueue + 'static,
) -> Result<()> {
    let mq = Arc::new(mq) as Arc<dyn MessageQueue>;

    let app = Router::new()
        .route("/callback", get(challenge).post(event))
        .layer(Extension(registry))
        .layer(Extension(mq));

    info!("Start serving callback on {}", config.bind);

    Ok(Server::bind(&config.bind)
        .serve(app.into_make_service())
        .await?)
}
