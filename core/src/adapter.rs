//! Transport adapter.
use std::marker::PhantomData;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures_util::{ready, sink::Sink, SinkExt, Stream, StreamExt};
use serde::de::DeserializeOwned;
use serde::Serialize;
use tokio_tungstenite::tungstenite::{Error, Message};

use crate::error::TransportError;

/// A transport adapter that implements `Transport` for Websocket stream.
pub struct WsTransport<S, Item>(S, PhantomData<Item>);

impl<S, Item> WsTransport<S, Item> {
    /// Create a new `WsTransport`.
    pub const fn new(stream: S) -> Self {
        Self(stream, PhantomData)
    }
}

impl<S, Item> Stream for WsTransport<S, Item>
where
    S: Stream<Item = Result<Message, Error>> + Unpin,
    Item: DeserializeOwned,
    Self: Unpin,
{
    type Item = Result<Item, TransportError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Poll::Ready(match ready!(self.0.poll_next_unpin(cx)) {
            Some(Ok(e)) => {
                if let Message::Binary(data) = e {
                    Some(Ok(bson::from_slice(&data)?))
                } else {
                    return Poll::Pending;
                }
            }
            Some(Err(e)) => Some(Err(e.into())),
            None => None,
        })
    }
}

impl<S, Item, SinkItem> Sink<SinkItem> for WsTransport<S, Item>
where
    S: Sink<Message, Error = Error> + Unpin,
    SinkItem: Serialize,
    Self: Unpin,
{
    type Error = TransportError;

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.0.poll_ready_unpin(cx).map_err(Into::into)
    }

    fn start_send(mut self: Pin<&mut Self>, item: SinkItem) -> Result<(), Self::Error> {
        let item = bson::to_vec(&item)?;
        Ok(self.0.start_send_unpin(Message::Binary(item))?)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.0.poll_ready_unpin(cx).map_err(Into::into)
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.0.poll_ready_unpin(cx).map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    #![allow(dead_code)]

    use tarpc::{ClientMessage, Response, Transport};
    use tokio::net::TcpStream;
    use tokio_tungstenite::WebSocketStream;

    use crate::adapter::WsTransport;

    fn assert_transport<T>()
    where
        T: Transport<ClientMessage<()>, Response<()>>,
    {
    }

    fn must_adapter_transport() {
        assert_transport::<WsTransport<WebSocketStream<TcpStream>, _>>();
    }
}
