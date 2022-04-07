//! Models for the entity collection.
use std::collections::{HashMap, HashSet};
use std::ops::{Deref, DerefMut};

use eyre::{bail, Result, WrapErr};
use isolanguage_1::LanguageCode;
use mongodb::bson::oid::ObjectId;
use mongodb::bson::Uuid;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use url::Url;

/// Entity for a vtuber.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entity {
    /// The unique identifier of the entity.
    pub id: Uuid,
    /// Metadata about the entity.
    pub meta: Meta,
    /// Tasks to be scheduled.
    pub tasks: Vec<Uuid>,
}

/// Meta of the vtuber.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Meta {
    /// Vtuber's name.
    pub name: Name,
    /// Affiliation of the vtuber.
    pub group: Option<Uuid>,
}

/// Name of a vtuber/group.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Name {
    /// Name in different languages. The key must be in ISO 639-1.
    pub name: HashMap<LanguageCode, String>,
    /// Preferred language of the name. Must be in ISO 639-1.
    pub default_language: LanguageCode,
}

/// A group/organization of vtubers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Group {
    /// The unique identifier of the group.
    pub id: Uuid,
    /// Group's name.
    pub name: Name,
}

/// Defined task for a vtuber.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

impl Event {
    /// Create a new event with a given id with its fields set by a serializable object.
    ///
    /// # Errors
    /// Returns an error if the fields cannot be serialized into a map.
    pub fn from_serializable_with_id(
        id: impl Into<Uuid>,
        kind: &str,
        entity: impl Into<Uuid>,
        fields: impl Serialize,
    ) -> Result<Self> {
        let value = serde_json::to_value(fields)
            .wrap_err("event fields can't be converted into json value")?;
        let fields = match value {
            Value::Null => Map::new(),
            Value::Object(m) => m,
            _ => bail!("event field is not a map"),
        };

        Ok(Self {
            id: id.into(),
            kind: kind.to_string(),
            entity: entity.into(),
            fields,
        })
    }
    /// Create a new event with its fields set by a serializable object.
    ///
    /// # Errors
    /// Returns an error if the fields cannot be serialized into a map.
    pub fn from_serializable(
        kind: &str,
        entity: impl Into<Uuid>,
        fields: impl Serialize,
    ) -> Result<Self> {
        Self::from_serializable_with_id(Uuid::new(), kind, entity, fields)
    }
}

/// IM subscriber.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct User {
    /// The unique identifier of the user. The same physical user in different IMs should have different id.
    pub id: Uuid,
    /// The IM that the user is in, e.g. "tg" for telegram
    pub im: String,
    /// IM payload, e.g. Chat id in telegram
    pub im_payload: String,
    /// Display name of the user.
    pub name: String,
    /// Avatar of the user.
    pub avatar: Url,
    /// Admin privilege of the user, this can be set via admin web ui.
    pub is_admin: bool,
    /// The events that the user is subscribed to.
    pub event_filter: EventFilter,
}

/// Filter for events.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
