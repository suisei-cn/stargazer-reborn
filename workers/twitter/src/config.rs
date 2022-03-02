//! Twitter worker config.

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
            coordinator_url: String::from("ws://localhost:7000"),
            twitter_token: String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
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
            assert_eq!(
                Config::from_env().unwrap(),
                Config {
                    id,
                    amqp_url: String::from("amqp://admin:admin@localhost:5672"),
                    coordinator_url: String::from("ws://localhost:8080"),
                    twitter_token: String::from("blabla"),
                }
            );
            Ok(())
        });
    }
}
