//! Blocking version of the client.

use reqwest::{IntoUrl, Url};
use serde::{de::DeserializeOwned, Serialize};

use crate::{
    client::{Result, Shim},
    rpc::{ApiResult, Request, ResponseObject},
};

/// Blocking version of the client to invoke API methods.
#[derive(Clone, Debug)]
pub struct Client {
    client: reqwest::blocking::Client,
    url: Url,
    token: Option<String>,
}

impl Client {
    /// Creates new client instance.
    ///
    /// Note that URL should comes with api version in path and a trailing
    /// slash.
    ///
    /// # Errors
    /// Fails on invalid URL.
    pub fn new(url: impl IntoUrl) -> Result<Self> {
        Self::with_client(reqwest::blocking::Client::new(), url)
    }

    /// Creates new client instance with given reqwest blocking client.
    ///
    /// Note that URL should comes with api version in path and a trailing
    /// slash.
    ///
    /// # Errors
    /// Fails on invalid URL.
    pub fn with_client(client: reqwest::blocking::Client, url: impl IntoUrl) -> Result<Self> {
        Ok(Self {
            token: None,
            client,
            url: url.into_url()?,
        })
    }

    /// Invoke an RPC method.
    ///
    /// # Errors
    /// Fails on invalid `Request` method, bad request body, network issue or
    /// bad response.
    pub fn invoke<R>(&self, req: &R) -> Result<R::Res>
    where
        R: Request + Serialize,
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

        let resp: ApiResult<_> = req
            .send()?
            .json::<ResponseObject<Shim<R::Res>>>()?
            .data
            .into();

        Ok(resp?)
    }

    pub fn set_token(&mut self, token: impl Into<String>) -> Option<String> {
        self.token.replace(token.into())
    }

    #[must_use]
    pub fn token(&self) -> Option<&str> {
        self.token.as_deref()
    }

    /// Login and store the credential for future use.
    /// Returns `Some(Token)` if there's already one stored.
    ///
    /// # Errors
    /// Fails on invalid `Login` method, bad request body, network issue or bad
    /// response.
    pub fn login_and_store(
        &mut self,
        username: impl Into<String>,
        password: impl Into<String>,
    ) -> Result<Option<String>> {
        let token = self.login(username.into(), password.into())?;
        Ok(self.token.replace(token.token))
    }
}
