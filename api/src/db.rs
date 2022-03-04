//! Database access.

use color_eyre::Result;
use mongodb::{Client, Collection};

use sg_core::models::InDB;

use crate::get_config;

/// Database instance.
pub struct DB {
    _collection: Collection<InDB<i32>>,
}

impl DB {
    /// Create a new DB instance.
    ///
    /// # Errors
    /// Returns an error if the database connection fails.
    pub async fn new() -> Result<DB> {
        let config = get_config();
        let client = Client::with_uri_str(&config.mongo_uri).await?;
        let db = client.database(&config.mongo_db);
        let collection = db.collection(&config.mongo_collection);

        Ok(Self {
            _collection: collection,
        })
    }

    pub async fn new_session() -> Result<()> {
        Ok(())
    }

    pub async fn lookup_session(_session_id: String) -> Result<()> {
        Ok(())
    }
}
