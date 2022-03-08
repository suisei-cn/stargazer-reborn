//! API config.

use std::net::SocketAddr;
use std::time::Duration;

use color_eyre::Result;
use figment::providers::{Env, Serialized};
use figment::Figment;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

static CONFIG: Lazy<Config> = Lazy::new(|| Config::from_env().expect("Failed to load config"));

pub fn get_config() -> &'static Config {
    &CONFIG
}

/// API config.
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct Config {
    /// Bind address for API server.
    pub bind: SocketAddr,
    /// Duration til session is timed out.
    #[serde(with = "humantime_serde")]
    pub session_timeout: Duration,
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
            .merge(Env::prefixed("API_"))
            .extract()?)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bind: "127.0.0.1:8000".parse().unwrap(),
            session_timeout: Duration::from_secs(10 * 60),
            mongo_uri: String::from("mongodb://localhost:27017"),
            mongo_db: String::from("stargazer-reborn"),
            mongo_collection: String::from("api_sessions"),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use figment::Jail;

    use crate::server::Config;

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
            jail.set_env("API_BIND", "0.0.0.0:8080");
            jail.set_env("API_SESSION_TIMEOUT", "10m");
            jail.set_env("API_MONGO_URI", "mongodb://suichan:27017");
            jail.set_env("API_MONGO_DB", "db");
            jail.set_env("API_MONGO_COLLECTION", "coll");
            assert_eq!(
                Config::from_env().unwrap(),
                Config {
                    bind: "0.0.0.0:8080".parse().unwrap(),
                    session_timeout: Duration::from_secs(60 * 10),
                    mongo_uri: String::from("mongodb://suichan:27017"),
                    mongo_db: String::from("db"),
                    mongo_collection: String::from("coll"),
                }
            );
            Ok(())
        });
    }
}
