use std::time::{Duration, SystemTime};

use color_eyre::{eyre::Context, Result};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::server::Config;

/// Our claims struct, it needs to derive `Serialize` and/or `Deserialize`
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    aud: String,
    exp: usize,
}

pub struct JWTContext {
    exp: usize,
    encode_key: EncodingKey,
    decode_key: DecodingKey,
    pub(crate) header: Header,
    pub(crate) val: Validation,
}

impl JWTContext {
    pub fn new(config: Config) -> Self {
        let bytes = config.bot_password.as_bytes();
        let encode_key = EncodingKey::from_secret(bytes);
        let decode_key = DecodingKey::from_secret(bytes);
        let exp: usize = (OffsetDateTime::now_utc() + config.session_timeout)
            .unix_timestamp()
            .try_into()
            .expect("Proper timestamp cannot be negative");

        Self {
            encode_key,
            decode_key,
            exp,
            val: Validation::default(),
            header: Header::default(),
        }
    }

    pub fn encode(&self, user_id: impl Into<String>) -> Result<String> {
        let claim = Claims {
            aud: user_id.into(),
            exp: self.exp,
        };
        encode(&self.header, &claim, &self.encode_key).wrap_err("Failed to encode JWT")
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
}

#[test]
fn test_jwt() {
    let user_id = "Test";

    let config = Config {
        bot_password: "Secret".to_string(),
        session_timeout: Duration::from_secs(1),
        ..Default::default()
    };

    let mut jwt = JWTContext::new(config);

    jwt.val.leeway = 0;

    let token = jwt.encode(user_id).unwrap();
    println!("{}", token);

    // Valid and not expired
    assert!(jwt.validate(&token, user_id).unwrap());

    std::thread::sleep(Duration::from_secs(2));

    // Valid but expired
    assert!(jwt.validate(&token, user_id).is_err());
}
