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
