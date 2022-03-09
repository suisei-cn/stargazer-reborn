use std::sync::Arc;

use axum::response::{IntoResponse, Response as AxumResponse};
use sg_core::models::User;

use crate::{
    rpc::{
        models::{GetUser, GetUserSettings, Requests, UserSettings},
        ApiError, Response,
    },
    server::DB,
};

#[derive(Debug, Clone)]
pub struct Context {
    pub db: Arc<DB>,
}

macro_rules! dispatch {
    ($self:ident, $ctx:ident, $( $req_variant: ident => $fn: ident),* ) => {
        match $self {
            $(
                Requests::$req_variant(req) => match $fn(req, $ctx.clone()).await {
                    Ok(res) => res.packed().into_response(),
                    Err(e) => e.packed().into_response(),
                }
            )*
            Self::Unknown => ApiError::unknown_method().packed().into_response(),
            #[allow(unreachable_patterns)]
            _ => {
                tracing::log::warn!("Method not implemented");
                ApiError::internal_error().into_response()
            },
        }
    };
}

impl Requests {
    pub async fn handle(self, ctx: Context) -> AxumResponse {
        dispatch![
            self, ctx,
            GetUser => get_user,
            GetUserSettings => get_user_settings
        ]
    }
}

async fn get_user(req: GetUser, _ctx: Context) -> Result<User, ApiError> {
    todo!()
}

async fn get_user_settings(req: GetUserSettings, _ctx: Context) -> Result<UserSettings, ApiError> {
    Ok(UserSettings::new())
}
