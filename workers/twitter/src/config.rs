//! Twitter worker config.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use sg_core::utils::Config;

/// Coordinator config.
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Config)]
pub struct Config {
    /// Unique worker ID.
    #[config(default)]
    pub id: Uuid,
    /// AMQP connection url.
    #[config(default_str = "amqp://guest:guest@localhost:5672")]
    pub amqp_url: String,
    /// AMQP exchange name.
    #[config(default_str = "stargazer-reborn")]
    pub amqp_exchange: String,
    /// The coordinator url to connect to.
    #[config(default_str = "ws://127.0.0.1:7000")]
    pub coordinator_url: String,
    /// Twitter API token.
    pub twitter_token: String,
    /// Interval between twitter polls.
    #[serde(with = "humantime_serde")]
    #[config(default_str = "60s")]
    pub poll_interval: Duration,
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use figment::Jail;
    use uuid::Uuid;

    use sg_core::utils::FigmentExt;

    use crate::config::Config;

    #[test]
    fn must_default() {
        Jail::expect_with(|jail| {
            jail.set_env("WORKER_TWITTER_TOKEN", "");
            assert_eq!(
                Config::from_env("WORKER_").unwrap(),
                Config {
                    id: Uuid::nil(),
                    amqp_url: String::from("amqp://guest:guest@localhost:5672"),
                    amqp_exchange: String::from("stargazer-reborn"),
                    coordinator_url: String::from("ws://127.0.0.1:7000"),
                    twitter_token: String::new(),
                    poll_interval: Duration::from_secs(60),
                }
            );
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
            jail.set_env("WORKER_TWITTER_TOKEN", "blabla");
            jail.set_env("WORKER_POLL_INTERVAL", "30s");
            assert_eq!(
                Config::from_env("WORKER_").unwrap(),
                Config {
                    id,
                    amqp_url: String::from("amqp://admin:admin@localhost:5672"),
                    amqp_exchange: String::from("some_exchange"),
                    coordinator_url: String::from("ws://localhost:8080"),
                    twitter_token: String::from("blabla"),
                    poll_interval: Duration::from_secs(30),
                }
            );
            Ok(())
        });
    }
}
