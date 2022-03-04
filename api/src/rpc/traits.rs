use color_eyre::{eyre::Context, Result};
use serde::{de::DeserializeOwned, Serialize};

use crate::ResponseObject;

pub trait Request: DeserializeOwned {
    const METHOD: &'static str;
    type Res: Response;

    fn from_json(json: &str) -> Result<Self> {
        serde_json::from_str(json).wrap_err("Failed to parse json")
    }
}

pub trait Response: Serialize + Sized {
    fn is_successful(&self) -> bool;
    fn into_json(self) -> serde_json::error::Result<String> {
        let success = self.is_successful();
        ResponseObject::new(self, success).to_json()
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
