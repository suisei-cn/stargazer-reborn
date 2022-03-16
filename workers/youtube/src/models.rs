use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Deserialize)]
pub struct ChallengeQuery {
    #[serde(rename = "hub.topic")]
    pub topic: Url,
    #[serde(rename = "hub.mode")]
    pub mode: Mode,
    #[serde(rename = "hub.challenge")]
    pub challenge: String,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Subscribe,
    Unsubscribe,
}

#[derive(Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Verify {
    Async,
    // Sync,
}

#[derive(Serialize)]
pub struct SubscribeForm {
    #[serde(rename = "hub.callback")]
    pub callback: String,
    #[serde(rename = "hub.mode")]
    pub mode: Mode,
    #[serde(rename = "hub.topic")]
    pub topic: String,
    #[serde(rename = "hub.verify")]
    pub verify: Verify,
    #[serde(rename = "hub.lease_seconds")]
    pub lease_seconds: u64,
}
