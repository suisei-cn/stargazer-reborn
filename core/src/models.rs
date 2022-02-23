//! Models for the entity collection.
use mongodb::bson::Document;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Entity for a vtuber.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    /// The unique identifier of the entity.
    #[serde(with = "mongodb::bson::serde_helpers::uuid_as_binary")]
    pub id: Uuid,
    /// Metadata about the entity.
    pub meta: Meta,
    /// Tasks to be scheduled.
    pub tasks: Vec<Task>,
}

/// Meta of the vtuber.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Meta {
    /// Vtuber canonical name.
    pub name: String,
}

/// Defined task for a vtuber.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    /// Task id. Used for scheduling.
    #[serde(with = "mongodb::bson::serde_helpers::uuid_as_binary")]
    pub id: Uuid,
    /// Kind of the task.
    pub kind: String,
    /// Parameters of the task.
    pub params: Document,
}
