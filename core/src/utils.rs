//! Utility structs and functions.

use std::ops::{Deref, DerefMut};

#[cfg(any(feature = "core_derive", test))]
pub use core_derive::Config;
#[cfg(any(feature = "figment", test))]
pub use figment_ext::*;
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
    ($k:expr, $v:expr) => {{
        let mut map = serde_json::Map::new();
        map.insert($k.into(), Value::String($v.into()));
        map
    }};
}

pub(crate) use map;

#[cfg(any(feature = "figment", test))]
mod figment_ext {
    use eyre::Result;
    use figment::{
        providers::{Env, Serialized},
        Figment,
    };
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
    /// use std::time::Duration;
    ///
    /// use core_derive::Config;
    /// use serde::Deserialize;
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
    ///     // Types annotated with `#[config(default)]` must implement `Serialize`.
    ///     #[config(default)]
    ///     delay: Duration,
    ///     // Partial default assignments are allowed,
    ///     // as long as given values are valid json literals.
    ///     #[config(default = r#"{ "a": 42 }"#)]
    ///     nested: Nested,
    /// }
    ///
    /// #[derive(Deserialize)]
    /// struct Nested {
    ///     a: usize,
    ///     b: usize,
    /// }
    /// ```
    /// # Inherit default values from nested field
    /// To inherit default config values from nested field, use the `inherit`
    /// attribute.
    ///
    /// ```rust
    /// use std::time::Duration;
    ///
    /// use core_derive::Config;
    /// use serde::Deserialize;
    ///
    /// // Override crate name for core crate if its name is not `sg_core`.
    /// // E.g. `#[config(core = "crate_name")]`
    /// #[derive(Deserialize, Config)]
    /// # #[config(core = "crate")]
    /// struct Config {
    ///     // Default values are inherited from derived `Config` of given struct.
    ///     #[config(inherit)]
    ///     nested: Nested,
    ///     // Partial default assignments take precedence over inherited values.
    ///     #[config(inherit, default = r#"{ "answer": 114514 }"#)]
    ///     nested_2: Nested,
    ///     // `inherit(flatten)` can be used in combination with `serde(flatten)`.
    ///     #[serde(flatten)]
    ///     #[config(inherit(flatten))]
    ///     flatten: Nested,
    /// }
    ///
    /// #[derive(Deserialize, Config)]
    /// struct Nested {
    ///     #[config(default = "42")]
    ///     answer: usize,
    ///     age: Nested,
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

    use core_derive::Config;
    use figment::Jail;
    use serde::Deserialize;
    use tokio::{task::yield_now, time::sleep};

    use crate::utils::{FigmentExt, ScopedJoinHandle};

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
    struct ConfigWithNoDefaults {
        a: String,
        b: usize,
    }

    #[test]
    fn must_config_with_no_defaults() {
        Jail::expect_with(|jail| {
            jail.set_env("TEST_A", "test");
            jail.set_env("TEST_B", "42");

            let config = ConfigWithNoDefaults::from_env("TEST_").unwrap();

            let ConfigWithNoDefaults { a, b } = config;
            assert_eq!(a, "test");
            assert_eq!(b, 42);

            Ok(())
        });
    }

    #[derive(Deserialize, Config)]
    #[config(core = "crate")]
    struct ConfigWithExplicitDefaults {
        a: String,
        #[config(default = "42")]
        b: usize,
    }

    #[test]
    fn must_config_with_explicit_defaults() {
        Jail::expect_with(|jail| {
            jail.set_env("TEST_A", "test");

            let config = ConfigWithExplicitDefaults::from_env("TEST_").unwrap();

            let ConfigWithExplicitDefaults { a, b } = config;
            assert_eq!(a, "test");
            assert_eq!(b, 42);

            Ok(())
        });
    }

    #[test]
    fn must_override_config_with_explicit_defaults() {
        Jail::expect_with(|jail| {
            jail.set_env("TEST_A", "test");
            jail.set_env("TEST_B", "0");

            let config = ConfigWithExplicitDefaults::from_env("TEST_").unwrap();

            let ConfigWithExplicitDefaults { a, b } = config;
            assert_eq!(a, "test");
            assert_eq!(b, 0);

            Ok(())
        });
    }

    #[derive(Deserialize, Config)]
    #[config(core = "crate")]
    struct ConfigWithStrDefaults {
        #[config(default_str = "test")]
        a: String,
        b: usize,
    }

    #[test]
    fn must_config_with_str_defaults() {
        Jail::expect_with(|jail| {
            jail.set_env("TEST_B", "42");

            let config = ConfigWithStrDefaults::from_env("TEST_").unwrap();

            let ConfigWithStrDefaults { a, b } = config;
            assert_eq!(a, "test");
            assert_eq!(b, 42);

            Ok(())
        });
    }

    #[derive(Deserialize, Config)]
    #[config(core = "crate")]
    struct ConfigWithCustomDeserialize {
        #[serde(with = "humantime_serde")]
        #[config(default_str = "10s")]
        delay: Duration,
    }

