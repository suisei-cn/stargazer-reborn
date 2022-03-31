use crate::{
    rpc::{
        model::{
            AddEntity, AddTask, AddUser, AuthUser, Authorized, DelEntity, DelTask, DelUser,
            Entities, GetEntities, NewSession, Requests, Session, UpdateEntity, UpdateSetting,
        },
        ApiError, ApiResult, Request, Response,
    },
    server::Context,
};

use axum::response::{IntoResponse, Response as AxumResponse};
use futures::{future::try_join, Future, TryStreamExt};
use mongodb::{
    bson::{doc, to_bson, Uuid},
    options::{FindOneAndUpdateOptions, ReturnDocument},
};
use sg_core::models::{self as m, Entity, EventFilter, Task, User};

fn assert_method<T: Request, M: Method<T>>(_: &M) {}

/// Marker trait to ensure handlers are in a good shape.
pub(crate) trait Method<Req: Request> {}

impl<Req, M, F> Method<Req> for M
where
    Req: Request,
    F: Future<Output = ApiResult<Req::Res>>,
    M: FnOnce(Req, Context) -> F,
{
}

macro_rules! static_dispatch {
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
        static_dispatch![
            self, ctx,
            GetEntities => get_entities,
            AddUser => add_user,
            AddEntity => add_entity,
            AddTask => add_task,
            AuthUser => auth_user,
            DelUser => del_user,
            DelEntity => del_entity,
            DelTask => del_task,
            UpdateEntity => update_entity,
            NewSession => new_session,
            UpdateSetting => update_setting,
        ]
    }
}

async fn add_entity(req: AddEntity, ctx: Context) -> ApiResult<Entity> {
    let AddEntity { meta, tasks, token } = req;

    ctx.validate_token(token)?.ensure_admin()?;

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

async fn update_entity(req: UpdateEntity, ctx: Context) -> ApiResult<Entity> {
    let UpdateEntity {
        entity_id,
        meta,
        token,
    } = req;

    ctx.validate_token(token)?.ensure_admin()?;

    let ser = to_bson(&meta).map_err(|_| ApiError::bad_request("Invalid meta"))?;

    ctx.entities()
        .find_one_and_update(
            doc! { "id": entity_id },
            doc! { "meta": ser },
            FindOneAndUpdateOptions::builder()
                .return_document(ReturnDocument::After)
                .build(),
        )
        .await?
        .ok_or_else(|| ApiError::entity_not_found(&entity_id))
}

async fn del_entity(req: DelEntity, ctx: Context) -> ApiResult<Entity> {
    let DelEntity { entity_id, token } = req;

    ctx.validate_token(token)?.ensure_admin()?;

    // Get the entity, make sure it exists and get all related tasks
    let entity = ctx
        .entities()
        .find_one_and_delete(doc! { "id": entity_id }, None)
        .await?
        .ok_or_else(|| ApiError::entity_not_found(&entity_id))?;

    // Delete all related tasks
    ctx.tasks()
        .delete_many(doc! { "id": { "$in": &entity.tasks } }, None)
        .await?;

    Ok(entity)
}

async fn add_task(req: AddTask, ctx: Context) -> ApiResult<Task> {
    ctx.validate_token(&req.token)?.ensure_admin()?;

    let id = req.entity_id;
    let task: Task = req.into();

    ctx.tasks().insert_one(&task, None).await?;

    if ctx
        .entities()
        .update_one(
            doc! { "id": id },
            doc! { "$push": { "tasks": task.id } },
            None,
        )
        .await?
        .modified_count
        == 0
    {
        Err(ApiError::entity_not_found(&id))
    } else {
        Ok(task)
    }
}

async fn del_task(req: DelTask, ctx: Context) -> ApiResult<Task> {
    let DelTask { task_id, token } = req;

    ctx.validate_token(token)?.ensure_admin()?;

    // Make sure this exists
    let task = ctx
        .tasks()
        .find_one_and_delete(doc! { "id": task_id }, None)
        .await?
        .ok_or_else(|| ApiError::task_not_found(&task_id))?;

    // Delete the task from the entity that holds it
    ctx.entities()
        .update_one(
            doc! { "id": task.entity },
            doc! { "tasks": { "$pull": task_id } },
            None,
        )
        .await?;

    Ok(task)
}

async fn update_setting(req: UpdateSetting, ctx: Context) -> ApiResult<User> {
    let UpdateSetting {
        token,
        event_filter,
    } = req;

    let user_id = ctx.validate_token(&token)?.id();

    let serialized =
        to_bson(&event_filter).map_err(|_| ApiError::bad_request("Invalid event filter"))?;

    ctx.users()
        .find_one_and_update(
            doc! { "id": user_id },
            doc! { "$set": { "event_filter": serialized } },
            None,
        )
        .await?
        .ok_or_else(|| ApiError::user_not_found(&user_id))
}

async fn get_entities(_: GetEntities, ctx: Context) -> ApiResult<Entities> {
    let (vtbs, groups) = try_join(
        async { ctx.entities().find(None, None).await?.try_collect().await },
        async { ctx.groups().find(None, None).await?.try_collect().await },
    )
    .await?;

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

async fn del_user(req: DelUser, ctx: Context) -> ApiResult<User> {
    let DelUser { password, user_id } = req;

    ctx.auth_password(password)?;
    ctx.find_user(&user_id).await?;
    ctx.users()
        .find_one_and_delete(doc! { "id": user_id }, None)
        .await?
        .ok_or_else(|| ApiError::user_not_found(&user_id))
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
