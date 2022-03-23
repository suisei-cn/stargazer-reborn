//! Context of the server. Contains the configuration and database handle.
use std::sync::Arc;

use mongodb::{bson::doc, Collection};
use sg_core::models::{Entity, Group, Task, User};

use crate::{
    rpc::ApiError,
    server::{config::Config, JWTContext, DB},
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

    pub async fn find_user(&self, id: &uuid::Uuid) -> Result<User, ApiError> {
        self.users()
            .find_one(doc! { "id": &id }, None)
            .await?
            .ok_or_else(|| ApiError::user_not_found(id.to_string()))
    }

    pub fn auth_password(&self, password: impl AsRef<str>) -> Result<(), ApiError> {
        if password.as_ref() != self.config.bot_password {
            Err(ApiError::wrong_password())
        } else {
            Ok(())
        }
    }
}
