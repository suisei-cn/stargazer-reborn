//! Youtube worker config.

use std::net::SocketAddr;
use std::time::Duration;

use eyre::Result;
use figment::providers::{Env, Serialized};
use figment::Figment;
use serde::{Deserialize, Serialize};
use url::Url;
use uuid::Uuid;

/// Coordinator config.
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct Config {
    /// Unique worker ID.
    pub id: Uuid,
    /// AMQP connection url.
    pub amqp_url: String,
    /// AMQP exchange name.
    pub amqp_exchange: String,
    /// The coordinator url to connect to.
    pub coordinator_url: String,
    /// The bind address for the worker.
    pub bind: SocketAddr,
    /// The callback base url for the worker.
    pub base_url: Url,
    /// Lease of each subscription.
    #[serde(with = "humantime_serde")]
    pub lease: Duration,
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
            amqp_exchange: String::from("stargazer-reborn"),
            coordinator_url: String::from("ws://127.0.0.1:7000"),
            bind: "0.0.0.0:8080".parse().unwrap(),
            base_url: "https://example.com".parse().unwrap(),
            lease: Duration::from_secs(43200),
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
            jail.set_env("WORKER_AMQP_EXCHANGE", "some_exchange");
            jail.set_env("WORKER_COORDINATOR_URL", "ws://localhost:8080");
            jail.set_env("WORKER_BIND", "0.0.0.0:8000");
            jail.set_env("WORKER_BASE_URL", "https://suisei.dev");
            jail.set_env("WORKER_LEASE", "1d");
            assert_eq!(
                Config::from_env().unwrap(),
                Config {
                    id,
                    amqp_url: String::from("amqp://admin:admin@localhost:5672"),
                    amqp_exchange: String::from("some_exchange"),
                    coordinator_url: String::from("ws://localhost:8080"),
                    bind: "0.0.0.0:8000".parse().unwrap(),
                    base_url: "https://suisei.dev".parse().unwrap(),
                    lease: Duration::from_secs(86400),
                }
            );
            Ok(())
        });
    }
}
