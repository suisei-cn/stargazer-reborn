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
use futures::{future::BoxFuture, Future};
use http::StatusCode;
use serde::{de::DeserializeOwned, Serialize};

/// Marker trait to ensure handlers are in a good shape.
pub(crate) trait Method<Req: Request>: Send + Clone + 'static {
    fn invoke(self, ctx: Context, req: Req) -> BoxFuture<'static, ApiResult<Req::Res>>;
}

impl<Req, M, F> Method<Req> for M
where
    Req: Request,
    F: Future<Output = ApiResult<Req::Res>> + Send + 'static,
    M: Send + Clone + FnOnce(Req, Context) -> F + 'static,
{
    fn invoke(self, ctx: Context, req: Req) -> BoxFuture<'static, ApiResult<Req::Res>> {
        Box::pin(self(req, ctx))
    }
}

pub(crate) trait RouterExt {
    fn mount<M, R>(self, method: M) -> Self
    where
        M: Method<R>,
        R: DeserializeOwned + Request + Send + 'static,
        R::Res: Serialize;
}

impl RouterExt for Router<Body> {
    fn mount<M, R>(self, method: M) -> Self
    where
        M: Method<R>,
        R: DeserializeOwned + Request + Send + 'static,
        R::Res: Serialize,
    {
        let handler = move |Json(req): Json<R>, Extension(ctx): Extension<Context>| async {
            match method.invoke(ctx, req).await {
                Ok(res) => res.packed().into_response(),
                Err(e) => e.packed().into_response(),
            }
        };

        self.route(&("/v1/".to_owned() + R::METHOD), post(handler))
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
        ApiError::internal_error()
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
