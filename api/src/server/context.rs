//! Context of the server. Contains the configuration and database handle.
use std::sync::Arc;

use color_eyre::Result;
use mongodb::{
    bson::{doc, Uuid},
    Client, Collection, Database,
};
use sg_core::models::{Entity, Group, Task, User};

use crate::{
    rpc::{ApiError, ApiResult, UserExt},
    server::{config::Config, Claims, JWTContext},
};

#[derive(Debug, Clone)]
/// Context being shared between handlers. This will be cloned every time a handler is called.
/// So all underlineing data should be wrapped in Arc or similar shared reference thingy.
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
    pub async fn new(jwt: Arc<JWTContext>, config: Arc<Config>) -> Result<Self> {
        let client = Client::with_uri_str(&config.mongo_uri).await?;
        let db = client.database(&config.mongo_db);

        Ok(Self { db, jwt, config })
    }
    pub fn users(&self) -> Collection<User> {
        self.db.collection(&self.config.users_collection)
    }

    pub fn tasks(&self) -> Collection<Task> {
        self.db.collection(&self.config.tasks_collection)
    }

    pub fn entities(&self) -> Collection<Entity> {
        self.db.collection(&self.config.entities_collection)
    }

    pub fn groups(&self) -> Collection<Group> {
        self.db.collection(&self.config.groups_collection)
    }

    pub async fn find_user(&self, id: &Uuid) -> Result<User, ApiError> {
        self.users()
            .find_one(doc! { "id": id }, None)
            .await?
            .ok_or_else(|| ApiError::user_not_found(id))
    }

    pub async fn find_entity(&self, id: &Uuid) -> Result<Entity, ApiError> {
        self.entities()
            .find_one(doc! { "id": id }, None)
            .await?
            .ok_or_else(|| ApiError::entity_not_found(id))
    }

    pub fn auth_password(&self, password: impl AsRef<str>) -> Result<(), ApiError> {
        if password.as_ref() != self.config.bot_password {
            Err(ApiError::wrong_password())
        } else {
            Ok(())
        }
    }

    pub fn validate_token(&self, token: impl AsRef<str>) -> ApiResult<Claims> {
        self.jwt.validate(token).map_err(|_| ApiError::bad_token())
    }

    pub async fn find_and_assert_token(&self, token: impl AsRef<str>) -> ApiResult<User> {
        let user_id = self.validate_token(&token)?.id();
        self.find_user(&user_id).await
    }

    pub async fn find_and_assert_admin(&self, token: impl AsRef<str>) -> ApiResult<User> {
        self.find_and_assert_token(token).await?.assert_admin()
    }
}
