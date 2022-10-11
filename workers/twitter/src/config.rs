//! Twitter worker config.

use std::time::Duration;

use base::NodeConfig;
use serde::Deserialize;
use sg_core::utils::Config;

/// Coordinator config.
#[derive(Debug, Clone, Deserialize, Config)]
pub struct Config {
    /// AMQP connection url.
    #[config(default_str = "amqp://guest:guest@localhost:5672")]
    pub amqp_url: String,
    /// AMQP exchange name.
    #[config(default_str = "stargazer-reborn")]
    pub amqp_exchange: String,
    /// Node configuration.
    #[config(inherit, default = r#"{"kind": "twitter"}"#)]
    pub node_config: NodeConfig,
    /// Twitter API token.
    pub twitter_token: String,
    /// Interval between twitter polls.
    #[serde(with = "humantime_serde")]
    #[config(default_str = "60s")]
    pub poll_interval: Duration,
}
