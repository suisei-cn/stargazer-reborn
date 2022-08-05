//! Utility structs and functions.

use std::ops::{Deref, DerefMut};

use tokio::task::JoinHandle;

#[cfg(any(feature = "core_derive", test))]
pub use core_derive::Config;
#[cfg(any(feature = "figment", test))]
pub use figment_ext::*;

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

#[cfg(any(feature = "figment", test))]
mod figment_ext {
    use eyre::Result;
    use figment::providers::{Env, Serialized};
    use figment::Figment;
    use serde::Deserialize;

    #[doc(hidden)]
    pub extern crate serde_json;

    /// Helper trait for config structs.
    ///
    /// All config structs should implement `Deserialize` and `Config`.
    ///
    /// # Default values
    /// To set default values for config fields, use the `default` attribute.
    ///
    /// ```
    /// use serde::{Deserialize, Serialize};
    /// use core_derive::Config;
    ///
    /// // Override crate name for core crate if its name is not `sg_core`.
    /// // E.g. `#[config(core = "crate_name")]`
    /// #[derive(Deserialize, Config)]
    /// # #[config(core = "crate")]
    /// struct Config {
    ///     name: String,
    ///     // Set default value for `age` field. Accepts a literal.
    ///     #[config(default = "18")]
    ///     age: usize,
    ///     // To make setting str literal easier, use `default_str` instead of `default`.
    ///     // Without `default_str`, you must write `#[config(default = "\"foo\"")]`.
    ///     #[config(default_str = "foo")]
    ///     field: String,
    ///     // Note: Types annotated with `#[config(default)]` must implement `Serialize`.
    ///     #[config(default)]
    ///     nested: Nested
    /// }
    ///
    /// #[derive(Deserialize, Serialize, Default)]
    /// struct Nested {
    ///     #[config(default)]
    ///     random_field: usize
    /// }
    /// ```
    pub trait FigmentExt {
        /// Load config from environment variables.
        ///
        /// # Nested structs
        ///
        /// Nested structs can be loaded by splitting the key with `__`.
        ///
        /// E.g. `PREFIX_A__B` can be loaded to `Config { a: Nested { b } }`.
        ///
        /// # Default values
        ///
        /// See trait documentation for more details.
        ///
        /// # Errors
        /// Returns error if part of the config is invalid.
        fn from_env(prefix: &str) -> Result<Self>
        where
            Self: Sized;
    }

    impl<'a, T> FigmentExt for T
    where
        T: Deserialize<'a> + ConfigDefault,
    {
        fn from_env(prefix: &str) -> Result<Self> {
            Ok(Figment::from(Serialized::defaults(Self::config_defaults()))
                .merge(Env::prefixed(prefix).split("__"))
                .extract()?)
        }
    }

    #[doc(hidden)]
    pub trait ConfigDefault {
        fn config_defaults() -> serde_json::Value;
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use figment::Jail;
    use serde::{Deserialize, Serialize};
    use tokio::task::yield_now;
    use tokio::time::sleep;

    use core_derive::Config;

    use crate::utils::FigmentExt;
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

    #[derive(Deserialize, Config)]
    #[config(core = "crate")]
    struct TestConfig {
        kind: String,
        #[config(default = "10")]
        age: usize,
        enabled: bool,
        #[serde(with = "humantime_serde")]
        #[config(default_str = "10s")]
        delay: Duration,
        #[config(default)]
        nested_a: TestNested,
        #[config(default = r#"{"b": 1}"#)]
        nested_b: TestNested,
    }

    #[derive(Serialize, Deserialize, Default)]
    struct TestNested {
        a: bool,
        b: usize,
    }

    #[test]
    fn must_from_env() {
        Jail::expect_with(|jail| {
            jail.set_env("TEST_KIND", "test");
            jail.set_env("TEST_AGE", "42");
            jail.set_env("TEST_ENABLED", "true");
            jail.set_env("TEST_NESTED_A__A", "false");
            jail.set_env("TEST_NESTED_B__A", "true");

            let config = TestConfig::from_env("TEST_").unwrap();

            let TestConfig {
                kind,
                age,
                enabled,
                delay,
                nested_a,
                nested_b,
            } = config;
            assert_eq!(kind, "test");
            assert_eq!(age, 42);
            assert!(enabled);
            assert_eq!(delay, Duration::from_secs(10));

            let TestNested { a, b } = nested_a;
            assert!(!a);
            assert_eq!(b, 0);

            let TestNested { a, b } = nested_b;
            assert!(a);
            assert_eq!(b, 1);

            Ok(())
        });
    }
}
