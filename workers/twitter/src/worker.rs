//! Worker implementation.

use std::{collections::HashMap, sync::Arc, time::Duration};

use egg_mode::{tweet::user_timeline, user::UserID, Token};
use eyre::Result;
use futures_util::StreamExt;
use parking_lot::Mutex;
use serde_json::Value;
use sg_core::{
    models::{Event, Task},
    mq::MessageQueue,
    protocol::WorkerRpc,
    utils::ScopedJoinHandle,
};
use tap::TapOptional;
use tarpc::context::Context;
use tokio::time::{interval, sleep};
use tracing::{error, info};
use uuid::Uuid;

use crate::{
    twitter::{TimelineStream, Tweet},
    Config,
};

/// Twitter worker.
#[derive(Clone)]
pub struct TwitterWorker {
    token: Arc<Token>,
    mq: Arc<dyn MessageQueue>,
    interval: Duration,

    #[allow(clippy::type_complexity)]
    tasks: Arc<Mutex<HashMap<Uuid, (Task, ScopedJoinHandle<()>)>>>,
}

impl TwitterWorker {
    /// Creates a new worker.
    #[must_use]
    pub fn new(config: Config, mq: impl MessageQueue + 'static) -> Self {
        Self {
            token: Arc::new(Token::Bearer(config.twitter_token)),
            mq: Arc::new(mq),
            interval: config.poll_interval,
            tasks: Arc::new(Mutex::new(HashMap::new())),
        }
    }
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

        // Prepare the worker future.
        let token = self.token.clone();
        let poll_interval = self.interval;

        let fut = async move {
            loop {
                info!(user_id=?id, "Spawning twitter task");
                if let Err(error) = twitter_task(
                    id.clone(),
                    &token,
                    task.entity.into(),
                    &*self.mq,
                    poll_interval,
                )
                .await
                {
                    error!(?error, "Failed to fetch timeline");

                    // Sleep to avoid looping if the task always fails.
                    sleep(poll_interval).await;
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

// Fetch the timeline for the given user and send the tweets to the message
// queue.
async fn twitter_task(
    user_id: UserID,
    token: &Token,
    entity_id: Uuid,
    mq: impl MessageQueue,
    poll_interval: Duration,
) -> Result<()> {
    let mut ticker = interval(poll_interval);

    // Construct a stream of tweets.
    let mut stream = TimelineStream::new(user_timeline(user_id, false, true, token)).await?;
    while let Some(resp) = stream.next().await {
        // Parse income tweets.
        for raw_tweet in resp?.response {
            let tweet_id = raw_tweet.id;
            let tweet = Tweet::from(raw_tweet);
            let event = Event::from_serializable("twitter", entity_id, tweet)?;

            // Send tweet to message queue.
            if let Err(error) = mq.publish(event, "translate".parse().unwrap()).await {
                error!(?error, %tweet_id, "Failed to publish tweet");
            }
        }

        // Tick.
        ticker.tick().await;
    }

    Ok(())
}
