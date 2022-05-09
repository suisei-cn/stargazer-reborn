use sg_core::models::Task;

use crate::rpc::model::AddTask;

impl From<AddTask> for Task {
    fn from(new_task: AddTask) -> Self {
        let AddTask {
            entity_id, param, ..
        } = new_task;
        param.into_task_with(entity_id)
    }
}
