//! Coordinator config.

use std::time::Duration;

use figment::providers::{Env, Serialized};
use figment::Figment;
use serde::{Deserialize, Serialize};

/// Coordinator config.
#[derive(Debug, Copy, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct Config {
    /// Determine how often coordinator sends ping to workers.
    #[serde(with = "humantime_serde")]
    pub ping_interval: Duration,
}

impl Config {
    /// Load config from environment variables.
    #[allow(clippy::missing_panics_doc)]
    #[must_use]
    pub fn from_env() -> Self {
        Figment::from(Serialized::defaults(Self::default()))
            .merge(Env::prefixed("COORDINATOR_"))
            .extract()
            .unwrap()
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            ping_interval: Duration::from_secs(10),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use figment::Jail;

    use crate::config::Config;

    #[test]
    fn must_default() {
        Jail::expect_with(|_| {
            assert_eq!(Config::from_env(), Config::default());
            Ok(())
        });
    }

    #[test]
    fn must_from_env() {
        Jail::expect_with(|jail| {
            jail.set_env("COORDINATOR_PING_INTERVAL", "1s");
            assert_eq!(
                Config::from_env(),
                Config {
                    ping_interval: Duration::from_secs(1),
                }
            );
            Ok(())
        });
    }
}
