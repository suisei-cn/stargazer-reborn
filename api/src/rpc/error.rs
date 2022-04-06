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
let resp = r#"{"data":{"error":["Cannot find user with ID `26721d57-37f5-458c-afea-2b18baf34925`"]},"success":false,"time":"2022-01-01T00:00:00.000000000Z"}"#;
let mut resp_obj = ApiError::user_not_found(
    &mongodb::bson::uuid::Uuid::parse_str("26721d57-37f5-458c-afea-2b18baf34925").unwrap(),
).packed();
# resp_obj.time = "2022-01-01T00:00:00.000000000Z".to_owned();
assert_eq!(resp, resp_obj.to_json());
# }
```

## Usage with `Axum`

Note: This requires feature `server`

```rust
# use api::rpc::ApiError; fn main() {
use axum::response::IntoResponse;

let error = ApiError::new(vec!["error1".to_string(), "error2".to_string()]);
let response = error.packed().into_response();
# }
```

[`IntoResponse`]: axum::response::IntoResponse
"##
)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiError {
    pub error: Vec<String>,
}

impl ApiError {
    #[must_use]
    pub fn new(error: Vec<String>) -> Self {
        Self { error }
    }

    pub fn packed(self) -> ResponseObject<Self> {
        ResponseObject::error(self)
    }

    #[must_use]
    pub fn bad_token() -> Self {
        Self::new(vec![
            "Bad token".to_owned(),
            "It's either expired or in bad shape".to_owned(),
        ])
    }

    #[must_use]
    pub fn unauthorized() -> Self {
        Self::new(vec![
            "Unauthorized".to_owned(),
            "Token is valid but does not have to sufficient privilege to access".to_owned(),
        ])
    }

    #[must_use]
    pub fn wrong_password() -> Self {
        Self::new(vec!["Wrong password".to_owned()])
    }

    #[must_use]
    pub fn user_not_found(user_id: &Uuid) -> Self {
        Self::new(vec![format!("Cannot find user with ID `{}`", user_id)])
    }

    pub fn user_not_found_from_im(im: impl AsRef<str>, im_payload: impl AsRef<str>) -> Self {
        Self::new(vec![format!(
            "Cannot find user with im `{}` and im_payload `{}`",
            im.as_ref(),
            im_payload.as_ref()
        )])
    }

    #[must_use]
    pub fn entity_not_found(entity_id: &Uuid) -> Self {
        Self::new(vec![format!("Cannot find entity with ID `{}`", entity_id)])
    }

    #[must_use]
    pub fn task_not_found(task_id: &Uuid) -> Self {
        Self::new(vec![format!("Cannot find task with ID `{}`", task_id)])
    }

    pub fn bad_request(error: impl Into<String>) -> Self {
        Self::new(vec!["Bad request".to_owned(), error.into()])
    }

    #[must_use]
    pub fn internal() -> Self {
        Self::new(vec!["Internal Error".to_owned()])
    }
}

impl Response for ApiError {
    fn is_successful(&self) -> bool {
        false
    }
}

pub type ApiResult<T> = Result<T, ApiError>;
