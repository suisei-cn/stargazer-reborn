#![allow(clippy::unused_async)]

use std::sync::Arc;

use axum::{extract::Extension, Router};
use color_eyre::Result;
use http::Method;
use mongodb::{bson::Uuid, Database};
use tower_http::{cors, trace};

use sg_auth::{Permission, PermissionSet};

use crate::{
    model::{GetInterest, Health, Interest, Login, Null, UserQuery},
    rpc::{
        ApiError,
        ApiResult, model::{
            AddEntity, AddTask, AddUser, Authorized, AuthUser, DelEntity, DelTask, DelUser,
            GetEntities, NewToken, Token, UpdateEntity, UpdateSetting,
        },
    },
    server::{Config, Context, JWTContext, JWTGuard, Privilege, RouterExt},
};

/// Construct the router.
///
/// # Errors
/// Fails on invalid db url
pub async fn make_app(config: Config) -> Result<Router> {
    make_app_with(config, None).await
}

/// Construct the router with given database.
///
/// # Errors
/// Fails on invalid db url
pub async fn make_app_with(config: Config, db: Option<Database>) -> Result<Router> {
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

    let ctx = match db {
        Some(db) => Context::new_with_db(db, jwt, config),
        None => Context::new(jwt, config).await?,
    };

    let api = Router::new()
        .mount(
            |AddUser {
                 im,
                 im_payload,
                 avatar,
                 name,
             },
             ctx: Context| {
                async move { ctx.add_user(im, im_payload, avatar, name).await }
            },
        )
        .mount(|AddEntity { meta, tasks }, ctx: Context| async move {
            ctx.add_entity(meta, tasks).await
        })
        .mount(|req: AddTask, ctx: Context| async move {
            let id = req.entity_id;
            ctx.add_task(&id, req.into()).await
        })
        .mount(
            |DelEntity { entity_id }, ctx: Context| async move { ctx.del_entity(&entity_id).await },
        )
        .mount(|DelTask { task_id }, ctx: Context| async move { ctx.del_task(&task_id).await })
        .mount(
            |UpdateEntity { entity_id, meta }, ctx: Context| async move {
                ctx.update_entity(&entity_id, &meta).await
            },
        )
        .layer(admin_guard)
        .mount(
            |GetInterest {
                 entity_id,
                 kind,
                 im,
             },
             ctx: Context| async move {
                ctx.get_interest(entity_id, &kind, &im)
                    .await
                    .map(|users| Interest { users })
            },
        )
        .mount(|GetEntities {}, ctx: Context| async move { ctx.get_entities().await })
        .mount(new_token)
        .mount(|DelUser { query }, ctx: Context| async move { ctx.del_user(&query).await })
        .layer(bot_guard)
        .mount(|UpdateSetting { event_filter }, ctx: Context| async move {
            let id = ctx.assert_user_claims()?.id();
            ctx.update_setting(&id, &event_filter).await
        })
        .mount(auth_user)
        .layer(user_guard)
        .mount(|Health {}, _| async { Ok(Null) })
        .mount(login)
        .layer(Extension(ctx))
        .layer(cors_layer)
        .layer(trace_layer);

    Ok(Router::new().nest("/v1", api))
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
