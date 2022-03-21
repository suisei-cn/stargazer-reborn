use std::time::SystemTime;

use sg_core::models::{Entity, EventFilter, Group, Meta, User};
use url::Url;
use uuid::Uuid;

use crate::rpc::Response;

impl Response for User {
    fn is_successful(&self) -> bool {
        true
    }
}

#[derive(Debug, Clone, PartialEq, Eq, ::serde::Serialize, ::serde::Deserialize)]
pub struct Vtb {
    pub id: Uuid,
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
pub struct NullResponse {}

impl Response for NullResponse {
    fn is_successful(&self) -> bool {
        true
    }
}

crate::methods! {
    // ---------------------- //
    // Does not require Token //
    // ---------------------- //

    "getUser" := GetUser {
        user_id: String
    } -> User,

    "getEntities" := GetEntities {} -> Entities {
        vtbs: Vec<Vtb>,
        groups: Vec<Group>
    },

    // -------------- //
    // Requires Token //
    // -------------- //

    "newSession" := NewSession {
        user_id: String,
        password: String
    } -> Session {
        token: String,
        #[serde(with = "humantime_serde")]
        valid_until: SystemTime
    }

    "delUser" := DelUser {
        token: String,
        user_id: String
    } -> NullResponse,

    "addUser" := AddUser {
        // The IM that the user is in.
        im: String,
        // Avatar of the user.
        avatar: Url,
        token: String,
        name: String
    } -> User,

    "updateUserSetting" := UpdateUserSetting {
        user_id: String,
        token: String,
        event_filter: EventFilter
    } -> User,

    "authMe" := AuthMe {
        user_id: String,
        token: String
    } -> User,
}
