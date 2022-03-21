//! Database access.

use color_eyre::Result;
use mongodb::{Client, Collection};

use sg_core::models::{Entity, Group, Task, User};

use crate::server::Config;

#[derive(Debug, Clone)]
/// Database instance.
pub struct DB {
    pub(crate) users: Collection<User>,
    pub(crate) tasks: Collection<Task>,
    pub(crate) entities: Collection<Entity>,
    pub(crate) groups: Collection<Group>,
}

impl DB {
    /// Create a new DB instance.
    ///
    /// # Errors
    /// Returns an error if the database connection fails.
    pub async fn new(config: &Config) -> Result<DB> {
        let client = Client::with_uri_str(&config.mongo_uri).await?;
        let db = client.database(&config.mongo_db);
        let users = db.collection(&config.users_collection);
        let tasks = db.collection(&config.tasks_collection);
        let entities = db.collection(&config.entities_collection);
        let groups = db.collection(&config.groups_collection);

        Ok(Self {
            users,
            tasks,
            entities,
            groups,
        })
    }
}
