//! API config.

use std::net::SocketAddr;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use sg_core::utils::Config;

/// Runtime configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Config)]
pub struct Config {
    /// Bind address for API server.
    #[config(default_str = "127.0.0.1:8000")]
    pub bind: SocketAddr,
    /// Duration the session(token) is valid.
    #[serde(with = "humantime_serde")]
    #[config(default_str = "10m")]
    pub token_timeout: Duration,
    /// MongoDB connection string.
    #[config(default_str = "mongodb://localhost:27017")]
    pub mongo_uri: String,
    /// MongoDB database name.
    #[config(default_str = "stargazer-reborn")]
    pub mongo_db: String,
    /// Secret used to sign JWT tokens.
    pub jwt_secret: String,
    /// MongoDB collection name for `Users`.
    #[config(default_str = "users")]
    pub users_collection: String,
    /// MongoDB collection name for `Tasks`.
    #[config(default_str = "tasks")]
    pub tasks_collection: String,
    /// MongoDB collection name for `VTBs`.
    #[config(default_str = "entities")]
    pub entities_collection: String,
    /// MongoDB collection name for `Groups`.
    #[config(default_str = "groups")]
    pub groups_collection: String,
    /// MongoDB collection name for `Auth`.
    #[config(default_str = "auth")]
    pub auth_collection: String,
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use figment::Jail;

    use sg_core::utils::FigmentExt;

    use crate::server::Config;

    #[test]
    fn must_default() {
        Jail::expect_with(|jail| {
            jail.set_env("API_JWT_SECRET", "TEST");
            assert_eq!(
                Config::from_env("API_").unwrap(),
                Config {
                    bind: "127.0.0.1:8000".parse().unwrap(),
                    token_timeout: Duration::from_secs(10 * 60),
                    mongo_uri: String::from("mongodb://localhost:27017"),
                    mongo_db: String::from("stargazer-reborn"),
                    jwt_secret: String::from("TEST"),
                    users_collection: String::from("users"),
                    tasks_collection: String::from("tasks"),
                    entities_collection: String::from("entities"),
                    groups_collection: String::from("groups"),
                    auth_collection: String::from("auth"),
                }
            );
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
            jail.set_env("API_AUTH_COLLECTION", "a");
            assert_eq!(
                Config::from_env("API_").unwrap(),
                Config {
                    bind: "0.0.0.0:8080".parse().unwrap(),
                    token_timeout: Duration::from_secs(60 * 10),
                    mongo_uri: String::from("mongodb://suichan:27017"),
                    mongo_db: String::from("db"),
                    jwt_secret: String::from("password"),
                    users_collection: String::from("u"),
                    tasks_collection: String::from("t"),
                    entities_collection: String::from("e"),
                    groups_collection: String::from("g"),
                    auth_collection: String::from("a"),
                }
            );
            Ok(())
        });
    }
}
