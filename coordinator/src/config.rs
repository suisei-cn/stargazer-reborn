//! Coordinator config.

use std::net::SocketAddr;
use std::time::Duration;

use eyre::Result;
use figment::providers::{Env, Serialized};
use figment::Figment;
use serde::{Deserialize, Serialize};

/// Coordinator config.
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct Config {
    /// Bind address for coordinator.
    pub bind: SocketAddr,
    /// Determine how often coordinator sends ping to workers.
    #[serde(with = "humantime_serde")]
    pub ping_interval: Duration,
    /// MongoDB connection string.
    pub mongo_uri: String,
    /// MongoDB database name.
    pub mongo_db: String,
    /// MongoDB collection name.
    pub mongo_collection: String,
}

impl Config {
    /// Load config from environment variables.
    ///
    /// # Errors
    /// Returns error if part of the config is invalid.
    pub fn from_env() -> Result<Self> {
        Ok(Figment::from(Serialized::defaults(Self::default()))
            .merge(Env::prefixed("COORDINATOR_"))
            .extract()?)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bind: "127.0.0.1:7000".parse().unwrap(),
            ping_interval: Duration::from_secs(10),
            mongo_uri: String::from("mongodb://localhost:27017"),
            mongo_db: String::from("stargazer-reborn"),
            mongo_collection: String::from("tasks"),
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
            assert_eq!(Config::from_env().unwrap(), Config::default());
            Ok(())
        });
    }

    #[test]
    fn must_from_env() {
        Jail::expect_with(|jail| {
            jail.set_env("COORDINATOR_BIND", "0.0.0.0:8080");
            jail.set_env("COORDINATOR_PING_INTERVAL", "1s");
            jail.set_env("COORDINATOR_MONGO_URI", "mongodb://suichan:27017");
            jail.set_env("COORDINATOR_MONGO_DB", "db");
            jail.set_env("COORDINATOR_MONGO_COLLECTION", "coll");
            assert_eq!(
                Config::from_env().unwrap(),
                Config {
                    bind: "0.0.0.0:8080".parse().unwrap(),
                    ping_interval: Duration::from_secs(1),
                    mongo_uri: String::from("mongodb://suichan:27017"),
                    mongo_db: String::from("db"),
                    mongo_collection: String::from("coll"),
                }
            );
            Ok(())
        });
    }
}
