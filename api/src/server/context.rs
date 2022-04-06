//! Context of the server. Contains the configuration and database handle.
use std::sync::Arc;

use color_eyre::Result;
use mongodb::{
    bson::{doc, Document, Uuid},
    options::{FindOneAndUpdateOptions, ReturnDocument},
    Client, Collection, Database,
};
use sg_core::models::{Entity, Group, Task, User};

use crate::{
    model::{Bot, UserQuery},
    rpc::{ApiError, ApiResult, UserExt},
    server::{config::Config, Claims, JWTContext},
};

/// Context being shared between handlers. This will be cloned every time a handler is called.
/// So all underlineing data should be wrapped in Arc or similar shared reference thingy.
#[must_use]
#[derive(Debug, Clone)]
pub struct Context {
    /// DB instance. Since DB is composed of [`Collection`](mongodb::Collection)s, cloning is cheap.
    pub(crate) db: Database,
    /// JWT context, used to decode, encode and validate JWT tokens.
    pub(crate) jwt: Arc<JWTContext>,
    /// Config.
    pub(crate) config: Arc<Config>,
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

    pub fn new_with_db(db: Database, jwt: Arc<JWTContext>, config: Arc<Config>) -> Self {
        Self { db, jwt, config }
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
    pub fn bots(&self) -> Collection<Bot> {
        self.db.collection(&self.config.bots_collection)
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
    /// Fail on incorrect password
    pub fn auth_password(&self, password: impl AsRef<str>) -> Result<(), ApiError> {
        if password.as_ref() == self.config.bot_password {
            Ok(())
        } else {
            Err(ApiError::wrong_password())
        }
    }

    /// Validate a JWT token. This does not check for privilege.
    /// To do so, use [`Claims::ensure_admin`](Claims::ensure_admin) or [`Claims::ensure_bot`](Claims::ensure_bot).
    ///
    /// # Errors
    /// Fails on invalid or expired token.
    pub fn validate_token(&self, token: impl AsRef<str>) -> ApiResult<Claims> {
        self.jwt.validate(token).map_err(|_| ApiError::bad_token())
    }

    /// # Errors
    /// Fail on bad token, database error or user not exist.
    pub async fn find_and_assert_token(&self, token: impl AsRef<str> + Send) -> ApiResult<User> {
        let user_id = self.validate_token(&token)?.id();
        self.find_user(&UserQuery::ById { id: user_id }).await
    }

    /// # Errors
    /// Fail on bad token, database error, user not exist or user is not admin.
    pub async fn find_and_assert_admin(&self, token: impl AsRef<str> + Send) -> ApiResult<User> {
        self.find_and_assert_token(token).await?.assert_admin()
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
