use std::{
    fmt::Debug,
    time::{Duration, SystemTime},
};

use color_eyre::{eyre::Context, Result};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, TokenData, Validation};
use serde::{Deserialize, Serialize};

use crate::server::Config;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
/// The JWT claim. Contains the user id and the expiry time.
pub struct Claims {
    /// Bytes representation of user id which can be decode and encoded into [`uuid::Uuid`].
    aud: [u8; 16],
    /// Expiration time represented in Unix timestamp.
    exp: usize,
}

#[derive(Debug, Clone)]
pub struct ValidateResult {
    pub valid: bool,
    pub claims: Claims,
}

impl Claims {
    /// This method will no validate exp since by default it has been validated by jsonwebtoken.
    pub fn validate(&self, user_id: &uuid::Uuid) -> bool {
        self.aud.as_bytes().eq(user_id.as_bytes())
    }

    /// The `exp` of the token in [`SystemTime`].
    pub fn valid_until(&self) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(self.exp as u64)
    }

    /// Expiration time of the token in Unix timestamp.
    pub fn exp(&self) -> usize {
        self.exp
    }
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
            timeout: config.token_timeout,
            val: Validation::default(),
            header: Header::default(),
        }
    }

    fn valid_until(&self) -> usize {
        (SystemTime::now() + self.timeout)
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs() as usize
    }

    pub fn encode(&self, user_id: &uuid::Uuid) -> Result<(String, Claims)> {
        let claim = Claims {
            aud: *user_id.as_bytes(),
            exp: self.valid_until(),
        };
        let token = jsonwebtoken::encode(&self.header, &claim, &self.encode_key)
            .wrap_err("Failed to encode JWT")?;

        Ok((token, claim))
    }

    pub fn decode(
        &self,
        token: impl AsRef<str>,
    ) -> Result<TokenData<Claims>, jsonwebtoken::errors::Error> {
        jsonwebtoken::decode::<Claims>(token.as_ref(), &self.decode_key, &self.val)
    }

    pub fn validate(
        &self,
        token: impl AsRef<str>,
        user_id: &uuid::Uuid,
    ) -> jsonwebtoken::errors::Result<ValidateResult> {
        let claims = self.decode(token)?.claims;

        Ok(ValidateResult {
            valid: claims.validate(user_id),
            claims,
        })
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
    let user_id = "20bdc51a-a23e-4f38-bbff-739d2b8ded4d"
        .parse::<uuid::Uuid>()
        .unwrap();

    let config = Config {
        bot_password: "Secret".to_string(),
        token_timeout: Duration::from_secs(1),
        ..Default::default()
    };

    let mut jwt = JWTContext::new(&config);
    jwt.val.leeway = 0;

    println!("{:#?}", jwt);

    let (token, _) = jwt.encode(&user_id).unwrap();
    println!("{}", token);

    // Valid and not expired
    assert!(jwt.validate(&token, &user_id).unwrap().valid);

    std::thread::sleep(Duration::from_secs(2));

    // Valid but expired
    assert!(jwt.validate(&token, &user_id).is_err());
}
