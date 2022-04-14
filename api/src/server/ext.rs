use crate::{
    rpc::{ApiError, ApiResult, Request, Response},
    server::Context,
};

use axum::{
    body::{self, Body, Full},
    extract::{Extension, Json},
    response::Response as AxumResponse,
    routing::{post, Router},
};
use futures::Future;
use http::{header, HeaderValue};
use serde::{de::DeserializeOwned, Serialize};

/// Marker trait to ensure handlers are in a good shape.
pub trait Method<Req: Request, F: Future<Output = ApiResult<Req::Res>>> {
    fn invoke(self, ctx: Context, req: Req) -> F;
}

impl<Req, Func, Fut> Method<Req, Fut> for Func
where
    Req: Request,
    Fut: Future<Output = ApiResult<Req::Res>>,
    Func: FnOnce(Req, Context) -> Fut,
{
    fn invoke(self, ctx: Context, req: Req) -> Fut {
        self(req, ctx)
    }
}

pub trait RouterExt {
    #[must_use]
    fn mount<M, Req, Fut>(self, method: M) -> Self
    where
        M: Method<Req, Fut> + Send + Clone + 'static,
        Fut: Future<Output = ApiResult<Req::Res>> + Send,
        Req: DeserializeOwned + Request + Send + 'static,
        Req::Res: Serialize;
}

impl RouterExt for Router<Body> {
    fn mount<M, R, F>(self, method: M) -> Self
    where
        M: Method<R, F> + Send + Clone + 'static,
        F: Future<Output = ApiResult<R::Res>> + Send,
        R: DeserializeOwned + Request + Send + 'static,
        R::Res: Serialize,
    {
        let handler = move |Json(req): Json<R>, Extension(ctx): Extension<Context>| async {
            match method.invoke(ctx, req).await {
                Ok(res) => res.as_response(),
                Err(e) => e.as_response(),
            }
        };

        self.route(&("/".to_owned() + R::METHOD), post(handler))
    }
}

impl From<jsonwebtoken::errors::Error> for ApiError {
    fn from(e: jsonwebtoken::errors::Error) -> Self {
        tracing::warn!("{}", e);
        Self::bad_token()
    }
}

impl From<mongodb::error::Error> for ApiError {
    fn from(detail: mongodb::error::Error) -> Self {
        tracing::error!(?detail, "Mongo error");
        Self::internal()
    }
}

impl From<sg_auth::Error> for ApiError {
    fn from(err: sg_auth::Error) -> Self {
        use sg_auth::Error::{Argon, Bson, Mongo};

        match err {
            Mongo(e) => e.into(),
            Argon(detail) => {
                tracing::error!(?detail, "Argon error");
                Self::internal()
            }
            Bson(detail) => {
                tracing::error!(?detail, "Bson error");
                Self::internal()
            }
        }
    }
}

pub trait ResponseExt: Response + Serialize {
    fn as_response(&self) -> AxumResponse;
}

impl<R: Response + Serialize> ResponseExt for R {
    fn as_response(&self) -> AxumResponse {
        AxumResponse::builder()
            .status(self.status())
            .header(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            )
            .body(body::boxed(Full::from(self.packed().to_json_bytes())))
            .expect("Status and header should be statically known and not having any parsing issue")
    }
}
