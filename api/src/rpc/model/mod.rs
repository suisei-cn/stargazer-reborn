//! Contains all model definition and trait implementations.

use std::time::SystemTime;

// Core models
use mongodb::bson::Uuid;
use sg_core::models::{Entity, EventFilter, Group, Meta, Task, User};
use url::Url;

use crate::successful_response;

mod_use::mod_use![bot, null, admin, add_task, user_query];

successful_response![Entity, Task, User, Group];

crate::methods! {
    // ---------------------- //
    // Does not require Token //
    // ---------------------- //
    /// Health check
    health := Health {} -> Null,

    /// Login with Username and Password
    ///
    /// This method checks for login information stored in DB,
    /// returns a token if matched and has sufficient permission.
    ///
    /// The token is composed with a nil user id (UUID with all 0),
    /// which cannot be used to request some methods that require user information
    /// like `update_setting` or `auth_user`
    login := Login {
        username: String,
        password: String,
    } -> Token {
        token: String,
        #[serde(with = "humantime_serde")]
        valid_until: SystemTime
    },

    // ----------- //
    // User method //
    // ----------  //
    /// Update user settings, return the updated `User`
    update_setting := UpdateSetting {
        /// New user preference
        event_filter: EventFilter
    } -> User,

    /// Get all entities, include vtbs and groups
    get_entities := GetEntities {
    } -> Entities {
        vtbs: Vec<Entity>,
        groups: Vec<Group>
    },

    /// Authorize user
    auth_user := AuthUser {
    } -> Authorized {
        /// Return info about user
        user: User,
        #[serde(with = "humantime_serde")]
        valid_until: SystemTime
    },

    // ---------- //
    // Bot method //
    // ---------- //

    /// Create a new token
    new_token := NewToken {
        /// Either (`user id`) or combination of (`im` and `im_payload`)
        /// that can be used to look up user
        #[serde(flatten)]
        query: UserQuery,
    } -> Token,

    /// Create a new user.
    add_user := AddUser {
        /// The IM that the user is in.
        im: String,
        /// IM payload, e.g. Chat id in telegram
        im_payload: String,
        /// Avatar of the user.
        avatar: Url,
        /// Name of the user.
        name: String
    } -> User,

    /// Delete an existing user.
    del_user := DelUser {
        /// Either `user id` or `im` and `im_payload` of the user
        #[serde(flatten)]
        query: UserQuery,
    } -> User,

    // ------------ //
    // Admin method //
    // ------------ //
    add_task := AddTask {
        #[serde(flatten)]
        /// Task parameter
        param: AddTaskParam,
        /// The ID of this entity which this task belongs to.
        entity_id: Uuid,
    } -> Task,

    del_task := DelTask {
        /// The ID of the task going to be deleted.
        task_id: Uuid
    } -> Task,

    add_entity := AddEntity {
        /// Meta of the entity
        meta: Meta,
        /// List of tasks that this entity has.
        tasks: Vec<AddTaskParam>
    } -> Entity,

    /// Update the entity's meta. Return the new entity.
    update_entity := UpdateEntity {
        /// The ID of the entity
        entity_id: Uuid,
        /// Meta of the entity
        meta: Meta,
    } -> Entity,

    /// Update an entity. Return the deleted entity.
    del_entity := DelEntity {
        /// The ID of the entity
        entity_id: Uuid
    } -> Entity,
}
