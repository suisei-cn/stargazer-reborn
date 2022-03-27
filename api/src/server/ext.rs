use crate::rpc::{ApiError, Response, ResponseObject};
use axum::{http::StatusCode, response::Response as AxumResponse, Json};

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

impl<R: Response> axum::response::IntoResponse for ResponseObject<R> {
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
