use std::collections::HashMap;
use std::sync::{Arc, Weak};

use chrono::Utc;
use diesel::associations::HasTable;
use diesel::dsl::now;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl, SqliteConnection};
use parking_lot::Mutex;
use tokio::time::sleep;
use tracing::{error, info};

use sg_core::mq::MessageQueue;
use sg_core::utils::ScopedJoinHandle;

use crate::schema::delayed_messages::{deliver_at, id};
use crate::{delayed_messages, DelayedMessage};

pub struct Scheduler {
    pool: Pool<ConnectionManager<SqliteConnection>>,
    mq: Arc<dyn MessageQueue>,
    delayed_messages: Mutex<HashMap<i64, DelayedTask>>,
}

pub struct DelayedTask {
    _handler: ScopedJoinHandle<()>,
}

impl DelayedTask {
    fn new(
        scheduler: Weak<Scheduler>,
        mq: impl MessageQueue + 'static,
        message: DelayedMessage,
    ) -> Self {
        let task = tokio::spawn(async move {
            let delay = message.deliver_at - Utc::now().naive_utc();
            let x_delay_id = message.id;
            let event_id = message.body.0.id;
            match delay.to_std() {
                Ok(delay) => {
                    sleep(delay).await;
                    if let Err(error) = mq.publish(message.body.0, message.middlewares.0).await {
                        error!(%event_id, %x_delay_id, ?error, "Unable to deliver delayed message");
                    }
                }
                Err(error) => {
                    error!(%event_id, %x_delay_id, ?error, "Deliver time is in the past");
                }
            }
            if let Some(scheduler) = scheduler.upgrade() {
                scheduler.remove_task(message.id);
            }
        });
        Self {
            _handler: ScopedJoinHandle(task),
        }
    }
}

impl Scheduler {
    pub fn new(
        pool: Pool<ConnectionManager<SqliteConnection>>,
        mq: impl MessageQueue + 'static,
    ) -> Self {
        Self {
            pool,
            mq: Arc::new(mq),
            delayed_messages: Mutex::new(HashMap::new()),
        }
    }
    #[allow(clippy::cognitive_complexity)]
    pub fn add_task(self: &Arc<Self>, msg: DelayedMessage, persist: bool) {
        if persist {
            let conn = self.pool.get().unwrap();
            let r = diesel::insert_into(delayed_messages::table())
                .values(&msg)
                .execute(&conn);
            match r {
                Ok(count) if count == 0 => {
                    error!(
                        error = "No rows inserted",
                        "Unable to persist delayed message."
                    );
                }
                Err(error) => {
                    error!(?error, "Unable to persist delayed message.");
                }
                _ => (),
            }
        }

        let msg_id = msg.id;
        let task = DelayedTask::new(Arc::downgrade(self), self.mq.clone(), msg);
        if self.delayed_messages.lock().insert(msg_id, task).is_some() {
            info!(id = %msg_id, "Overwriting existing delayed message");
        } else {
            info!(id = %msg_id, "Added delayed message");
        }
    }
    pub fn remove_task(&self, task_id: i64) {
        if self.delayed_messages.lock().remove(&task_id).is_some() {
            let conn = self.pool.get().expect("No db conn available");
            if let Err(error) =
                diesel::delete(delayed_messages.filter(id.eq(task_id))).execute(&conn)
            {
                error!(?error, "Failed to remove task from database");
            }
        }

        if self.delayed_messages.lock().remove(&task_id).is_some() {
            info!(id = %task_id, "Removed delayed message");
        } else {
            info!(id = %task_id, "No delayed message to remove");
        }
    }
    pub fn load(self: &Arc<Self>) {
        let conn = self.pool.get().expect("No db conn available");
        let results = delayed_messages.load::<DelayedMessage>(&conn);
        match results {
            Ok(messages) => {
                for message in messages {
                    self.add_task(message, false);
                }
            }
            Err(error) => {
                error!(?error, "Failed to load persisted delayed messages");
            }
        }
    }
    pub fn cleanup(&self) {
        let conn = self.pool.get().expect("No db conn available");
        let r = diesel::delete(delayed_messages::table())
            .filter(deliver_at.lt(now))
            .execute(&conn);
        match r {
            Ok(count) => {
                info!(count = %count, "Removed misfired delayed messages from database");
            }
            Err(error) => {
                error!(%error, "Failed to remove misfired delayed messages from database");
            }
        }
    }
}
