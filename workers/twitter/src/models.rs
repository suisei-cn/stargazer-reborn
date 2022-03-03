use egg_mode::entities::MediaType;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Hash)]
pub struct Tweet {
    pub id: u64,
    pub text: String,
    pub photos: Vec<String>,
    pub link: String,
    pub is_rt: bool,
}

impl From<egg_mode::tweet::Tweet> for Tweet {
    fn from(tweet: egg_mode::tweet::Tweet) -> Self {
        let photos = tweet
            .entities
            .media
            .into_iter()
            .flatten()
            .filter(|medium| medium.media_type == MediaType::Photo)
            .map(|medium| medium.media_url_https)
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
