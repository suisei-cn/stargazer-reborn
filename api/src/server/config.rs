//! API config.

use std::net::SocketAddr;
use std::time::Duration;

use color_eyre::Result;
use figment::providers::{Env, Serialized};
use figment::Figment;
use serde::{Deserialize, Serialize};

/// Runtime configuration.
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
    /// Secret password used to authenticate API requests from bot. This is also used to sign JWT tokens.
    pub bot_password: String,
    /// MongoDB collection name for `Users`.
    pub users_collection: String,
    /// MongoDB collection name for `Tasks`.
    pub tasks_collection: String,
    /// MongoDB collection name for `VTBs`.
    pub entities_collection: String,
    /// MongoDB collection name for `VTBs`.
    pub groups_collection: String,
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

/// API config defaults.
///
/// **THIS SHOULD NOT BE USED IN PRODUCTION**,
/// use [`Config::from_env()`] and pass in custom value from environment instead.
impl Default for Config {
    fn default() -> Self {
        Self {
            bind: "127.0.0.1:8000".parse().unwrap(),
            session_timeout: Duration::from_secs(10 * 60),
            mongo_uri: String::from("mongodb://localhost:27017"),
            mongo_db: String::from("stargazer-reborn"),
            bot_password: String::from("TEST"),
            users_collection: String::from("users"),
            tasks_collection: String::from("tasks"),
            entities_collection: String::from("entities"),
            groups_collection: String::from("groups"),
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
            jail.set_env("API_BOT_PASSWORD", "password");
            jail.set_env("API_USERS_COLLECTION", "u");
            jail.set_env("API_TASKS_COLLECTION", "t");
            jail.set_env("API_ENTITIES_COLLECTION", "e");
            jail.set_env("API_GROUPS_COLLECTION", "g");
            assert_eq!(
                Config::from_env().unwrap(),
                Config {
                    bind: "0.0.0.0:8080".parse().unwrap(),
                    session_timeout: Duration::from_secs(60 * 10),
                    mongo_uri: String::from("mongodb://suichan:27017"),
                    mongo_db: String::from("db"),
                    bot_password: String::from("password"),
                    users_collection: String::from("u"),
                    tasks_collection: String::from("t"),
                    entities_collection: String::from("e"),
                    groups_collection: String::from("g"),
                }
            );
            Ok(())
        });
    }
}
