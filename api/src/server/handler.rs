use crate::{
    rpc::{
        models::{
            AddEntity, AddTask, AddUser, AuthUser, Authorized, DelUser, Entities, GetEntities,
            NewSession, Null, Requests, Session, UpdateUserSetting,
        },
        ApiError, ApiResult, Request, Response,
    },
    server::Context,
};

use axum::response::{IntoResponse, Response as AxumResponse};
use futures::{
    future::{try_join, BoxFuture},
    Future, FutureExt, TryStreamExt,
};
use mongodb::bson::{doc, Uuid};
use sg_core::models::{self as m, Entity, EventFilter, Task, User};

fn assert_method<T: Request, M: Method<T>>(_: &M) {}

pub(crate) trait Method<Req: Request> {
    fn handle(self, req: Req, context: Context) -> BoxFuture<'static, ApiResult<Req::Res>>;
}

impl<Req, M, F> Method<Req> for M
where
    Req: Request,
    F: Future<Output = ApiResult<Req::Res>> + Send + 'static,
    M: FnOnce(Req, Context) -> F,
{
    fn handle(self, req: Req, context: Context) -> BoxFuture<'static, ApiResult<Req::Res>> {
        self(req, context).boxed()
    }
}

macro_rules! dispatch {
    ($self:ident, $ctx:ident, $( $req_variant: ident => $func: expr $(,)? )* ) => {

        match $self {
            $(
                Requests::$req_variant(req) => {
                    ::tracing::debug!(
                        method = stringify!($req_variant),
                        params = ?req,
                        "Income request"
                    );
                    let func = $func;
                    assert_method::<$req_variant, _>(&func);
                    match func(req, $ctx).await {
                        Ok(res) => {
                            res.packed().into_response()
                        },
                        Err(e) => e.packed().into_response(),
                    }
                }
            )*
        Self::Unknown => ApiError::bad_request("Unknown method").packed().into_response(),
            #[cfg(debug_assertions)]
            #[allow(unreachable_patterns)]
            n => {
                tracing::warn!(method = ?n, "Method not implemented");
                ApiError::internal_error().into_response()
            },
        }
    };
}

impl Requests {
    /// Dispatch the request to the appropriate handler.
    ///
    /// # Unimplemented methods
    /// Under debug mode, unimplemented methods will compile,
    /// return an internal error and log a warning message during runtime.
    ///
    /// While in release mode, unimplemented methods will simply not compile.
    pub async fn handle(self, ctx: Context) -> AxumResponse {
        dispatch![
            self, ctx,
            // GetUser => |req: GetUser, ctx: Context| async move { ctx.find_user(&req.user_id).await.map(Into::into) },
            GetEntities => get_entities,
            AddUser => add_user,
            AddEntity => add_entity,
            AddTask => add_task,
            AuthUser => auth_user,
            DelUser => del_user,
            NewSession => new_session,
            UpdateUserSetting => update_user_setting,
        ]
    }
}

async fn add_entity(req: AddEntity, ctx: Context) -> ApiResult<Entity> {
    let AddEntity { meta, tasks, token } = req;

    ctx.validate_token(token)?.assert_admin()?;

    tracing::info!(meta = ?meta);

    let mut ent = Entity {
        id: Uuid::new(),
        meta,
        tasks: vec![],
    };

    ctx.entities().insert_one(&ent, None).await?;

    let tasks = tasks
        .into_iter()
        .map(|x| x.into_task_with(ent.id))
        .collect::<Vec<_>>();

    ctx.tasks().insert_many(&tasks, None).await?;

    ent.tasks = tasks.into_iter().map(|x| x.id).collect();

    Ok(ent)
}

async fn add_task(req: AddTask, ctx: Context) -> ApiResult<Task> {
    ctx.validate_token(&req.token)?.assert_admin()?;

    let id = req.entity_id;
    let task: Task = req.into();

    ctx.tasks().insert_one(&task, None).await?;

    let res = ctx
        .entities()
        .update_one(
            doc! { "id": id },
            doc! { "$push": { "tasks": task.id } },
            None,
        )
        .await?;

    match res.matched_count {
        1 => Ok(task),
        0 => Err(ApiError::entity_not_found(&id)),
        n => {
            tracing::error!("One entity_id mapped to {} entities", n);
            Ok(task)
        }
    }
}

async fn update_user_setting(req: UpdateUserSetting, ctx: Context) -> ApiResult<Null> {
    let UpdateUserSetting {
        token,
        event_filter,
    } = req;

    let user_id = ctx.validate_token(&token)?.id();

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

    let user = m::User {
        im,
        avatar,
        name,
        is_admin: false,
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

async fn auth_user(AuthUser { token, user_id }: AuthUser, ctx: Context) -> ApiResult<Authorized> {
    let claims = ctx.validate_token(&token)?;
    let user = ctx.find_user(&user_id).await?;

    Ok(Authorized {
        user,
        valid_until: claims.valid_until(),
    })
}

async fn new_session(req: NewSession, ctx: Context) -> ApiResult<Session> {
    let NewSession {
        ref user_id,
        password,
    } = req;

    ctx.auth_password(password)?;

    // make sure user exists
    let user = ctx.find_user(user_id).await?;

    let (token, claim) = match ctx.jwt.encode(user_id, user.is_admin) {
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
