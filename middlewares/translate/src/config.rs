//! Translate middleware config.

use serde::{Deserialize, Serialize};
use sg_core::utils::Config;

/// Middleware config.
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Config)]
pub struct Config {
    /// AMQP connection url.
    #[config(default_str = "amqp://guest:guest@localhost:5672")]
    pub amqp_url: String,
    /// AMQP exchange name.
    #[config(default_str = "stargazer-reborn")]
    pub amqp_exchange: String,
    /// Baidu translate app id.
    pub baidu_app_id: usize,
    /// Baidu translate app secret.
    pub baidu_app_secret: String,
    /// Debug only.
    #[config(default = "false")]
    pub debug: bool,
}

#[cfg(test)]
mod tests {
    use figment::Jail;
    use sg_core::utils::FigmentExt;

    use crate::config::Config;

    #[test]
    fn must_default() {
        Jail::expect_with(|jail| {
            jail.set_env("MIDDLEWARE_BAIDU_APP_ID", "0");
            jail.set_env("MIDDLEWARE_BAIDU_APP_SECRET", "");
            assert_eq!(
                Config::from_env("MIDDLEWARE_").unwrap(),
                Config {
                    amqp_url: String::from("amqp://guest:guest@localhost:5672"),
                    amqp_exchange: String::from("stargazer-reborn"),
                    baidu_app_id: 0,
                    baidu_app_secret: String::new(),
                    debug: false,
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
            jail.set_env("MIDDLEWARE_BAIDU_APP_ID", "1");
            jail.set_env("MIDDLEWARE_BAIDU_APP_SECRET", "<secret>");
            jail.set_env("MIDDLEWARE_DEBUG", "true");
            assert_eq!(
                Config::from_env("MIDDLEWARE_").unwrap(),
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
