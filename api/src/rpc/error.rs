use std::{
    error::Error as StdError,
    fmt::{Display, Formatter},
};

use http::StatusCode;
use mongodb::bson::Uuid;
use serde::{Deserialize, Serialize};

use crate::{model::UserQuery, rpc::Response};

#[cfg_attr(
    feature = "server",
    doc = r##"
Represents an API Error.

# Examples

## Format into JSON
```rust
# use api::{rpc::{ApiError,Response}, server::ResponseExt}; fn main() {
let resp = r#"{"data":{"error":["Not Found","Cannot find user with ID `26721d57-37f5-458c-afea-2b18baf34925`"]},"success":false,"time":"2022-01-01T00:00:00.000000000Z"}"#;
let mut resp_obj = ApiError::user_not_found_with_id(
    &mongodb::bson::uuid::Uuid::parse_str("26721d57-37f5-458c-afea-2b18baf34925").unwrap(),
).into_packed();
# resp_obj.time = "2022-01-01T00:00:00.000000000Z".to_owned();
assert_eq!(resp, resp_obj.to_json());
# }
```
"##
)]
#[must_use]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiError {
    error: Vec<String>,
    #[serde(skip)]
    status: StatusCode,
}

impl Display for ApiError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Api Error")?;
        write!(f, "({})", self.status.as_str())?;

        self.error.iter().try_for_each(|e| write!(f, " {},", e))
    }
}

impl StdError for ApiError {}

impl ApiError {
    #[inline]
    pub fn new(status: StatusCode) -> Self {
        let error = match status.canonical_reason() {
            Some(reason) => vec![reason.to_owned()],
            None => vec![],
        };
        Self { error, status }
    }

    #[must_use]
    #[inline]
    #[allow(clippy::missing_const_for_fn)]
    pub fn into_errors(self) -> Vec<String> {
        self.error
    }

    #[inline]
    #[must_use]
    pub fn errors(&self) -> &[String] {
        &self.error
    }

    /// Returns the canonical reason for error status.
    ///
    /// Returns `None` if the error is not a standard HTTP status.
    #[inline]
    #[must_use]
    pub fn error_reason(&self) -> Option<&'static str> {
        self.status.canonical_reason()
    }

    #[inline]
    #[must_use]
    pub const fn status(&self) -> StatusCode {
        self.status
    }

    /// Match the text with the error reasons.
    ///
    /// Returns `true` if the text is a substring of any of the errors.
    #[must_use]
    pub fn matches(&self, status_text: &str) -> bool {
        self.errors().iter().any(|e| e.contains(status_text))
    }

    #[inline]
    #[must_use]
    pub fn matches_status(&self, status: StatusCode) -> bool {
        self.status == status
    }

    /// Push an explanatory error message to the error list.
    #[inline]
    pub fn explain(mut self, error: impl Into<String>) -> Self {
        self.error.push(error.into());
        self
    }

    /// Throw multiple error explanation at once.
    #[inline]
    pub fn tirade<I, S>(mut self, error: I) -> Self
    where
        S: Into<String>,
        I: IntoIterator<Item = S>,
    {
        self.error.extend(error.into_iter().map(Into::into));
        self
    }

    #[inline]
    pub fn bad_token() -> Self {
        Self::new(StatusCode::UNAUTHORIZED).explain("Token is either expired or in bad shape")
    }

    #[inline]
    pub fn missing_token() -> Self {
        Self::new(StatusCode::UNAUTHORIZED).explain("Token is missing")
    }

    #[inline]
    pub fn unauthorized() -> Self {
        Self::new(StatusCode::UNAUTHORIZED).explain("Not permitted to access")
    }

    #[inline]
    pub fn user_not_found_with_id(user_id: &Uuid) -> Self {
        Self::new(StatusCode::NOT_FOUND).explain(format!("Cannot find user with ID `{}`", user_id))
    }

    #[inline]
    pub fn user_not_found_with_im(im: impl AsRef<str>, im_payload: impl AsRef<str>) -> Self {
        Self::new(StatusCode::NOT_FOUND).explain(format!(
            "Cannot find user with im `{}` and im_payload `{}`",
            im.as_ref(),
            im_payload.as_ref()
        ))
    }

    #[inline]
    pub fn user_not_found_with_query(query: &UserQuery) -> Self {
        match query {
            UserQuery::ById { user_id } => Self::user_not_found_with_id(user_id),
            UserQuery::ByIm { im, im_payload } => Self::user_not_found_with_im(im, im_payload),
        }
    }

    #[inline]
    pub fn user_already_exists(im: impl AsRef<str>, im_payload: impl AsRef<str>) -> Self {
        Self::new(StatusCode::CONFLICT).explain(format!(
            "User already exists im `{}` and im_payload `{}`",
            im.as_ref(),
            im_payload.as_ref()
        ))
    }

    #[inline]
    pub fn entity_not_found(entity_id: &Uuid) -> Self {
        Self::new(StatusCode::NOT_FOUND)
            .explain(format!("Cannot find entity with ID `{}`", entity_id))
    }

    #[inline]
    pub fn task_not_found(task_id: &Uuid) -> Self {
        Self::new(StatusCode::NOT_FOUND).explain(format!("Cannot find task with ID `{}`", task_id))
    }

    #[inline]
    pub fn bad_request(error: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST).explain(error)
    }

    #[inline]
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