    #[test]
    fn must_config_with_custom_defaults() {
        Jail::expect_with(|_| {
            let config = ConfigWithCustomDeserialize::from_env("TEST_").unwrap();

            let ConfigWithCustomDeserialize { delay } = config;
            assert_eq!(delay, Duration::from_secs(10));

            Ok(())
        });
    }

    #[test]
    fn must_override_config_with_custom_defaults() {
        Jail::expect_with(|jail| {
            jail.set_env("TEST_DELAY", "1s");

            let config = ConfigWithCustomDeserialize::from_env("TEST_").unwrap();

            let ConfigWithCustomDeserialize { delay } = config;
            assert_eq!(delay, Duration::from_secs(1));

            Ok(())
        });
    }

    #[derive(Deserialize, Config)]
    #[config(core = "crate")]
    struct ConfigWithImplicitDefaults {
        #[config(default)]
        a: usize,
    }

    #[test]
    fn must_config_with_implicit_defaults() {
        Jail::expect_with(|_| {
            let config = ConfigWithImplicitDefaults::from_env("TEST_").unwrap();

            let ConfigWithImplicitDefaults { a } = config;
            assert_eq!(a, 0);

            Ok(())
        });
    }

    #[test]
    fn must_override_config_with_implicit_defaults() {
        Jail::expect_with(|jail| {
            jail.set_env("TEST_A", "42");

            let config = ConfigWithImplicitDefaults::from_env("TEST_").unwrap();

            let ConfigWithImplicitDefaults { a } = config;
            assert_eq!(a, 42);

            Ok(())
        });
    }

    #[derive(Deserialize, Config)]
    #[config(core = "crate")]
    struct Nested {
        #[config(default = "false")]
        b: bool,
        c: usize,
    }

    #[derive(Deserialize, Config)]
    #[config(core = "crate")]
    struct ConfigWithStructDefaults {
        #[config(default = r#"{ "c": 42 }"#)]
        a: Nested,
    }

    #[test]
    fn must_config_with_struct_defaults() {
        Jail::expect_with(|jail| {
            jail.set_env("TEST_A__B", "true");

            let config = ConfigWithStructDefaults::from_env("TEST_").unwrap();

            let ConfigWithStructDefaults { a: Nested { b, c } } = config;
            assert!(b);
            assert_eq!(c, 42);

            Ok(())
        });
    }

    #[derive(Deserialize, Config)]
    #[config(core = "crate")]
    struct ConfigWithInheritDefaults {
        #[config(inherit)]
        a: Nested,
    }

    #[test]
    fn must_config_with_inherit_defaults() {
        Jail::expect_with(|jail| {
            jail.set_env("TEST_A__C", "42");

            let config = ConfigWithInheritDefaults::from_env("TEST_").unwrap();

            let ConfigWithInheritDefaults { a: Nested { b, c } } = config;
            assert!(!b);
            assert_eq!(c, 42);

            Ok(())
        });
    }

    #[derive(Deserialize, Config)]
    #[config(core = "crate")]
    struct ConfigWithInheritAndExplicitDefaults {
        #[config(inherit, default = r#"{ "b": true }"#)]
        a: Nested,
    }

    #[test]
    fn must_config_with_inherit_and_explicit_defaults() {
        Jail::expect_with(|jail| {
            jail.set_env("TEST_A__C", "42");

            let config = ConfigWithInheritAndExplicitDefaults::from_env("TEST_").unwrap();

            let ConfigWithInheritAndExplicitDefaults { a: Nested { b, c } } = config;
            assert!(b);
            assert_eq!(c, 42);

            Ok(())
        });
    }

    #[derive(Deserialize, Config)]
    #[config(core = "crate")]
    struct ConfigWithInheritAndExplicitDefaultsTwo {
        #[config(inherit, default = r#"{ "c": 42 }"#)]
        a: Nested,
    }

    #[test]
    fn must_config_with_inherit_and_explicit_defaults_2() {
        Jail::expect_with(|_| {
            let config = ConfigWithInheritAndExplicitDefaultsTwo::from_env("TEST_").unwrap();

            let ConfigWithInheritAndExplicitDefaultsTwo { a: Nested { b, c } } = config;
            assert!(!b);
            assert_eq!(c, 42);

            Ok(())
        });
    }

    #[derive(Deserialize, Config)]
    #[config(core = "crate")]
    struct ConfigWithFlattenInheritDefaults {
        d: usize,
        #[serde(flatten)]
        #[config(inherit(flatten))]
        a: Nested,
    }

    #[test]
    fn must_config_with_flatten_inherit_defaults() {
        Jail::expect_with(|jail| {
            jail.set_env("TEST_C", "41");
            jail.set_env("TEST_D", "42");

            let config = ConfigWithFlattenInheritDefaults::from_env("TEST_").unwrap();

            let ConfigWithFlattenInheritDefaults {
                d,
                a: Nested { b, c },
            } = config;
            assert!(!b);
            assert_eq!(c, 41);
            assert_eq!(d, 42);

            Ok(())
        });
    }
}
