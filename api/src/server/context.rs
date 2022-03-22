use std::sync::Arc;

use mongodb::Collection;
use sg_core::models::{Entity, Group, Task, User};

use crate::server::{JWTContext, DB};

#[derive(Debug, Clone)]
/// Context being shared between handlers. This will be cloned every time a handler is called.
/// So all underlineing data should be wrapped in Arc or similar shared reference thingy.
pub struct Context {
    /// DB instance. Since DB is composed of [`Collection`](mongodb::Collection)s, cloning is cheap.
    pub db: DB,
    pub jwt: Arc<JWTContext>,
}

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
}
