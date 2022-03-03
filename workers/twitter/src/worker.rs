use std::collections::HashMap;
use std::sync::Arc;

use egg_mode::tweet::user_timeline;
use egg_mode::user::UserID;
use egg_mode::Token;
use eyre::Result;
use futures_util::StreamExt;
use parking_lot::Mutex;
use serde_json::Value;
use tap::TapOptional;
use tarpc::context::Context;
use tracing::{error, info};
use uuid::Uuid;

use sg_core::models::Task;
use sg_core::protocol::WorkerRpc;
use sg_core::utils::ScopedJoinHandle;

use crate::models::Tweet;
use crate::mq::MessageQueue;
use crate::twitter::TimelineStream;

#[derive(Clone)]
pub struct TwitterWorker {
    token: Arc<Token>,
    mq: Arc<MessageQueue>,

    #[allow(clippy::type_complexity)]
    tasks: Arc<Mutex<HashMap<Uuid, (Task, ScopedJoinHandle<()>)>>>,
}

impl TwitterWorker {
    pub fn new(token: String, mq: MessageQueue) -> Self {
        Self {
            token: Arc::new(Token::Bearer(token)),
            mq: Arc::new(mq),
            tasks: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

async fn twitter_task(
    user_id: UserID,
    token: &Token,
    entity_id: Uuid,
    mq: &MessageQueue,
) -> Result<()> {
    let mut stream =
        TimelineStream::new(user_timeline(user_id, false, true, token)).await?;
    while let Some(resp) = stream.next().await {
        for raw_tweet in resp?.response {
            let tweet = Tweet::from(raw_tweet);
            let tweet_id = tweet.id;
            if let Err(error) = mq.publish(entity_id, tweet).await {
                error!(?error, %tweet_id, "Failed to publish tweet");
            }
        }
    }
    Ok(())
}

#[tarpc::server]
impl WorkerRpc for TwitterWorker {
    async fn ping(self, _: Context, id: u64) -> u64 {
        id
    }

    async fn add_task(self, _: Context, task: Task) -> bool {
        let mut tasks = self.tasks.lock();
        if tasks.contains_key(&task.id.into()) {
            // If the task is already running, do nothing.
            return false;
        }

        info!(task_id = ?task.id, "Adding task");

        // Extract the twitter id from the task.
        let id = match task.params.get("id") {
            Some(Value::Number(id)) if id.is_u64() => UserID::ID(id.as_u64().unwrap()),
            Some(Value::String(screen_name)) => UserID::from(screen_name.to_string()),
            Some(_) => {
                error!("ID field: type mismatch. Expected: u64 or String");
                return false;
            }
            None => {
                error!("ID field: missing");
                return false;
            }
        };

        let token = self.token.clone();

        // Prepare the worker future.
        let fut = async move {
            loop {
                if let Err(error) = twitter_task(id.clone(), &token, task.entity.into(), &*self.mq).await {
                    error!(?error, "Failed to fetch timeline");
                }
            }
        };

        // Spawn the worker and insert it into the tasks map.
        tasks.insert(task.id.into(), (task, ScopedJoinHandle(tokio::spawn(fut))));

        true
    }

    async fn remove_task(self, _: Context, id: Uuid) -> bool {
        self.tasks
            .lock()
            .remove(&id)
            .tap_some(|_| info!(task_id=?id, "Removing task"))
            .is_some()
    }

    async fn tasks(self, _: Context) -> Vec<Task> {
        self.tasks
            .lock()
            .values()
            .map(|(task, _)| task)
            .cloned()
            .collect()
    }
}
