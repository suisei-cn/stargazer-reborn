use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Debug, Deserialize)]
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Feed {
    pub entry: Option<Entry>,
    pub deleted_entry: Option<DeletedEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub video_id: String,
    pub link: Link,
    pub title: String,
    pub channel_id: String,
}

#[derive(Debug, Deserialize)]
pub struct Link {
    pub href: Url,
}

#[derive(Debug, Deserialize)]
pub struct DeletedEntry {
    #[serde(rename = "ref")]
    pub video_id: String,
}
