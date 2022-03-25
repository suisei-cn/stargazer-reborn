//! Contains all model definition and trait implementations.

use std::{
    collections::{HashMap, HashSet},
    time::SystemTime,
};

use sg_core::models::{Entity, EventFilter, Group, Meta, User};
use url::Url;

use crate::rpc::Response;

impl Response for User {
    fn is_successful(&self) -> bool {
        true
    }
}

#[derive(Debug, Clone, PartialEq, Eq, ::serde::Serialize, ::serde::Deserialize)]
pub struct Vtb {
    pub id: uuid::Uuid,
    #[serde(flatten)]
    pub meta: Meta,
}

impl From<Entity> for Vtb {
    fn from(entity: Entity) -> Self {
        Self {
            id: entity.id.into(),
            meta: entity.meta,
        }
    }
}

/// Response object that has no content.
/// Usually be used to indictate the operation is successful but nothing to return.
/// Similar to HTTP Code 204.
#[derive(Debug, Clone, PartialEq, Eq, ::serde::Serialize, ::serde::Deserialize)]
pub struct Null;

impl Response for Null {
    fn is_successful(&self) -> bool {
        true
    }
}

crate::methods! {
    // ---------------------- //
    // Does not require Token //
    // ---------------------- //

    /// Get the user information.
    "getUser" := GetUser {
        user_id: uuid::Uuid
    } -> User,

    /// Get all entities, include vtbs and groups
    "getEntities" := GetEntities {} -> Entities {
        vtbs: Vec<Vtb>,
        groups: Vec<Group>
    },

    // ---------------------- //
    // Does requires Password //
    // ---------------------- //

    /// Create a new session for user. This method should only be used by bots.
    ///
    /// **TODO**: `password` should be replaced by a more secure way in future.
    "newSession" := NewSession {
        user_id: uuid::Uuid,
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
    "addUser" := AddUser {
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
    "delUser" := DelUser {
        user_id: uuid::Uuid
        // Bot password
        password: String,
    } -> Null,

    // -------------- //
    // Requires Token //
    // -------------- //

    "updateUserSetting" := UpdateUserSetting {
        user_id: uuid::Uuid,
        token: String,
        event_filter: EventFilter
    } -> Null,

    "authUser" := AuthUser {
        user_id: uuid::Uuid,
        token: String
    } -> Authorized {
        user: User,
        #[serde(with = "humantime_serde")]
        valid_until: SystemTime
    },
}
