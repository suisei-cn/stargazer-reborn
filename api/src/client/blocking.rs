use reqwest::{IntoUrl, Url};
use serde::{de::DeserializeOwned, Serialize};

use crate::{
    client::{Result, Shim},
    rpc::{ApiResult, Request, ResponseObject},
};

#[derive(Clone, Debug)]
pub struct Client {
    client: reqwest::blocking::Client,
    url: Url,
}

impl Client {
    pub fn new(url: impl IntoUrl) -> Result<Self> {
        Ok(Self {
            client: reqwest::blocking::Client::new(),
            url: url.into_url()?,
        })
    }

    pub fn invoke<R>(&self, req: R) -> Result<ApiResult<R::Res>>
    where
        R: Request + Serialize,
        R::Res: DeserializeOwned,
    {
        Ok(self
            .client
            .post(self.url.join(R::METHOD)?)
            .body(serde_json::to_vec(&req)?)
            .header("Content-Type", "application/json")
            .send()?
            .json::<ResponseObject<Shim<R::Res>>>()?
            .data
            .into())
    }
}