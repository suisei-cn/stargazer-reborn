use reqwest::{IntoUrl, Url};
use serde::{de::DeserializeOwned, Serialize};

use crate::{
    client::{Result, Shim},
    rpc::{ApiResult, Request, ResponseObject},
};

#[derive(Clone, Debug)]
pub struct Client {
    client: reqwest::Client,
    url: Url,
    token: Option<String>,
}

impl Client {
    /// Creates new client instance.
    ///
    /// # Errors
    /// Fails on invalid URL.
    pub fn new(url: impl IntoUrl) -> Result<Self> {
        Ok(Self {
            client: reqwest::Client::new(),
            token: None,
            url: url.into_url()?,
        })
    }

    /// Invoke an RPC method.
    ///
    /// # Errors
    /// Fails on invalid `Request` method, bad request body, network issue or bad response.
    pub async fn invoke<R>(&self, req: R) -> Result<ApiResult<R::Res>>
    where
        R: Request + Serialize + Send + Sync,
        R::Res: DeserializeOwned,
    {
        let mut req = self
            .client
            .post(self.url.join(R::METHOD)?)
            .body(serde_json::to_vec(&req)?)
            .header("Content-Type", "application/json");

        if let Some(token) = &self.token {
            req = req.bearer_auth(token);
        }

        let resp = req
            .send()
            .await?
            .json::<ResponseObject<Shim<R::Res>>>()
            .await?
            .data
            .into();
        Ok(resp)
    }

    pub fn set_token(&mut self, token: impl Into<String>) {
        self.token = Some(token.into());
    }

    #[must_use]
    pub fn token(&self) -> Option<&str> {
        self.token.as_deref()
    }

    /// Login and store the credential for future use.
    ///
    /// # Errors
    /// Fails on invalid `Login` method, bad request body, network issue or bad response.
    pub async fn login_and_store(
        &mut self,
        username: impl Into<String> + Send,
        password: impl Into<String> + Send,
    ) -> Result<ApiResult<()>> {
        match self.login(username.into(), password.into()).await? {
            Ok(token) => {
                self.token.replace(token.token);
                Ok(Ok(()))
            }
            Err(err) => Ok(Err(err)),
        }
    }
}
