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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use chrono::Utc;
    use diesel::r2d2::{ConnectionManager, Pool};
    use diesel::{RunQueryDsl, SqliteConnection};
    use tokio::time::sleep;
    use uuid::Uuid;

    use sg_core::models::Event;
    use sg_core::mq::mock::MockMQ;
    use sg_core::mq::Middlewares;

    use crate::{delayed_messages, embedded_migrations, DelayedMessage, Scheduler};

    #[derive(Debug, Eq, PartialEq)]
    enum TestAction {
        Normal,
        Cleanup,
        Cancel,
    }

    #[tokio::test]
    async fn must_persist() {
        test_persist(TestAction::Normal).await;
    }

    #[tokio::test]
    async fn must_cancel() {
        test_persist(TestAction::Cancel).await;
    }

    #[tokio::test]
    async fn must_cleanup() {
        test_persist(TestAction::Cleanup).await;
    }

    async fn test_persist(action: TestAction) {
        // Prepare temp file.
        let temp_file = tempfile::NamedTempFile::new().unwrap();
        let db_path = temp_file.path().to_string_lossy().to_string();

        // Prepare the db.
        let pool = Pool::new(ConnectionManager::new(&db_path)).unwrap();
        embedded_migrations::run(&pool.get().unwrap()).unwrap();

        let mq = MockMQ::default();

        {
            let scheduler = Arc::new(Scheduler::new(pool, mq));

            let msg = DelayedMessage::new(
                114_514,
                Middlewares::default(),
                Event::from_serializable("", Uuid::nil(), ()).unwrap(),
                Utc::now().naive_utc() + chrono::Duration::milliseconds(500), // Deliver the message later so it may be added to the queue.
            );
            scheduler.add_task(msg, true);
            assert_eq!(
                scheduler.delayed_messages.lock().len(),
                1,
                "There should be one delayed message"
            );

            if action == TestAction::Cancel {
                scheduler.remove_task(114_514);
                assert!(
                    scheduler.delayed_messages.lock().is_empty(),
                    "There should be no delayed messages"
                );
            }
        }
        // Now the scheduler is out of scope.

        if action == TestAction::Cleanup {
            // We wait for the message to expire.
            sleep(std::time::Duration::from_secs(2)).await;
        }

        // Now load the db again.
        let pool = Pool::new(ConnectionManager::new(&db_path)).unwrap();
        let mq = MockMQ::default();
        let scheduler = Arc::new(Scheduler::new(pool, mq));
        if action == TestAction::Cleanup {
            scheduler.cleanup();
        }
        scheduler.load();

        match action {
            TestAction::Normal => {
                assert_eq!(
                    scheduler.delayed_messages.lock().len(),
                    1,
                    "There should be one delayed messages"
                );
            }
            TestAction::Cleanup => {
                assert!(
                    scheduler.delayed_messages.lock().is_empty(),
                    "There should be no delayed messages"
                );

                // And we make sure the entry in db is removed.
                let pool = Pool::new(ConnectionManager::<SqliteConnection>::new(&db_path)).unwrap();
                let conn = pool.get().expect("No db conn available");
                let results = delayed_messages.load::<DelayedMessage>(&conn).unwrap();
                assert!(
                    results.is_empty(),
                    "There should be no delayed messages in db"
                );
            }
            TestAction::Cancel => {
                assert!(
                    scheduler.delayed_messages.lock().is_empty(),
                    "There should be no delayed messages"
                );
            }
        }
    }
}
