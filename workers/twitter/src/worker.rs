use std::collections::HashMap;
use std::sync::Arc;

use egg_mode::tweet::user_timeline;
use egg_mode::user::UserID;
use egg_mode::Token;
use eyre::Result;
use futures_util::future::join_all;
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
    twitter_id: u64,
    token: &Token,
    entity_id: Uuid,
    mq: &MessageQueue,
) -> Result<()> {
    let mut stream =
        TimelineStream::new(user_timeline(UserID::ID(twitter_id), false, true, token)).await?;
    while let Some(res) = stream.next().await {
        let tweets = join_all(
            res?.into_iter()
                .map(|tweet| Tweet::from_raw(tweet.response)),
        )
        .await;
        for tweet in tweets {
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
        let id = if let Some(id) = task.params.get("id").and_then(Value::as_u64) {
            id
        } else {
            error!("Missing id in task params.");
            return false;
        };

        let token = self.token.clone();

        // Prepare the worker future.
        let fut = async move {
            loop {
                if let Err(error) = twitter_task(id, &token, task.entity.into(), &*self.mq).await {
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
