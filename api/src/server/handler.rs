use crate::{
    rpc::{
        models::{
            AddUser, AuthUser, Authorized, DelUser, Entities, GetEntities, GetUser, NewSession,
            Null, Requests, Session,
        },
        ApiError, Response,
    },
    server::Context,
};

use axum::response::{IntoResponse, Response as AxumResponse};
use futures::{future::try_join, TryStreamExt};
use mongodb::bson::doc;
use sg_core::models::{EventFilter, User};

macro_rules! dispatch {
    ($self:ident, $ctx:ident, $( $req_variant: ident => $fn: expr $(,)? )* ) => {
        match $self {
            $(
                Requests::$req_variant(req) => {
                    ::tracing::debug!(
                        method = stringify!($req_variant),
                        params = ?req,
                        "Received request"
                    );
                    match $fn(req, $ctx).await {
                        Ok(res) => res.packed().into_response(),
                        Err(e) => e.packed().into_response(),
                    }
                }
            )*
            Self::Unknown => ApiError::bad_request("Unknown method").packed().into_response(),
            #[allow(unreachable_patterns)]
            _ => {
                tracing::warn!("Method not implemented");
                ApiError::internal_error().into_response()
            },
        }
    };
}

impl Requests {
    /// Dispatch the request to the appropriate handler.
    ///
    /// Unimplemented methods will return an internal error and log a warn.
    pub async fn handle(self, ctx: Context) -> AxumResponse {
        dispatch![
            self, ctx,
            GetUser => |req: GetUser, ctx: Context| async move { ctx.find_user(&req.user_id).await },
            GetEntities => get_entities,
            AddUser => add_user,
            AuthUser => auth_user,
            DelUser => del_user,
            NewSession => new_session,
            UpdateUserSetting => update_user_setting,
        ]
    }
}

async fn update_user_setting(req: UpdateUserSetting, ctx: Context) -> ApiResult<Null> {
    let UpdateUserSetting {
        user_id,
        token,
        event_filter,
    } = req;

    ctx.validate_token(&token, &user_id)?;

    let serialized = mongodb::bson::to_bson(&event_filter)
        .expect("reqs deserialized from JSON should be legal to be serialized again");

    let res = ctx
        .users()
        .update_one(
            doc! { "id": user_id },
            doc! { "$set": { "event_filter": serialized } },
            None,
        )
        .await?;

    match res.matched_count {
        1 => Ok(Null),
        0 => Err(ApiError::user_not_found(&user_id)),
        n => {
            tracing::error!("One user_id mapped to {} users", n);
            Ok(Null)
        }
    }
}

async fn get_entities(_: GetEntities, ctx: Context) -> ApiResult<Entities> {
    let (entities, groups) = try_join(
        ctx.entities().find(None, None),
        ctx.groups().find(None, None),
    )
    .await?;
    let vtbs = entities.map_ok(Into::into).try_collect().await?;
    let groups = groups.try_collect().await?;

    Ok(Entities { vtbs, groups })
}

async fn add_user(req: AddUser, ctx: Context) -> ApiResult<User> {
    let AddUser {
        im,
        avatar,
        password,
        name,
    } = req;

    ctx.auth_password(password)?;

    let user = User {
        im,
        avatar,
        name,
        event_filter: EventFilter {
            entities: Default::default(),
            kinds: Default::default(),
        },
        id: Default::default(),
    };

    ctx.users().insert_one(&user, None).await?;

    Ok(user)
}

async fn del_user(req: DelUser, ctx: Context) -> ApiResult<Null> {
    let DelUser { password, user_id } = req;

    ctx.auth_password(password)?;
    ctx.find_user(&user_id).await?;
    ctx.users().delete_one(doc! { "id": user_id }, None).await?;

    Ok(Null)
}

async fn auth_user(req: AuthUser, ctx: Context) -> ApiResult<Authorized> {
    let AuthUser { user_id, token } = &req;

    let claims = ctx.validate_token(token, user_id)?;

    let user = ctx.find_user(user_id).await?;

    Ok(Authorized {
        user,
        valid_until: claims.valid_until(),
    })
}

async fn new_session(req: NewSession, ctx: Context) -> ApiResult<Session> {
    let NewSession { user_id, password } = &req;

    ctx.auth_password(password)?;

    // make sure user exists
    ctx.find_user(user_id).await?;

    let (token, claim) = match ctx.jwt.encode(user_id) {
        Ok(token) => token,
        Err(e) => {
            tracing::error!(error = %e, "Failed to generate token");
            return Err(ApiError::internal_error());
        }
    };

    Ok(Session {
        token,
        valid_until: claim.valid_until(),
    })
}
