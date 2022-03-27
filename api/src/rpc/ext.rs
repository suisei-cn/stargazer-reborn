//! Contains several extensive traits for core models;

use mongodb::bson::Uuid;
use sg_core::models::{Task, User};

use crate::{
    map,
    rpc::{models::AddTask, ApiError, ApiResult},
};

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
    fn new_youtube(channel_id: impl Into<String>, parent: Uuid) -> Self;
    fn new_bilibili(uid: impl Into<String>, parent: Uuid) -> Self;
    fn new_twitter(id: impl Into<String>, parent: Uuid) -> Self;
}

impl TaskExt for Task {
    fn new_youtube(channel_id: impl Into<String>, parent: Uuid) -> Self {
        let channel_id = channel_id.into();
        Self {
            id: Uuid::new(),
            entity: parent,
            kind: "youtube".to_string(),
            params: map("channel_id", channel_id),
        }
    }

    fn new_bilibili(uid: impl Into<String>, parent: Uuid) -> Self {
        Self {
            id: Uuid::new(),
            entity: parent,
            kind: "bililive".to_string(),
            params: map("uid", uid),
        }
    }

    fn new_twitter(id: impl Into<String>, parent: Uuid) -> Self {
        Self {
            id: Uuid::new(),
            entity: parent,
            kind: "twitter".to_string(),
            params: map("id", id),
        }
    }
}

impl From<AddTask> for Task {
    fn from(new_task: AddTask) -> Self {
        let AddTask {
            entity_id, param, ..
        } = new_task;
        param.into_task_with(entity_id)
    }
}
