use http::StatusCode;
use serde::{Deserialize, Serialize};

use crate::Response;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Null;

impl Response for Null {
    fn status(&self) -> StatusCode {
        StatusCode::NO_CONTENT
    }
}
