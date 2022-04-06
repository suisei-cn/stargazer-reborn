//! Context of the server. Contains the configuration and database handle.
use std::sync::Arc;

use color_eyre::Result;
use mongodb::{
    bson::{doc, Document, Uuid},
    options::{FindOneAndUpdateOptions, ReturnDocument},
    Client, Collection, Database,
};
use sg_auth::AuthClient;
use sg_core::models::{Entity, Group, Task, User};

use crate::{
    model::{Bot, UserQuery},
    rpc::{ApiError, ApiResult},
    server::{config::Config, Claims, JWTContext, Privilege},
};

/// Context being shared between handlers. This will be cloned every time a handler is called.
/// So all underlineing data should be wrapped in Arc or similar shared reference thingy.
///
/// Since this is intended to be cloned everytime, `Option<Claims>` will be reset upon every request.
#[must_use]
#[derive(Clone)]
pub struct Context {
    /// Config.
    pub(crate) config: Arc<Config>,
    /// JWT
    jwt: Arc<JWTContext>,
    /// DB instance. Since DB is composed of [`Collection`](mongodb::Collection)s, cloning is cheap.
    db: Database,
    /// Auth context.
    auth: AuthClient,
    /// Claims that are extracted from the JWT token header by auth middleware, optionally.
    claims: Option<Claims>,
}

/// Context of the server. Contains the configuration and database handle.

impl Context {
    /// # Errors
    /// Fail on invalid database url.
    pub async fn new(jwt: Arc<JWTContext>, config: Arc<Config>) -> Result<Self> {
        let client = Client::with_uri_str(&config.mongo_uri).await?;
        let db = client.database(&config.mongo_db);

        Ok(Self::new_with_db(db, jwt, config))
    }

    /// Construct self with preconnected database.
    pub fn new_with_db(db: Database, jwt: Arc<JWTContext>, config: Arc<Config>) -> Self {
        let auth = AuthClient::new(db.collection(&config.auth_collection));
        Self {
            db,
            jwt,
            auth,
            config,
            claims: None,
        }
    }

    /// Get the claims from the JWT token header and assert its validity.
    /// Only use this if trying to get user information from the token.
    ///
    /// # Errors
    /// Fails if the token is not present, or the token is not issued for a subscriber.
    pub fn assert_user_claims(&self) -> ApiResult<&Claims> {
        self.claims
            .as_ref()
            .ok_or_else(ApiError::unauthorized)
            .and_then(|c| {
                if c.as_bytes() == &[0; 16] {
                    Err(ApiError::unauthorized())
                } else {
                    Ok(c)
                }
            })
    }

    /// Get the claims from the JWT token header.
    #[must_use]
    pub const fn claims(&self) -> Option<&Claims> {
        self.claims.as_ref()
    }

    /// Insert claims, if there's already one, return it
    pub fn set_claims(&mut self, claims: Claims) -> Option<Claims> {
        self.claims.replace(claims)
    }

    /// Encode the user id and corresponding privilege into a JWT token.
    ///
    /// # Errors
    /// Fails when encoding failed. This is unlikely to happen, but if it does, it's a bug.
    pub fn encode(&self, user_id: &Uuid, privilege: Privilege) -> ApiResult<(String, Claims)> {
        self.jwt.encode(user_id, privilege).map_err(|e| {
            tracing::error!(e = ?e, "Failed to encode JWT token");
            ApiError::internal()
        })
    }

    #[must_use]
    pub fn users(&self) -> Collection<User> {
        self.db.collection(&self.config.users_collection)
    }

    #[must_use]
    pub fn tasks(&self) -> Collection<Task> {
        self.db.collection(&self.config.tasks_collection)
    }

    #[must_use]
    pub fn entities(&self) -> Collection<Entity> {
        self.db.collection(&self.config.entities_collection)
    }

    #[must_use]
    pub fn groups(&self) -> Collection<Group> {
        self.db.collection(&self.config.groups_collection)
    }

    #[must_use]
    pub fn auth_db(&self) -> Collection<Bot> {
        self.db.collection(&self.config.auth_collection)
    }

    #[must_use]
    pub const fn auth(&self) -> &AuthClient {
        &self.auth
    }

    /// # Errors
    /// Fail on database error or user not found
    pub async fn find_user(&self, query: &UserQuery) -> Result<User, ApiError> {
        self.users()
            .find_one(query.as_document(), None)
            .await?
            .ok_or_else(|| query.as_error())
    }

    /// # Errors
    /// Fail on database error or user not found
    pub async fn del_user(&self, query: &UserQuery) -> Result<User, ApiError> {
        self.users()
            .find_one_and_delete(query.as_document(), None)
            .await?
            .ok_or_else(|| query.as_error())
    }

    /// # Errors
    /// Fail on database error or user not found
    pub async fn update_user(&self, query: &UserQuery, update: Document) -> Result<User, ApiError> {
        self.users()
            .find_one_and_update(
                query.as_document(),
                update,
                FindOneAndUpdateOptions::builder()
                    .return_document(ReturnDocument::After)
                    .build(),
            )
            .await?
            .ok_or_else(|| query.as_error())
    }

    /// # Errors
    /// Fail on database error or entity not found
    pub async fn find_entity(&self, id: &Uuid) -> Result<Entity, ApiError> {
        self.entities()
            .find_one(doc! { "id": id }, None)
            .await?
            .ok_or_else(|| ApiError::entity_not_found(id))
    }

    /// # Errors
    /// Fail on bad token, database error, the uuid is "nil" or user not exist.
    ///
    /// An uuid is "nil" if all bytes are set to 0, which at here represents that
    /// the token is issued to an admin or bot, who does not represent a subscriber.
    pub async fn find_and_assert_claim(&self) -> ApiResult<User> {
        let user_id = self.assert_user_claims()?.id();
        if user_id.bytes() == [0; 16] {
            return Err(ApiError::unauthorized());
        }
        self.find_user(&UserQuery::ById { user_id }).await
    }
}

#[tokio::test]
async fn test_fetch_entity_from_db() {
    tracing_subscriber::fmt::init();
    let client = Client::with_uri_str("mongodb://192.168.1.53:27017")
        .await
        .unwrap();
    let db = client.database("stargazer-reborn");
    let col = db.collection::<Entity>("entities");

    let res = col
        .find_one(doc! { "meta.name.name": { "$gt": {} } }, None)
        .await
        .unwrap();
    tracing::info!(?res);
}

#[test]
fn test_bson_entity() {
    use isolanguage_1::LanguageCode;
    use mongodb::bson::{from_bson, to_bson};
    use sg_core::models::{Meta, Name};
    use std::collections::HashMap;

    let name = Name {
        name: HashMap::from_iter([(LanguageCode::En, "Test".to_owned())]),
        default_language: LanguageCode::En,
    };

    let meta = Meta { name, group: None };

    let ser = to_bson(&meta).unwrap();

    tracing::info!(ser = ?ser);

    let de = from_bson(ser).unwrap();

    assert_eq!(meta, de);
}
