use egg_mode::entities::MediaType;
use eyre::{Report, Result};
use futures_util::future::join_all;
use once_cell::sync::Lazy;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tap::TapFallible;
use tracing::error;

static CLIENT: Lazy<Client> = Lazy::new(Client::new);

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Hash)]
pub struct Tweet {
    pub id: u64,
    pub text: String,
    pub photos: Vec<Vec<u8>>,
    pub link: String,
    pub is_rt: bool,
}

impl Tweet {
    pub async fn from_raw(tweet: egg_mode::tweet::Tweet) -> Self {
        let photos = join_all(
            tweet
                .entities
                .media
                .into_iter()
                .flatten()
                .filter(|medium| medium.media_type == MediaType::Photo)
                .map(|medium| medium.media_url_https)
                .map(|url| async move {
                    let resp = CLIENT.get(url).send().await?;
                    Result::<_, Report>::Ok(resp.bytes().await?.to_vec())
                }),
        )
        .await
        .into_iter()
        .filter_map(|img| {
            img.tap_err(|e| error!(error=?e, "Failed to download tweet image"))
                .ok()
        })
        .collect();

        Self {
            id: tweet.id,
            text: tweet.text,
            photos,
            link: format!(
                "https://twitter.com/{}/status/{}",
                tweet.user.expect("not a part of `TwitterUser`").screen_name,
                tweet.id
            ),
            is_rt: tweet.retweeted_status.is_some(),
        }
    }
}
