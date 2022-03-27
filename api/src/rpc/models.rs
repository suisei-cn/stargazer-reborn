//! Contains all model definition and trait implementations.

use std::time::SystemTime;

use serde::{Deserialize, Serialize};

// Core models
use mongodb::bson::Uuid;
use sg_core::models::{Entity, EventFilter, Group, Meta, Task, User};
use url::Url;

use crate::{rpc::TaskExt, successful_response};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
#[serde(rename_all = "lowercase")]
pub enum AddTaskParam {
    Youtube { channel_id: String },
    Bilibili { uid: String },
    Twitter { id: String },
}

impl AddTaskParam {
    pub fn into_task_with(self, entity_id: Uuid) -> Task {
        match self {
            AddTaskParam::Youtube { channel_id } => Task::new_youtube(channel_id, entity_id),
            AddTaskParam::Bilibili { uid } => Task::new_bilibili(uid, entity_id),
            AddTaskParam::Twitter { id } => Task::new_twitter(id, entity_id),
        }
    }
}

successful_response![Entity, Task, User, Group];

crate::methods! {
    // ---------------------- //
    // Does not require Token //
    // ---------------------- //

    /// Get all entities, include vtbs and groups
    get_entities := GetEntities {} -> Entities {
        vtbs: Vec<Entity>,
        groups: Vec<Group>
    },

    // ------------------ //
    // Does require Token //
    // ------------------ //

    update_setting := UpdateSetting {
        token: String,
        event_filter: EventFilter
    } -> User,

    auth_user := AuthUser {
        user_id: Uuid,
        token: String,
    } -> Authorized {
        user: User,
        #[serde(with = "humantime_serde")]
        valid_until: SystemTime
    },

    // ------------------ //
    // Does require Admin //
    // ------------------ //

    add_task := AddTask {
        token: String,
        #[serde(flatten)]
        param: AddTaskParam,
        entity_id: Uuid,
    } -> Task,

    del_task := DelTask {
        token: String,
        task_id: Uuid
    } -> Task,

    add_entity := AddEntity {
        token: String,
        meta: Meta,
        tasks: Vec<AddTaskParam>
    } -> Entity,

    /// Update the entity's meta. Return the updated entity.
    update_entity := UpdateEntity {
        token: String,
        entity_id: Uuid,
        meta: Meta,
    } -> Entity,

    del_entity := DelEntity {
        token: String,
        entity_id: Uuid
    } -> Entity,

    // --------------------- //
    // Does require Password //
    // --------------------- //

    /// Create a new session for user. This method should only be used by bots.
    ///
    /// **TODO**: `password` should be replaced by a more secure way in future.
    new_session := NewSession {
        user_id: Uuid,
        // Bot password
        password: String
    } -> Session {
        token: String,
        #[serde(with = "humantime_serde")]
        valid_until: SystemTime
    }

    /// Create a new user. This method should only be used by bots.
    ///
    /// **TODO**: `password` should be replaced by a more secure way in future.
    add_user := AddUser {
        // The IM that the user is in.
        im: String,
        // Avatar of the user.
        avatar: Url,
        // Bot password
        password: String,
        // Name of the user.
        name: String
    } -> User,

    /// Delete an existing user. This method should only be used by bots.
    ///
    /// **TODO**: `password` should be replaced by a more secure way in future.
    del_user := DelUser {
        user_id: Uuid
        // Bot password
        password: String,
    } -> User,
}
