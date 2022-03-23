use axum::response::IntoResponse;
use color_eyre::{eyre::Context, Result};
use serde::{de::DeserializeOwned, Serialize};

use crate::rpc::{RequestObject, ResponseObject};

pub trait Request: DeserializeOwned {
    const METHOD: &'static str;
    type Res: Response;

    fn from_json(json: &str) -> Result<Self> {
        serde_json::from_str(json).wrap_err("Failed to parse json")
    }

    fn packed(self) -> RequestObject<Self> {
        RequestObject {
            method: Self::METHOD.to_owned(),
            params: self,
        }
    }
}

pub trait Response: Serialize + Sized {
    fn is_successful(&self) -> bool;

    fn packed(self) -> ResponseObject<Self> {
        let success = self.is_successful();
        ResponseObject::new(self, success)
    }

    fn into_axum_response(self) -> axum::response::Response {
        self.packed().into_response()
    }
}

pub trait Method<Req: Request> {
    fn handle(&self, req: Req) -> Result<Req::Res>;
}

impl<R: Request, M> Method<R> for M
where
    M: Fn(R) -> R::Res,
{
    fn handle(&self, req: R) -> Result<R::Res> {
        Ok(self(req))
    }
}
