//! Utility structs and functions.

use std::ops::{Deref, DerefMut};

use tokio::task::JoinHandle;

/// A wrapper that holds a join handle and abort the task if dropped.
#[derive(Debug)]
pub struct ScopedJoinHandle<T>(pub JoinHandle<T>);

impl<T> Deref for ScopedJoinHandle<T> {
    type Target = JoinHandle<T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for ScopedJoinHandle<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T> Drop for ScopedJoinHandle<T> {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// A macro to quickly create a single `kv` [`map`].
///
/// [`map`]: serde_json::Map
macro_rules! map {
    ($k: expr, $v: expr) => {{
        let mut map = serde_json::Map::new();
        map.insert($k.into(), Value::String($v.into()));
        map
    }};
}

pub(crate) use map;

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::task::yield_now;
    use tokio::time::sleep;

    use crate::utils::ScopedJoinHandle;

    #[tokio::test]
    async fn must_abort_on_drop() {
        let (tx, rx) = tokio::sync::oneshot::channel::<()>();
        let handle = ScopedJoinHandle(tokio::spawn(async move {
            // Hold the receiver.
            let _rx = rx;

            // Sleep infinitely.
            loop {
                sleep(Duration::from_secs(99999)).await;
            }
        }));

        // Drop the handle to abort the task.
        drop(handle);

        // Yield to the runtime to let the task abort.
        yield_now().await;

        // The task should be aborted, and the channel should be closed.
        assert!(tx.is_closed());
    }
}
