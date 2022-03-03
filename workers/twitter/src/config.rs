//! Twitter worker config.

use std::time::Duration;

use eyre::Result;
use figment::providers::{Env, Serialized};
use figment::Figment;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Coordinator config.
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct Config {
    /// Unique worker ID.
    pub id: Uuid,
    /// AMQP connection url.
    pub amqp_url: String,
    /// The coordinator url to connect to.
    pub coordinator_url: String,
    /// Twitter API token.
    pub twitter_token: String,
    /// Interval between twitter polls.
    #[serde(with = "humantime_serde")]
    pub poll_interval: Duration,
}

impl Config {
    /// Load config from environment variables.
    ///
    /// # Errors
    /// Returns error if part of the config is invalid.
    pub fn from_env() -> Result<Self> {
        Ok(Figment::from(Serialized::defaults(Self::default()))
            .merge(Env::prefixed("WORKER_"))
            .extract()?)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            id: Uuid::nil(),
            amqp_url: String::from("amqp://guest:guest@localhost:5672"),
            coordinator_url: String::from("ws://127.0.0.1:7000"),
            twitter_token: String::new(),
            poll_interval: Duration::from_secs(60),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use figment::Jail;
    use uuid::Uuid;

    use crate::config::Config;

    #[test]
    fn must_default() {
        Jail::expect_with(|_| {
            assert_eq!(Config::from_env().unwrap(), Config::default());
            Ok(())
        });
    }

    #[test]
    fn must_from_env() {
        Jail::expect_with(|jail| {
            let id = Uuid::from_u128(1);
            jail.set_env("WORKER_ID", &id);
            jail.set_env("WORKER_AMQP_URL", "amqp://admin:admin@localhost:5672");
            jail.set_env("WORKER_COORDINATOR_URL", "ws://localhost:8080");
            jail.set_env("WORKER_TWITTER_TOKEN", "blabla");
            jail.set_env("WORKER_POLL_INTERVAL", "30s");
            assert_eq!(
                Config::from_env().unwrap(),
                Config {
                    id,
                    amqp_url: String::from("amqp://admin:admin@localhost:5672"),
                    coordinator_url: String::from("ws://localhost:8080"),
                    twitter_token: String::from("blabla"),
                    poll_interval: Duration::from_secs(30),
                }
            );
            Ok(())
        });
    }
}
