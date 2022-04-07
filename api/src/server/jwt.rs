#![allow(clippy::missing_errors_doc)]
#![allow(clippy::use_self)]

use std::{
    fmt::Debug,
    sync::Arc,
    time::{Duration, SystemTime},
};

use axum::{body::BoxBody, http::Request, response::IntoResponse};
use jsonwebtoken::{
    errors::Result as JwtResult, DecodingKey, EncodingKey, Header, TokenData, Validation,
};
use mongodb::bson::Uuid;
use serde::{Deserialize, Serialize};
use tower_http::auth::{AuthorizeRequest, RequireAuthorizationLayer};

use crate::{
    rpc::ApiError,
    server::{Config, Context},
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
    exp: u64,
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
    pub const fn valid_until_timestamp(&self) -> u64 {
        self.exp
    }

    /// User id represented as [`Uuid`].
    #[must_use]
    pub const fn id(&self) -> Uuid {
        Uuid::from_bytes(self.aud)
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.aud
    }

    #[must_use]
    pub const fn into_bytes(self) -> [u8; 16] {
        self.aud
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
    // TODO: use pem instead of secret key to sign the token
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

    fn calculate_exp(&self) -> u64 {
        (SystemTime::now() + self.timeout)
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs()
    }

    /// Encode the user id and corresponding privilege into a JWT token.
    pub fn encode(&self, user_id: &Uuid, privilege: Privilege) -> JwtResult<(String, Claims)> {
        let claim = Claims {
            aud: user_id.bytes(),
            exp: self.calculate_exp(),
            prv: privilege,
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

/// A guard that can be used with [`tower_http::auth::RequireAuthorizationLayer`]
/// to garante the user is authorized and authenticated.
/// ( Privilege must be greater than `guard` )
#[derive(Clone)]
pub struct JWTGuard {
    pub(crate) jwt: Arc<JWTContext>,
    guard: Privilege,
}

impl JWTGuard {
    #[must_use]
    pub fn new(jwt: Arc<JWTContext>, guard: Privilege) -> Self {
        Self { jwt, guard }
    }

    #[must_use]
    pub fn into_layer(self) -> RequireAuthorizationLayer<Self> {
        RequireAuthorizationLayer::custom(self)
    }
}

impl<B> AuthorizeRequest<B> for JWTGuard
where
    B: Send + Sync + 'static,
{
    type ResponseBody = BoxBody;

    fn authorize(
        &mut self,
        request: &mut Request<B>,
    ) -> std::result::Result<(), http::Response<Self::ResponseBody>> {
        tracing::debug!(method = ?request.uri().path(), "Authorizing request");
        let token = request
            .headers()
            .get(http::header::AUTHORIZATION)
            .ok_or_else(|| ApiError::missing_token().into_response())?
            .to_str()
            .map_err(|_| {
                ApiError::bad_request("Invalid header authentication encoding").into_response()
            })?
            .strip_prefix("Bearer ")
            .ok_or_else(|| {
                ApiError::bad_request(
                    "Invalid authentication header, this should be in bearer token format",
                )
                .into_response()
            })?;

        let claims = self
            .jwt
            .validate(token)
            .map_err(|_| ApiError::bad_token().into_response())?;

        tracing::debug!(privilege = ?claims.prv, guard = ?self.guard);

        if self.guard > claims.prv {
            return Err(ApiError::unauthorized().into_response());
        }

        let _ = request
            .extensions_mut()
            .get_mut::<Context>()
            .expect("Context not set")
            .set_claims(claims);

        Ok(())
    }
}

#[test]
fn test_jwt() {
    let user_id = Uuid::parse_str("20bdc51a-a23e-4f38-bbff-739d2b8ded4d").unwrap();

    let config = Config {
        bot_password: "Secret".to_string(),
        token_timeout: Duration::from_secs(1),
        ..Config::default()
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
