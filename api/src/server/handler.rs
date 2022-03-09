use std::sync::Arc;

use axum::response::{IntoResponse, Response as AxumResponse};

use crate::{
    rpc::{
        model::{GetUser, GetUserSettings, Requests, User, UserSettings},
        ApiError,
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
            $( Requests::$req_variant(req) => $fn(req, $ctx.clone()).await.into_response(), )*
            #[allow(unreachable_patterns)]
            _ => ApiError::bad_request("Method does not exist or not implemented").into_response(),
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
    Ok(User::new(req.user_id, "test".to_owned()))
}

async fn get_user_settings(req: GetUserSettings, _ctx: Context) -> Result<UserSettings, ApiError> {
    Ok(UserSettings::new())
}
