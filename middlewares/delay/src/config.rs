//! Translate middleware config.

use eyre::Result;
use figment::providers::{Env, Serialized};
use figment::Figment;
use serde::{Deserialize, Serialize};

/// Coordinator config.
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct Config {
    /// AMQP connection url.
    pub amqp_url: String,
    /// AMQP exchange name.
    pub amqp_exchange: String,
    /// Database connection url.
    pub database_url: String,
}

impl Config {
    /// Load config from environment variables.
    ///
    /// # Errors
    /// Returns error if part of the config is invalid.
    pub fn from_env() -> Result<Self> {
        Ok(Figment::from(Serialized::defaults(Self::default()))
            .merge(Env::prefixed("MIDDLEWARE_"))
            .extract()?)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            amqp_url: String::from("amqp://guest:guest@localhost:5672"),
            amqp_exchange: String::from("stargazer-reborn"),
            database_url: "db.sqlite".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
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
            jail.set_env("MIDDLEWARE_AMQP_URL", "amqp://admin:admin@localhost:5672");
            jail.set_env("MIDDLEWARE_AMQP_EXCHANGE", "some_exchange");
            jail.set_env(
                "MIDDLEWARE_DATABASE_URL",
                "mysql://guest:guest@localhost/test",
            );
            assert_eq!(
                Config::from_env().unwrap(),
                Config {
                    amqp_url: String::from("amqp://admin:admin@localhost:5672"),
                    amqp_exchange: String::from("some_exchange"),
                    database_url: String::from("mysql://guest:guest@localhost/test"),
                }
            );
            Ok(())
        });
    }
}
