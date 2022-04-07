#![allow(clippy::unused_async)]

use std::{collections::HashSet, sync::Arc};

use crate::{
    model::{Health, Login, Null, UserQuery},
    rpc::{
        model::{
            AddEntity, AddTask, AddUser, AuthUser, Authorized, DelEntity, DelTask, DelUser,
            Entities, GetEntities, NewToken, Token, UpdateEntity, UpdateSetting,
        },
        ApiError, ApiResult,
    },
    server::{Config, Context, JWTContext, JWTGuard, Privilege, RouterExt},
};

use axum::{extract::Extension, Router};
use color_eyre::Result;
use futures::{future::try_join, TryStreamExt};
use http::Method;
use mongodb::{
    bson::{doc, to_bson, Uuid},
    options::{FindOneAndUpdateOptions, ReturnDocument},
};
use sg_auth::{Permission, PermissionSet};
use sg_core::models::{Entity, EventFilter, Task, User};
use tower_http::{
    cors,
    trace::{self, DefaultOnRequest},
};

/// Construct the router.
///
/// # Errors
/// Fails on invalid db url
pub async fn make_app(config: Config) -> Result<Router> {
    let config = Arc::new(config);

    let cors_layer = cors::CorsLayer::new()
        .allow_methods(vec![Method::POST])
        .allow_credentials(true)
        .allow_origin(cors::Any);
    let trace_layer = trace::TraceLayer::new_for_http();

    let jwt = Arc::new(JWTContext::new(&config));
    let user_guard = JWTGuard::new(jwt.clone(), Privilege::User).into_layer();
    let bot_guard = JWTGuard::new(jwt.clone(), Privilege::Bot).into_layer();
    let admin_guard = JWTGuard::new(jwt.clone(), Privilege::Admin).into_layer();

    let ctx = Context::new(jwt, config).await?;

    let app = Router::new()
        .mount(add_user)
        .mount(add_entity)
        .mount(add_task)
        .mount(del_entity)
        .mount(del_task)
        .mount(update_entity)
        .layer(admin_guard)
        .mount(get_entities)
        .mount(new_token)
        .mount(del_user)
        .layer(bot_guard)
        .mount(update_setting)
        .mount(auth_user)
        .layer(user_guard)
        .mount(health)
        .mount(login)
        .layer(Extension(ctx))
        .layer(cors_layer)
        .layer(trace_layer);

    Ok(app)
}

async fn health(_: Health, _: Context) -> ApiResult<Null> {
    Ok(Null)
}

async fn login(req: Login, ctx: Context) -> ApiResult<Token> {
    let prv = match ctx
        .auth()
        .look_up(req.username, req.password.as_bytes())
        .await?
    {
        PermissionSet { admin: Some(p), .. } if p == Permission::ReadWrite => Privilege::Admin,
        PermissionSet { api: Some(p), .. } if p == Permission::ReadWrite => Privilege::Bot,
        _ => return Err(ApiError::unauthorized()),
    };

    let (token, claims) = ctx.encode(&Uuid::from_bytes([0; 16]), prv)?;

    Ok(Token {
        token,
        valid_until: claims.valid_until(),
    })
}

async fn add_entity(req: AddEntity, ctx: Context) -> ApiResult<Entity> {
    let AddEntity { meta, tasks } = req;

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
    let UpdateEntity { entity_id, meta } = req;

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
    let DelEntity { entity_id } = req;

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
    let DelTask { task_id } = req;

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
    let UpdateSetting { event_filter } = req;
    let user_id = ctx.assert_user_claims()?.id();

    let serialized =
        to_bson(&event_filter).map_err(|_| ApiError::bad_request("Invalid event filter"))?;

    ctx.users()
        .find_one_and_update(
            doc! { "id": user_id },
            doc! { "$set": { "event_filter": serialized } },
            FindOneAndUpdateOptions::builder()
                .return_document(ReturnDocument::After)
                .build(),
        )
        .await?
        .ok_or_else(|| ApiError::user_not_found_with_id(&user_id))
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
        im_payload,
        avatar,

        name,
    } = req;

    let user = User {
        im,
        im_payload,
        avatar,
        name,
        is_admin: false,
        event_filter: EventFilter {
            entities: HashSet::default(),
            kinds: HashSet::default(),
        },
        id: Uuid::default(),
    };

    ctx.users().insert_one(&user, None).await?;

    Ok(user)
}

async fn del_user(req: DelUser, ctx: Context) -> ApiResult<User> {
    let DelUser { query } = req;

    ctx.del_user(&query).await
}

async fn auth_user(_: AuthUser, ctx: Context) -> ApiResult<Authorized> {
    let claims = ctx.assert_user_claims()?;
    let user = ctx
        .find_user(&UserQuery::ById {
            user_id: claims.id(),
        })
        .await?;

    Ok(Authorized {
        user,
        valid_until: claims.valid_until(),
    })
}

async fn new_token(req: NewToken, ctx: Context) -> ApiResult<Token> {
    let NewToken { query } = &req;

    let user = ctx.find_user(query).await?;

    let prv = if user.is_admin {
        Privilege::Admin
    } else {
        Privilege::User
    };

    let (token, claim) = ctx.encode(&user.id, prv)?;

    Ok(Token {
        token,
        valid_until: claim.valid_until(),
    })
}
