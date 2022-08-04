//! Translate middleware config.

use serde::{Deserialize, Serialize};

use sg_core::utils::Config;

/// Coordinator config.
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Config)]
pub struct Config {
    /// AMQP connection url.
    #[config(default_str = "amqp://guest:guest@localhost:5672")]
    pub amqp_url: String,
    /// AMQP exchange name.
    #[config(default_str = "stargazer-reborn")]
    pub amqp_exchange: String,
    /// Database connection url.
    #[config(default_str = "db.sqlite")]
    pub database_url: String,
}

#[cfg(test)]
mod tests {
    use figment::Jail;

    use sg_core::utils::FigmentExt;

    use crate::config::Config;

    #[test]
    fn must_default() {
        Jail::expect_with(|_| {
            assert_eq!(
                Config::from_env("MIDDLEWARE_").unwrap(),
                Config {
                    amqp_url: String::from("amqp://guest:guest@localhost:5672"),
                    amqp_exchange: String::from("stargazer-reborn"),
                    database_url: "db.sqlite".to_string(),
                }
            );
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
                Config::from_env("MIDDLEWARE_").unwrap(),
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
