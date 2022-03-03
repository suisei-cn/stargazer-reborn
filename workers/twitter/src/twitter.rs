//! Twitter struct and stream.

use std::pin::Pin;
use std::task::{Context, Poll};

use egg_mode::entities::MediaType;
use egg_mode::error::Error;
use egg_mode::tweet::Tweet as RawTweet;
use egg_mode::tweet::{Timeline, TimelineFuture};
use egg_mode::Response;
use futures_util::{FutureExt, Stream};
use serde::{Deserialize, Serialize};

/// Represents a tweet.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Hash)]
pub struct Tweet {
    /// The tweet's unique identifier.
    pub id: u64,
    /// The tweet's text.
    pub text: String,
    /// URLs of media attached to the tweet.
    pub photos: Vec<String>,
    /// The url of the tweet.
    pub link: String,
    /// Whether the tweet is a retweet.
    pub is_rt: bool,
}

impl From<RawTweet> for Tweet {
    fn from(tweet: RawTweet) -> Self {
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

/// Twitter stream.
pub struct TimelineStream {
    timeline: Option<TimelineFuture>,
}

impl TimelineStream {
    /// Creates a new stream of tweets.
    ///
    /// # Errors
    /// Returns an error if the stream could not be created due to network issues.
    pub async fn new(timeline: Timeline) -> Result<Self, Error> {
        let (timeline, _) = timeline.start().await?;
        Ok(Self {
            timeline: Some(timeline.newer(None)),
        })
    }
}

impl Stream for TimelineStream {
    type Item = Result<Response<Vec<RawTweet>>, Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if let Some(mut timeline) = self.timeline.take() {
            match timeline.poll_unpin(cx) {
                Poll::Ready(Ok((timeline, resp))) => {
                    self.timeline = Some(timeline.newer(None));
                    Poll::Ready(Some(Ok(resp)))
                }
                Poll::Ready(Err(e)) => Poll::Ready(Some(Err(e))),
                Poll::Pending => {
                    self.timeline = Some(timeline);
                    Poll::Pending
                }
            }
        } else {
            Poll::Ready(None)
        }
    }
}
