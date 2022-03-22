use std::{fmt::Debug, result::Result as StdResult, time::Duration};

use color_eyre::{eyre::Context, Result};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::{rpc::ApiError, server::Config};

/// Our claims struct, it needs to derive `Serialize` and/or `Deserialize`
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    aud: String,
    exp: usize,
}

#[derive(Clone)]
pub struct JWTContext {
    timeout: Duration,
    encode_key: EncodingKey,
    decode_key: DecodingKey,
    pub(crate) header: Header,
    pub(crate) val: Validation,
}

impl JWTContext {
    pub fn new(config: &Config) -> Self {
        let bytes = config.bot_password.as_bytes();
        let encode_key = EncodingKey::from_secret(bytes);
        let decode_key = DecodingKey::from_secret(bytes);

        Self {
            encode_key,
            decode_key,
            timeout: config.session_timeout,
            val: Validation::default(),
            header: Header::default(),
        }
    }

    fn exp(&self) -> usize {
        (OffsetDateTime::now_utc() + self.timeout)
            .unix_timestamp()
            .try_into()
            .expect("Proper timestamp cannot be negative")
    }

    pub fn encode(&self, user_id: impl Into<String>) -> Result<String> {
        let claim = Claims {
            aud: user_id.into(),
            exp: self.exp(),
        };
        jsonwebtoken::encode(&self.header, &claim, &self.encode_key)
            .wrap_err("Failed to encode JWT")
    }

    pub fn validate(
        &self,
        token: &str,
        user_id: &str,
    ) -> std::result::Result<bool, jsonwebtoken::errors::Error> {
        let ret = jsonwebtoken::decode::<Claims>(token, &self.decode_key, &self.val)?
            .claims
            .aud
            .eq(user_id);

        Ok(ret)
    }

    pub fn api_validate(&self, token: &str, user_id: &str) -> StdResult<(), ApiError> {
        match self.validate(token, user_id) {
            Ok(true) => Ok(()),
            Ok(false) => Err(ApiError::bad_token()),
            Err(e) => Err(e.into()),
        }
    }
}

impl Debug for JWTContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JWTContext")
            .field("timeout", &self.timeout)
            .field("encode_key", &"*")
            .field("decode_key", &"*")
            .field("header", &self.header)
            .field("val", &self.val)
            .finish()
    }
}

#[test]
fn test_jwt() {
    let user_id = "Test";

    let config = Config {
        bot_password: "Secret".to_string(),
        session_timeout: Duration::from_secs(1),
        ..Default::default()
    };

    let mut jwt = JWTContext::new(&config);
    jwt.val.leeway = 0;

    println!("{:#?}", jwt);

    let token = jwt.encode(user_id).unwrap();
    println!("{}", token);

    // Valid and not expired
    assert!(jwt.validate(&token, user_id).unwrap());

    std::thread::sleep(Duration::from_secs(2));

    // Valid but expired
    assert!(jwt.validate(&token, user_id).is_err());
}
