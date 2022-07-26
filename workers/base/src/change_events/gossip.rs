//! Gossip provider.
use eyre::Result;
use foca::Notification;
use futures::{stream, Stream, StreamExt, TryStreamExt};
use tokio_stream::wrappers::BroadcastStream;

use crate::{common::Event, gossip::runtime::TokioFocaCtl};

/// Change stream from gossip protocol.
///
/// Provides cluster member changes.
pub async fn foca_events(foca: &TokioFocaCtl) -> impl Stream<Item = Result<Event>> {
    let rx_foca = foca.recv().await;
    let nodes: Vec<_> = *foca
        .with(|foca| {
            foca.iter_members()
                .map(|member| member.addr().clone())
                .collect()
        })
        .await;
    stream::iter(nodes.into_iter().map(|node| Ok(Event::NodeUp(node)))).chain(
        BroadcastStream::new(rx_foca)
            .try_filter_map(|notification| async move {
                Ok(match notification {
                    Notification::MemberUp(id) => Some(Event::NodeUp(id.addr().clone())),
                    Notification::MemberDown(id) => Some(Event::NodeDown(id.addr().clone())),
                    _ => None,
                })
            })
            .map_err(Into::into),
    )
}
