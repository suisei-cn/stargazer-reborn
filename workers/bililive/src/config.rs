//! Twitter worker config.

use serde::{Deserialize, Serialize};
use sg_core::utils::Config;
use uuid::Uuid;

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
}

#[cfg(test)]
mod tests {
    use figment::Jail;
    use sg_core::utils::FigmentExt;
    use uuid::Uuid;

    use crate::config::Config;

    #[test]
    fn must_default() {
        Jail::expect_with(|_| {
            assert_eq!(
                Config::from_env("WORKER_").unwrap(),
                Config {
                    id: Uuid::nil(),
                    amqp_url: String::from("amqp://guest:guest@localhost:5672"),
                    amqp_exchange: String::from("stargazer-reborn"),
                    coordinator_url: String::from("ws://127.0.0.1:7000"),
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
            assert_eq!(
                Config::from_env("WORKER_").unwrap(),
                Config {
                    id,
                    amqp_url: String::from("amqp://admin:admin@localhost:5672"),
                    amqp_exchange: String::from("some_exchange"),
                    coordinator_url: String::from("ws://localhost:8080"),
                }
            );
            Ok(())
        });
    }
}
