//! Contains several extensive traits for core models;

use mongodb::bson;
use sg_core::models::{Task, User};

use crate::rpc::{map, ApiError, ApiResult};

pub trait UserExt: Sized {
    fn assert_admin(self) -> ApiResult<Self>;
}

impl UserExt for User {
    fn assert_admin(self) -> ApiResult<Self> {
        if self.is_admin {
            Ok(self)
        } else {
            Err(ApiError::unauthorized())
        }
    }
}

pub trait TaskExt: Sized {
    fn new_youtube(channel_id: impl Into<String>, parent: mongodb::bson::Uuid) -> Self;
    fn new_bilibili(uid: impl Into<String>, parent: mongodb::bson::Uuid) -> Self;
    fn new_twitter(id: impl Into<String>, parent: mongodb::bson::Uuid) -> Self;
}

impl TaskExt for Task {
    fn new_youtube(channel_id: impl Into<String>, parent: mongodb::bson::Uuid) -> Self {
        let channel_id = channel_id.into();
        Self {
            id: bson::Uuid::new(),
            entity: parent,
            kind: "youtube".to_string(),
            params: map("channel_id", channel_id),
        }
    }

    fn new_bilibili(uid: impl Into<String>, parent: mongodb::bson::Uuid) -> Self {
        Self {
            id: bson::Uuid::new(),
            entity: parent,
            kind: "bililive".to_string(),
            params: map("uid", uid),
        }
    }

    fn new_twitter(id: impl Into<String>, parent: mongodb::bson::Uuid) -> Self {
        Self {
            id: bson::Uuid::new(),
            entity: parent,
            kind: "twitter".to_string(),
            params: map("id", id),
        }
    }
}
