use mongodb::bson::Uuid;
use serde::{Deserialize, Serialize};
use sg_core::models::Task;

use crate::TaskExt;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
#[serde(rename_all = "lowercase")]
pub enum AddTaskParam {
    Youtube { channel_id: String },
    Bilibili { uid: String },
    Twitter { id: String },
}

impl AddTaskParam {
    #[must_use]
    pub fn into_task_with(self, entity_id: Uuid) -> Task {
        match self {
            AddTaskParam::Youtube { channel_id } => Task::new_youtube(channel_id, entity_id),
            AddTaskParam::Bilibili { uid } => Task::new_bilibili(uid, entity_id),
            AddTaskParam::Twitter { id } => Task::new_twitter(id, entity_id),
        }
    }
}
