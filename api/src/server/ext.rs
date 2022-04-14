use crate::{
    rpc::{ApiError, ApiResult, Request, Response, ResponseObject},
    server::Context,
};

use axum::{
    body::Body,
    extract::{Extension, Json},
    response::{IntoResponse, Response as AxumResponse},
    routing::{post, Router},
};
use futures::Future;
use http::StatusCode;
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
                Ok(res) => res.packed().into_response(),
                Err(e) => e.packed().into_response(),
            }
        };

        self.route(&("/".to_owned() + R::METHOD), post(handler))
    }
}

impl axum::response::IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        self.packed().into_response()
    }
}

impl From<jsonwebtoken::errors::Error> for ApiError {
    fn from(e: jsonwebtoken::errors::Error) -> Self {
        tracing::warn!("{}", e);
        Self::bad_token()
    }
}

impl From<mongodb::error::Error> for ApiError {
    fn from(err: mongodb::error::Error) -> Self {
        let err_str = err.to_string();
        tracing::error!(error = err_str.as_str(), "Mongo error");
        Self::internal()
    }
}

impl From<sg_auth::Error> for ApiError {
    fn from(err: sg_auth::Error) -> Self {
        use sg_auth::Error::{Argon, Bson, Mongo};

        match err {
            Mongo(e) => e.into(),
            Argon(e) => {
                tracing::error!(e = ?e, "Argon error");
                Self::internal()
            }
            Bson(e) => {
                tracing::error!(e = ?e, "Bson error");
                Self::internal()
            }
        }
    }
}

impl<R: Response> axum::response::IntoResponse for ResponseObject<R>
where
    R: Serialize,
{
    fn into_response(self) -> AxumResponse {
        let status = if self.success {
            StatusCode::OK
        } else {
            StatusCode::BAD_REQUEST
        };

        let mut body = Json(self).into_response();
        *body.status_mut() = status;
        body
    }
}
