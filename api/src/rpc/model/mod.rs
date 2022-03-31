//! Contains all model definition and trait implementations.

use std::time::SystemTime;

// Core models
use mongodb::bson::Uuid;
use sg_core::models::{Entity, EventFilter, Group, Meta, Task, User};
use url::Url;

use crate::successful_response;

mod_use::mod_use![null, admin, add_task];

successful_response![Null, Entity, Task, User, Group];

crate::methods! {
    // ---------------------- //
    // Does not require Token //
    // ---------------------- //
    /// Health check
    health := Health {} -> Null,


    // ----------- //
    // User method //
    // ----------  //
    update_setting := UpdateSetting {
        token: String,
        event_filter: EventFilter
    } -> User,

    /// Get all entities, include vtbs and groups
    get_entities := GetEntities {
        token: String
    } -> Entities {
        vtbs: Vec<Entity>,
        groups: Vec<Group>
    },

    auth_user := AuthUser {
        user_id: Uuid,
        token: String,
    } -> Authorized {
        user: User,
        #[serde(with = "humantime_serde")]
        valid_until: SystemTime
    },

    // ---------- //
    // Bot method //
    // ---------- //

    /// Create a new session for user. This method should only be used by bots.
    new_session := NewSession {
        user_id: Uuid,
        // Bot password
        token: String
    } -> Session {
        token: String,
        #[serde(with = "humantime_serde")]
        valid_until: SystemTime
    },

    /// Create a new user. This method should only be used by bots.
    add_user := AddUser {
        // The IM that the user is in.
        im: String,
        // Avatar of the user.
        avatar: Url,
        // Bot password
        token: String,
        // Name of the user.
        name: String
    } -> User,

    /// Delete an existing user. This method should only be used by bots.
    del_user := DelUser {
        user_id: Uuid
        // Bot password
        token: String,
    } -> User,

    refresh_token := RefreshToken {
        token: String
    } -> BotToken,

    // ------------ //
    // Admin method //
    // ------------ //

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

    /// Update the entity's meta. Return the new entity.
    update_entity := UpdateEntity {
        token: String,
        entity_id: Uuid,
        meta: Meta,
    } -> Entity,

    /// Update an entity. Return the deleted entity.
    del_entity := DelEntity {
        token: String,
        entity_id: Uuid
    } -> Entity,

    /// Create a new bot token. This will also generate a corresponding bot record in database.
    /// The created token will expire in a period of time.
    /// When it's expired, the bot can use [`RefreshToken`] to refresh it, if the bot is not revoked.
    new_bot_token := NewBotToken {
        token: String,
    } -> BotToken {
        token: String,
        uuid: Uuid,
        #[serde(with = "humantime_serde")]
        valid_until: SystemTime
    },


}
