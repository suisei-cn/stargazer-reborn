use std::pin::Pin;
use std::task::{Context, Poll};

use egg_mode::error::Error;
use egg_mode::tweet::{Timeline, TimelineFuture, Tweet};
use egg_mode::Response;
use futures_util::{FutureExt, Stream};

pub struct TimelineStream {
    timeline: Option<TimelineFuture>,
}

impl Stream for TimelineStream {
    type Item = Result<Response<Vec<Tweet>>, Error>;

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

impl TimelineStream {
    pub async fn new(timeline: Timeline) -> Result<Self, Error> {
        let (timeline, _) = timeline.start().await?;
        Ok(Self {
            timeline: Some(timeline.newer(None)),
        })
    }
}
