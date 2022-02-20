//! Models for the entity collection.
use mongodb::bson::{Document, Uuid};
use serde::{Deserialize, Serialize};

/// Entity for a vtuber.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    /// The unique identifier of the entity.
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
    pub id: Uuid,
    /// Type of the task.
    pub ty: String,
    /// Parameters of the task.
    pub params: Document,
}
