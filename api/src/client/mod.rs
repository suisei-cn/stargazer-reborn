use color_eyre::{eyre::Context, Result};
use reqwest::{IntoUrl, Url};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::rpc::{ApiError, ApiResult, Request, ResponseObject};

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum Shim<R> {
    Ok(R),
    Err(ApiError),
}

impl<T> From<Shim<T>> for ApiResult<T> {
    fn from(shim: Shim<T>) -> Self {
        match shim {
            Shim::Ok(res) => ApiResult::Ok(res),
            Shim::Err(err) => ApiResult::Err(err),
        }
    }
}

pub struct Client {
    client: reqwest::Client,
    url: Url,
}

impl Client {
    pub fn new(url: impl IntoUrl) -> Result<Self> {
        Ok(Self {
            client: reqwest::Client::new(),
            url: url.into_url()?,
        })
    }

    pub async fn invoke<R>(&self, req: R) -> Result<ApiResult<R::Res>>
    where
        R: Request + Serialize,
        R::Res: DeserializeOwned,
    {
        let res = self
            .client
            .post(self.url.clone())
            .body(req.packed().to_json())
            .header("Content-Type", "application/json")
            .send()
            .await
            .wrap_err("Failed to send request")?
            .text()
            .await
            .wrap_err("Failed to read response")?;

        tracing::info!(res = res.as_str());
        let serialized = serde_json::from_str::<ResponseObject<Shim<R::Res>>>(&res)
            .wrap_err("Failed to deserialize")
            .unwrap();

        Ok(serialized.data.into())
    }
}
