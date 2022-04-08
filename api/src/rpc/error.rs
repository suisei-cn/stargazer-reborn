use std::{
    error::Error as StdError,
    fmt::{Display, Formatter},
};

use http::StatusCode;
use mongodb::bson::Uuid;
use serde::{Deserialize, Serialize};

use crate::rpc::{Response, ResponseObject};

#[cfg_attr(
    feature = "server",
    doc = r##"
Represents an API Error. Implemented [`axum::response::IntoResponse`] trait.

# Examples

## Format into JSON
```rust
# use api::rpc::ApiError; fn main() {
let resp = r#"{"data":{"error":["Not Found","Cannot find user with ID `26721d57-37f5-458c-afea-2b18baf34925`"]},"success":false,"time":"2022-01-01T00:00:00.000000000Z"}"#;
let mut resp_obj = ApiError::user_not_found_with_id(
    &mongodb::bson::uuid::Uuid::parse_str("26721d57-37f5-458c-afea-2b18baf34925").unwrap(),
).packed();
# resp_obj.time = "2022-01-01T00:00:00.000000000Z".to_owned();
assert_eq!(resp, resp_obj.to_json());
# }
```

## Usage with `Axum`

Note: This requires feature `server`

```rust
# use api::rpc::ApiError; use http::StatusCode; fn main() {
use axum::response::IntoResponse;

let error = ApiError::new(StatusCode::BAD_REQUEST).explain("Invalid request");
let response = error.packed().into_response();
# }
```

[`IntoResponse`]: axum::response::IntoResponse
"##
)]
#[must_use]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiError {
    pub error: Vec<String>,
    #[serde(skip)]
    status: StatusCode,
}

impl Display for ApiError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Api Error: ")?;
        write!(f, "[Status {}]", self.status.as_str())?;

        self.error
            .iter()
            .map(String::as_str)
            .try_for_each(|e| write!(f, " {},", e))
    }
}

impl StdError for ApiError {}

impl ApiError {
    pub fn new(status: StatusCode) -> Self {
        let error = match status.canonical_reason() {
            Some(reason) => vec![reason.to_string()],
            None => vec![],
        };
        Self { error, status }
    }

    /// Push an explanatory error message to the error list.
    pub fn explain(mut self, error: impl Into<String>) -> Self {
        self.error.push(error.into());
        self
    }

    /// Throw multiple error explanation at once.
    pub fn tirade<I, Item>(mut self, error: I) -> Self
    where
        Item: Into<String>,
        I: IntoIterator<Item = Item>,
    {
        self.error.extend(error.into_iter().map(Into::into));
        self
    }

    #[must_use]
    pub const fn status(&self) -> StatusCode {
        self.status
    }

    pub fn packed(self) -> ResponseObject<Self> {
        ResponseObject::error(self)
    }

    pub fn bad_token() -> Self {
        Self::new(StatusCode::BAD_REQUEST).explain("Token is either expired or in bad shape")
    }

    pub fn missing_token() -> Self {
        Self::new(StatusCode::UNAUTHORIZED).explain("Token is missing")
    }

    pub fn unauthorized() -> Self {
        Self::new(StatusCode::UNAUTHORIZED).explain("Not permitted to access")
    }

    pub fn user_not_found_with_id(user_id: &Uuid) -> Self {
        Self::new(StatusCode::NOT_FOUND).explain(format!("Cannot find user with ID `{}`", user_id))
    }

    pub fn user_not_found_with_im(im: impl AsRef<str>, im_payload: impl AsRef<str>) -> Self {
        Self::new(StatusCode::NOT_FOUND).explain(format!(
            "Cannot find user with im `{}` and im_payload `{}`",
            im.as_ref(),
            im_payload.as_ref()
        ))
    }

    pub fn entity_not_found(entity_id: &Uuid) -> Self {
        Self::new(StatusCode::NOT_FOUND)
            .explain(format!("Cannot find entity with ID `{}`", entity_id))
    }

    pub fn task_not_found(task_id: &Uuid) -> Self {
        Self::new(StatusCode::NOT_FOUND).explain(format!("Cannot find task with ID `{}`", task_id))
    }

    pub fn bad_request(error: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST).explain(error)
    }

    pub fn internal() -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR)
    }
}

impl Response for ApiError {
    fn status(&self) -> StatusCode {
        self.status
    }
}

pub type ApiResult<T> = Result<T, ApiError>;
