//! Translate middleware config.

use color_eyre::Result;
use figment::providers::Env;
use figment::Figment;
use reqwest::Url;
use serde::{Deserialize, Serialize};

mod default;

/// Coordinator config.
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct Config {
    /// AMQP connection url.
    #[serde(default = "default::amqp_url")]
    pub amqp_url: String,
    /// AMQP exchange name.
    #[serde(default = "default::amqp_exchange")]
    pub amqp_exchange: String,
    /// Api url.
    #[serde(default = "default::api_url")]
    pub api_url: Url,
    /// Api username.
    pub api_username: String,
    /// Api password.
    pub api_password: String,
    /// Telegram bot token.
    pub tg_token: String,
}

impl Config {
    /// Load config from environment variables.
    ///
    /// # Errors
    /// Returns error if part of the config is invalid.
    pub fn from_env() -> Result<Self> {
        Ok(Figment::from(Env::prefixed("BOT_")).extract()?)
    }
}

#[cfg(test)]
mod tests {
    use figment::Jail;
    use reqwest::Url;

    use crate::config::Config;

    #[test]
    fn test_default() {
        Jail::expect_with(|jail| {
            jail.set_env("BOT_AMQP_URL", "amqp://admin:admin@localhost:5672");
            jail.set_env("BOT_AMQP_EXCHANGE", "some_exchange");
            jail.set_env("BOT_API_URL", "http://localhost:8080");
            jail.set_env("BOT_API_USERNAME", "admin");
            jail.set_env("BOT_API_PASSWORD", "admin");
            jail.set_env("BOT_TG_TOKEN", "some_token");
            assert_eq!(
                Config::from_env().unwrap(),
                Config {
                    amqp_url: String::from("amqp://admin:admin@localhost:5672"),
                    amqp_exchange: String::from("some_exchange"),
                    api_url: Url::parse("http://localhost:8080").unwrap(),
                    api_username: String::from("admin"),
                    api_password: String::from("admin"),
                    tg_token: String::from("some_token"),
                }
            );
            Ok(())
        });
    }

    #[test]
    fn must_from_env() {
        Jail::expect_with(|jail| {
            jail.set_env("BOT_AMQP_URL", "amqp://admin:admin@localhost:5672");
            jail.set_env("BOT_AMQP_EXCHANGE", "some_exchange");
            jail.set_env("BOT_API_URL", "http://localhost:8080");
            jail.set_env("BOT_API_USERNAME", "admin");
            jail.set_env("BOT_API_PASSWORD", "admin");
            jail.set_env("BOT_TG_TOKEN", "some_token");
            assert_eq!(
                Config::from_env().unwrap(),
                Config {
                    amqp_url: String::from("amqp://admin:admin@localhost:5672"),
                    amqp_exchange: String::from("some_exchange"),
                    api_url: Url::parse("http://localhost:8080").unwrap(),
                    api_username: String::from("admin"),
                    api_password: String::from("admin"),
                    tg_token: String::from("some_token"),
                }
            );
            Ok(())
        });
    }
}
