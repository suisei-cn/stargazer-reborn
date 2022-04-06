#![allow(clippy::missing_errors_doc)]

use std::{
    fmt::Debug,
    time::{Duration, SystemTime},
};

use jsonwebtoken::{
    errors::Result as JwtResult, DecodingKey, EncodingKey, Header, TokenData, Validation,
};
use mongodb::bson::Uuid;
use serde::{Deserialize, Serialize};

use crate::{
    rpc::{ApiError, ApiResult},
    server::Config,
};

/// Privilege of a token. Three levels: User, Bot, Admin.
///
/// - **User** can only access some API, mostly related to themselves.
/// - **Bot** can access more API, include creating session for users.
/// - **Admin** can access all API.
#[must_use]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Privilege {
    User,
    Bot,
    Admin,
}

#[must_use]
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
/// The JWT claim. Contains the user id and the expiry time.
pub struct Claims {
    /// Bytes representation of user id which can be decode and encoded into [`Uuid`].
    aud: [u8; 16],
    /// Expiration time represented in Unix timestamp.
    exp: usize,
    /// Privilege of this token
    prv: Privilege,
}

impl Claims {
    /// The `exp` of the token in [`SystemTime`].
    #[must_use]
    pub fn valid_until(&self) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(self.exp as u64)
    }

    /// Expiration time of the token in Unix timestamp.
    #[must_use]
    pub const fn valid_until_timestamp(&self) -> usize {
        self.exp
    }

    /// User id represented as [`Uuid`].
    #[must_use]
    pub const fn id(&self) -> Uuid {
        Uuid::from_bytes(self.aud)
    }

    /// Validate the user has admin privilege.
    pub fn ensure_admin(&self) -> ApiResult<()> {
        if self.prv == Privilege::Admin {
            Ok(())
        } else {
            Err(ApiError::unauthorized())
        }
    }

    /// Validate the user has bot privilege, which can be two cases:
    ///
    /// - The user is a bot
    /// - The user is an admin
    pub fn ensure_bot(&self) -> ApiResult<()> {
        if self.prv >= Privilege::Bot {
            Ok(())
        } else {
            Err(ApiError::unauthorized())
        }
    }
}

#[must_use]
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

    fn get_exp(&self) -> usize {
        (SystemTime::now() + self.timeout)
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs()
            .try_into()
            .expect("Time went too far, or you're running on a 16-bit system?")
    }

    /// Encode the user id and corresponding privilege into a JWT token.
    pub fn encode(&self, user_id: &Uuid, privilege: Privilege) -> JwtResult<(String, Claims)> {
        let claim = Claims {
            aud: user_id.bytes(),
            exp: self.get_exp(),
            prv: privilege,
        };
        let token = jsonwebtoken::encode(&self.header, &claim, &self.encode_key)?;
        Ok((token, claim))
    }

    /// Generate a token that is literally never gonna expire. Exp is set to [`usize::MAX`].
    /// The safety is garanted by the handler, which should validate the bot id is still in database.
    pub fn encode_bot_token(&self, user_id: &Uuid) -> JwtResult<(String, Claims)> {
        let claim = Claims {
            aud: user_id.bytes(),
            exp: usize::MAX,
            prv: Privilege::Bot,
        };
        let token = jsonwebtoken::encode(&self.header, &claim, &self.encode_key)?;
        Ok((token, claim))
    }

    /// Decode the token and validate the token is not expired, which is done automatically by [`jsonwebtoken`].
    pub fn decode(&self, token: impl AsRef<str>) -> JwtResult<TokenData<Claims>> {
        jsonwebtoken::decode::<Claims>(token.as_ref(), &self.decode_key, &self.val)
    }

    /// Helper fn wrap around [`JWTContext::decode`] that only returns the [`Claims`].
    pub fn validate(&self, token: impl AsRef<str>) -> JwtResult<Claims> {
        Ok(self.decode(token)?.claims)
    }
}

impl Debug for JWTContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JWTContext")
            .field("timeout", &self.timeout)
            .field("encode_key", &"[:REDACTED:]")
            .field("decode_key", &"[:REDACTED:]")
            .field("header", &self.header)
            .field("val", &self.val)
            .finish()
    }
}

#[test]
fn test_jwt() {
    let user_id = Uuid::parse_str("20bdc51a-a23e-4f38-bbff-739d2b8ded4d").unwrap();

    let config = Config {
        bot_password: "Secret".to_string(),
        token_timeout: Duration::from_secs(1),
        ..Default::default()
    };

    let mut jwt = JWTContext::new(&config);
    jwt.val.leeway = 0;

    println!("{:#?}", jwt);

    let (token, _) = jwt.encode(&user_id, Privilege::User).unwrap();
    println!("{}", token);

    // Valid and not expired
    let _ = jwt.validate(&token).unwrap();

    std::thread::sleep(Duration::from_secs(2));

    // Valid but expired
    assert!(jwt.validate(&token).is_err());
}

#[test]
fn test_privilege() {
    let admin = Privilege::Admin;
    let bot = Privilege::Bot;
    let user = Privilege::User;

    assert!(admin > bot);
    assert!(bot > user);
}
