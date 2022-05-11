#![allow(clippy::unused_async)]

use std::{collections::HashSet, sync::Arc};

use crate::{
    model::{GetInterest, Health, Interest, Login, Null, UserQuery},
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
    bson::{doc, to_document, Uuid},
    options::{FindOneAndUpdateOptions, ReturnDocument},
};
use sg_auth::{Permission, PermissionSet};
use sg_core::models::{Entity, EventFilter, Task, User};
use tower_http::{cors, trace};

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

    let api = Router::new()
        .mount(add_user)
        .mount(add_entity)
        .mount(add_task)
        .mount(del_entity)
        .mount(del_task)
        .mount(update_entity)
        .layer(admin_guard)
        .mount(get_interest)
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

    Ok(Router::new().nest("/v1", api))
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

    ent.tasks = ctx
        .add_tasks(&ent.id, tasks.into_iter())
        .await?
        .into_iter()
        .map(|x| x.id)
        .collect();

    Ok(ent)
}

async fn update_entity(req: UpdateEntity, ctx: Context) -> ApiResult<Entity> {
    let UpdateEntity { entity_id, meta } = &req;

    ctx.update_entity(entity_id, meta).await
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
    let task = req.into();

    ctx.add_task(&id, task).await
}

async fn del_task(req: DelTask, ctx: Context) -> ApiResult<Task> {
    let DelTask { task_id } = req;

    ctx.del_task(&task_id).await
}

async fn update_setting(req: UpdateSetting, ctx: Context) -> ApiResult<User> {
    let UpdateSetting { event_filter } = req;
    let id = ctx.assert_user_claims()?.id();

    ctx.update_setting(&id, &event_filter).await
}

async fn get_entities(_: GetEntities, ctx: Context) -> ApiResult<Entities> {
    let (vtbs, groups) = try_join(
        async { ctx.entities().find(None, None).await?.try_collect().await },
        async { ctx.groups().find(None, None).await?.try_collect().await },
    )
    .await?;

    Ok(Entities { vtbs, groups })
}

async fn get_interest(req: GetInterest, ctx: Context) -> ApiResult<Interest> {
    let GetInterest {
        entity_id,
        kind,
        im,
    } = req;

    let users = ctx
        .users()
        .find(
            doc! {
              "event_filter.entities": entity_id,
              "event_filter.kinds": kind,
              "im": im,
            },
            None,
        )
        .await?
        .try_collect()
        .await?;

    Ok(Interest { users })
}

async fn add_user(req: AddUser, ctx: Context) -> ApiResult<User> {
    let AddUser {
        im,
        im_payload,
        avatar,
        name,
    } = req;

    if ctx
        .find_user(&UserQuery::ByIm {
            im: im.clone(),
            im_payload: im_payload.clone(),
        })
        .await?
        .is_some()
    {
        return Err(ApiError::user_already_exists(&im, &im_payload));
    };

    let user = User {
        im,
        im_payload,
        avatar,
        name,
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
        .await?
        .ok_or_else(|| ApiError::user_not_found_with_id(&claims.id()))?;

    Ok(Authorized {
        user,
        valid_until: claims.valid_until(),
    })
}

async fn new_token(req: NewToken, ctx: Context) -> ApiResult<Token> {
    let NewToken { query } = &req;

    let user = ctx
        .find_user(query)
        .await?
        .ok_or_else(|| ApiError::user_not_found_with_query(query))?;

    let (token, claim) = ctx.encode(&user.id, Privilege::User)?;

    Ok(Token {
        token,
        valid_until: claim.valid_until(),
    })
}
