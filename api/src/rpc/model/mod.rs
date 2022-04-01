//! Contains all model definition and trait implementations.

use std::time::SystemTime;

// Core models
use mongodb::bson::Uuid;
use sg_core::models::{Entity, EventFilter, Group, Meta, Task, User};
use url::Url;

use crate::successful_response;

mod_use::mod_use![bot, null, admin, add_task, user_query];

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
    /// Update user settings, return the updated `User`
    update_setting := UpdateSetting {
        /// User token
        token: String,
        /// New user preference
        event_filter: EventFilter
    } -> User,

    /// Get all entities, include vtbs and groups
    get_entities := GetEntities {
        /// User token
        token: String
    } -> Entities {
        vtbs: Vec<Entity>,
        groups: Vec<Group>
    },

    /// Authorize user
    auth_user := AuthUser {
        /// User token
        token: String,
    } -> Authorized {
        /// Return info about user
        user: User,
        /// `exp` of the token
        #[serde(with = "humantime_serde")]
        valid_until: SystemTime
    },

    // ---------- //
    // Bot method //
    // ---------- //

    /// Generate a new token for user.
    new_token := NewToken {
        /// Either `user id` or `im` and `im_payload` of the user
        #[serde(flatten)]
        query: UserQuery,
        /// Bot or admin token
        token: String
    } -> Token {
        token: String,
        #[serde(with = "humantime_serde")]
        valid_until: SystemTime
    },

    /// Create a new user.
    add_user := AddUser {
        /// The IM that the user is in.
        im: String,
        /// IM payload, e.g. Chat id in telegram
        im_payload: String,
        /// Avatar of the user.
        avatar: Url,
        /// Bot or admin token
        token: String,
        /// Name of the user.
        name: String
    } -> User,

    /// Delete an existing user.
    del_user := DelUser {
        /// Either `user id` or `im` and `im_payload` of the user
        #[serde(flatten)]
        query: UserQuery,
        /// Bot or admin token
        token: String,
    } -> User,

    // ------------ //
    // Admin method //
    // ------------ //

    adjust_user_privilege := AdjustUserPrivilege {
        /// Either `user id` or `im` and `im_payload` of the user
        #[serde(flatten)]
        query: UserQuery,
        /// See [`User`]
        is_admin: bool,
        /// Admin token
        token: String
    } -> User,

    add_task := AddTask {
        /// Admin token
        token: String,
        #[serde(flatten)]
        /// Task parameter
        param: AddTaskParam,
        /// The ID of this entity which this task belongs to.
        entity_id: Uuid,
    } -> Task,

    del_task := DelTask {
        /// Admin token
        token: String,
        /// The ID of the task going to be deleted.
        task_id: Uuid
    } -> Task,

    add_entity := AddEntity {
        /// Admin token
        token: String,
        /// Meta of the entity
        meta: Meta,
        /// List of tasks that this entity has.
        tasks: Vec<AddTaskParam>
    } -> Entity,

    /// Update the entity's meta. Return the new entity.
    update_entity := UpdateEntity {
        /// Admin token
        token: String,
        /// The ID of the entity
        entity_id: Uuid,
        /// Meta of the entity
        meta: Meta,
    } -> Entity,

    /// Update an entity. Return the deleted entity.
    del_entity := DelEntity {
        /// Admin token
        token: String,
        /// The ID of the entity
        entity_id: Uuid
    } -> Entity,

    /// Create a new bot. This will also generate a corresponding bot record in database.
    new_bot := NewBot {
        /// Admin token
        token: String,
    } -> BotInfo {
        /// UUID of the new bots
        uuid: Uuid,
        /// Token of the bot
        token: String,
    },

    get_bots := GetBots {
        /// Admin token
        token: String
    } -> Bots {
        /// List of bots this admin created
        bots: Vec<BotInfo>
    },
}
