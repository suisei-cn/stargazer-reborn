//! Models for the entity collection.
use std::collections::{HashMap, HashSet};
use std::ops::{Deref, DerefMut};

use isolanguage_1::LanguageCode;
use mongodb::bson::oid::ObjectId;
use mongodb::bson::Uuid;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

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
    /// Vtuber's name in different languages. The key must be in ISO 639-1.
    pub name: HashMap<LanguageCode, String>,
    /// Preferred language of the vtuber. Must be in ISO 639-1.
    pub default_language: LanguageCode,
    /// Affiliation of the vtuber.
    pub group: Option<String>,
}

/// Defined task for a vtuber.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    /// The unique identifier of the task.
    pub id: Uuid,
    /// Parent entity id.
    pub entity: Uuid,
    /// Kind of the task.
    pub kind: String,
    /// Parameters of the task.
    pub params: Map<String, Value>,
}

/// Event pushed by workers (or addons) to the message queue and received by IM agents.
pub struct Event {
    /// The unique identifier of the event.
    pub id: Uuid,
    /// Kind of the event.
    pub kind: String,
    /// Entity affected by the event.
    pub entity: Uuid,
    /// Fields of the event.
    pub fields: Map<String, Value>,
}

/// IM subscriber.
pub struct User {
    /// The unique identifier of the user. The same physical user in different IMs should have different id.
    pub id: Uuid,
    /// The IM that the user is in.
    pub im: String,
    /// Name of the user.
    pub name: String,
    /// Avatar of the user.
    pub avatar: Vec<u8>,
    /// The events that the user is subscribed to.
    pub event_filter: EventFilter,
}

/// Filter for events.
pub struct EventFilter {
    /// Event must be related to these entities.
    pub entities: HashSet<Uuid>,
    /// Event must be in these kinds.
    pub kinds: HashSet<String>,
}

/// Wrapper for model providing `MongoDB` `ObjectId`.
#[derive(Debug, Serialize, Deserialize)]
pub struct InDB<T> {
    #[serde(rename = "_id")]
    id: ObjectId,
    #[serde(flatten)]
    inner: T,
}

impl<T> InDB<T> {
    /// Get the `ObjectId`.
    pub const fn id(&self) -> ObjectId {
        self.id
    }
    /// Get the inner body.
    #[allow(clippy::missing_const_for_fn)]
    pub fn inner(self) -> T {
        self.inner
    }
}

impl<T> Deref for InDB<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T> DerefMut for InDB<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}
