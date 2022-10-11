//! Database access.

use std::collections::HashMap;

use eyre::Result;
use futures_util::StreamExt;
use mongodb::{
    bson,
    bson::oid::ObjectId,
    change_stream::event::OperationType,
    options::{ChangeStreamOptions, FullDocumentType},
    Client,
    Collection,
};
use sg_core::models::{InDB, Task};
use tracing::{error, info};
use uuid::Uuid;

use crate::{App, Config};

/// Database instance.
pub struct DB {
    app: App,
    collection: Collection<InDB<Task>>,
    oid_map: HashMap<ObjectId, Uuid>,
}

impl DB {
    /// Create a new DB instance.
    ///
    /// # Errors
    /// Returns an error if the database connection fails.
    pub async fn new(app: App, config: Config) -> Result<Self> {
        let client = Client::with_uri_str(config.mongo_uri).await?;
        let db = client.database(&config.mongo_db);
        let collection = db.collection(&config.mongo_collection);

        Ok(Self {
            app,
            collection,
            oid_map: HashMap::new(),
        })
    }

    /// Import all tasks from the database.
    ///
    /// # Errors
    /// Returns an error if the database query fails.
    pub async fn init_tasks(&mut self) -> Result<()> {
        let mut count = 0;
        let mut tasks = self.collection.find(None, None).await?;

        while let Some(task) = tasks.next().await {
            let task = task?;

            self.oid_map.insert(task.id(), task.id.into());
            self.app.add_task(task.inner()).await;

            count += 1;
        }

        info!("{} task(s) loaded from database", count);
        Ok(())
    }

    /// Watch for changes in the database, and add/remove tasks as necessary.
    ///
    /// # Errors
    /// Returns an error if the database query fails.
    pub async fn watch_tasks(&mut self) -> Result<()> {
        let mut changes = self
            .collection
            .watch(
                None,
                ChangeStreamOptions::builder()
                    .full_document(Some(FullDocumentType::UpdateLookup))
                    .build(),
            )
            .await?;

        info!("Watching database for task changes");

        while let Some(event) = changes.next().await {
            let event = event?;
            match event.operation_type {
                OperationType::Insert => {
                    let task = event
                        .full_document
                        .expect("Full document must be available");

                    info!(task_id = %task.id, "Task added");

                    self.oid_map.insert(task.id(), task.id.into());
                    self.app.add_task(task.inner()).await;
                }
                OperationType::Update => {
                    let task = event
                        .full_document
                        .expect("Full document must be available");

                    info!(task_id = %task.id, "Task updated");

                    self.app.remove_task(task.id.into()).await;
                    self.app.add_task(task.inner()).await;
                }
                OperationType::Replace => {
                    let task = event
                        .full_document
                        .expect("Full document must be available");

                    info!(task_id = %task.id, "Task updated");

                    self.app.remove_task(task.id.into()).await;
                    self.app.add_task(task.inner()).await;
                }
                OperationType::Delete => {
                    let task: InDB<()> = bson::from_document(
                        event.document_key.expect("DocumentKey must be available"),
                    )
                    .expect("_id must be available");

                    if let Some(id) = self.oid_map.remove(&task.id()) {
                        info!(task_id = %id, "Task removed");

                        self.app.remove_task(id).await;
                    } else {
                        error!("Task not found in oid map: {:?}.", task.id());
                    }
                }
                OperationType::Invalidate => {
                    error!("Change stream invalidated.");
                }
                ty => {
                    error!("Unexpected event type: {:?}", ty);
                }
            }
        }

        Ok(())
    }
}
