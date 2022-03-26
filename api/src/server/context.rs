//! Context of the server. Contains the configuration and database handle.
use std::sync::Arc;

use mongodb::{
    bson::{doc, Uuid},
    Collection,
};
use sg_core::models::{Entity, Group, Task, User};

use crate::{
    rpc::{ApiError, ApiResult, UserExt},
    server::{config::Config, Claims, JWTContext, DB},
};

#[derive(Debug, Clone)]
/// Context being shared between handlers. This will be cloned every time a handler is called.
/// So all underlineing data should be wrapped in Arc or similar shared reference thingy.
pub struct Context {
    /// DB instance. Since DB is composed of [`Collection`](mongodb::Collection)s, cloning is cheap.
    pub db: DB,
    /// JWT context, used to decode, encode and validate JWT tokens.
    pub jwt: Arc<JWTContext>,
    /// Config.
    pub config: Arc<Config>,
}

/// Context of the server. Contains the configuration and database handle.
impl Context {
    pub fn users(&self) -> &Collection<User> {
        &self.db.users
    }

    pub fn tasks(&self) -> &Collection<Task> {
        &self.db.tasks
    }

    pub fn entities(&self) -> &Collection<Entity> {
        &self.db.entities
    }

    pub fn groups(&self) -> &Collection<Group> {
        &self.db.groups
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
