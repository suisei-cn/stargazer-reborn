use axum::response::{IntoResponse, Response as AxumResponse};
use futures::{future::try_join, TryStreamExt};
use sg_core::models::User;

use crate::{
    rpc::{
        models::{AuthMe, Entities, GetEntities, GetUser, Requests},
        ApiError, Response,
    },
    server::Context,
};

macro_rules! dispatch {
    ($self:ident, $ctx:ident, $( $req_variant: ident => $fn: ident $(,)? )* ) => {
        match $self {
            $(
                Requests::$req_variant(req) => match $fn(req, $ctx.clone()).await {
                    Ok(res) => res.packed().into_response(),
                    Err(e) => e.packed().into_response(),
                }
            )*
            Self::Unknown => ApiError::bad_request("Bad method or request body").packed().into_response(),
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
            GetEntities => get_entities,
            AuthMe => auth_me
        ]
    }
}

async fn get_user(req: GetUser, ctx: Context) -> Result<User, ApiError> {
    let id = req.user_id.as_str();
    ctx.users()
        .find_one(mongodb::bson::doc! { "id": id }, None)
        .await?
        .ok_or_else(|| ApiError::user_not_found(id))
}

async fn get_entities(_: GetEntities, ctx: Context) -> Result<Entities, ApiError> {
    let (entities, groups) = try_join(
        ctx.entities().find(None, None),
        ctx.groups().find(None, None),
    )
    .await?;
    let vtbs = entities.map_ok(Into::into).try_collect().await?;
    let groups = groups.try_collect().await?;

    Ok(Entities { vtbs, groups })
}

async fn auth_me(req: AuthMe, ctx: Context) -> Result<User, ApiError> {
    ctx.jwt.as_ref().api_validate(&req.token, &req.user_id)?;

    let user = ctx
        .users()
        .find_one(mongodb::bson::doc! { "id": &req.user_id }, None)
        .await?
        .ok_or_else(|| ApiError::user_not_found(req.user_id.as_str()))?;

    Ok(user)
}
