//! Translate middleware config.

use eyre::Result;
use figment::providers::{Env, Serialized};
use figment::Figment;
use serde::{Deserialize, Serialize};

/// Middleware config.
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct Config {
    /// AMQP connection url.
    pub amqp_url: String,
    /// AMQP exchange name.
    pub amqp_exchange: String,
    /// Baidu translate app id.
    pub baidu_app_id: usize,
    /// Baidu translate app secret.
    pub baidu_app_secret: String,
    /// Debug only.
    pub debug: bool,
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
            baidu_app_id: 0,
            baidu_app_secret: String::new(),
            debug: false,
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
            jail.set_env("MIDDLEWARE_BAIDU_APP_ID", "1");
            jail.set_env("MIDDLEWARE_BAIDU_APP_SECRET", "<secret>");
            jail.set_env("MIDDLEWARE_DEBUG", "true");
            assert_eq!(
                Config::from_env().unwrap(),
                Config {
                    amqp_url: String::from("amqp://admin:admin@localhost:5672"),
                    amqp_exchange: String::from("some_exchange"),
                    baidu_app_id: 1,
                    baidu_app_secret: String::from("<secret>"),
                    debug: true,
                }
            );
            Ok(())
        });
    }
}
