use std::{
    pin::Pin,
    str::FromStr,
    sync::Arc,
    task::{Context, Poll},
};

use async_trait::async_trait;
use eyre::{bail, eyre, ContextCompat, Result, WrapErr};
use futures::{
    sink::SinkExt,
    stream::{SplitStream, Stream, StreamExt},
};
use rustls::ServerName;
use sg_core::utils::ScopedJoinHandle;
use tap::TapFallible;
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{
        mpsc::{channel, Receiver, Sender},
        oneshot,
        Mutex,
    },
};
use tokio_rustls::{TlsAcceptor, TlsConnector, TlsStream};
use tokio_tungstenite::{
    accept_hdr_async,
    client_async,
    tungstenite::{
        client::IntoClientRequest,
        handshake::server::{Request, Response},
        http::{HeaderValue, StatusCode, Uri},
        Message,
    },
};
use tracing::{error, field, info, warn, Span};
use webpki::{DnsNameRef, EndEntityCert};

use super::certificate::Certificates;
use crate::gossip::{
    ident::ID,
    resolver::DNSResolver,
    transport::{ConnPool, GossipSink, GossipStream, Ws},
};

const MISSING_HEADER: &str = "Missing `X-Sender-Host` header.";
const INVALID_HEADER: &str = "Invalid `X-Sender-Host` header.";

/// Websocket stream of gossip messages.
pub struct WsGossipStream {
    /// Receiver of websocket messages. Real receiving logic is in the receiving
    /// task.
    rx: Receiver<Vec<u8>>,
    /// RAII handle of receiving task.
    _handle: ScopedJoinHandle<()>,
}

impl Stream for WsGossipStream {
    type Item = Vec<u8>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.rx.poll_recv(cx)
    }
}

impl GossipStream for WsGossipStream {}

/// Websocket sink of gossip messages.
#[derive(Clone)]
pub struct WsGossipSink<R: DNSResolver> {
    /// Connection pool.
    pool: Arc<ConnPool>,
    /// Base URL of this node.
    base_uri: Uri,
    /// Sender of websocket messages.
    /// Websocket is duplex so incoming messages should be relayed to receiving
    /// end.
    tx_recv: Sender<Vec<u8>>,
    /// Configured TLS connector for outgoing connections.
    tls_connector: TlsConnector,
    /// DNS resolver for outgoing connections.
    resolver: R,
}

#[async_trait]
impl<R> GossipSink<ID> for WsGossipSink<R>
where
    R: DNSResolver,
{
    async fn send(&self, target: ID, payload: Vec<u8>) -> Result<()> {
        let target = target.addr();
        let payload = Message::binary(payload);

        // Lock the pool, find the cell of the target node, and create one if it doesn't
        // exist. The lock of the pool is released immediately.
        let locked_cell = self
            .pool
            .lock()
            .unwrap()
            .entry(target.clone())
            .or_default()
            .clone();
        // Lock the cell to make sure no two connections are created to the same node.
        let mut cell = locked_cell.lock().await;

        // Acquire the connection to target node.
        let sink = {
            match &mut *cell {
                // We've connected to the node before.
                Some(ws) => ws,
                None => {
                    // This is a new target node. We need to connect to it.
                    let ws = connect_ws(
                        target,
                        &self.base_uri,
                        self.tls_connector.clone(),
                        self.resolver.clone(),
                    )
                    .await?;
                    let (sink, stream) = ws.split();

                    // Websocket is a duplex protocol,
                    // so we need to start a receiving task.
                    tokio::spawn({
                        let tx_recv = self.tx_recv.clone();
                        recv_loop(stream, tx_recv)
                    });

                    // Save the sending end to cell so we may use it later.
                    *cell = Some(sink);
                    cell.as_mut().unwrap()
                }
            }
        };

        // Send the message to the target node.
        sink.send(payload).await.tap_err(|e| {
            // An error has occur. Remove the connection from pool.
            warn!("Failed to send message to {}: {}", target, e);
            self.pool.lock().unwrap().remove(target);
        })?;
        Ok(())
    }
}

/// Receiving loop of incoming stream.
async fn recv_loop(mut stream: SplitStream<Ws>, tx_recv: Sender<Vec<u8>>) {
    while let Some(Ok(msg)) = stream.next().await {
        if let Message::Binary(data) = msg {
            if tx_recv.send(data).await.is_err() {
                // Foca has stopped.
                break;
            }
        }
    }
}

/// Validate that the `X-Sender-Host` header is valid for given certificate
/// chain.
fn validate_x_sender_host(cert: &EndEntityCert, value: &HeaderValue) -> Result<Uri> {
    let s = value.to_str().wrap_err(INVALID_HEADER)?;
    let uri = Uri::from_str(s).wrap_err(INVALID_HEADER)?;
    let host = uri.host().wrap_err(INVALID_HEADER)?;
    let dns_name = DnsNameRef::try_from_ascii_str(host).wrap_err(INVALID_HEADER)?;
    cert.verify_is_valid_for_dns_name(dns_name)
        .wrap_err("Client's certificate is invalid for its claimed hostname.")?;
    Ok(uri)
}

pub async fn connect_ws(
    host: &Uri,
    base_uri: &Uri,
    connector: TlsConnector,
    resolver: impl DNSResolver,
) -> Result<Ws> {
    // Advertise the uri of this node by sending `X-Sender-Host` header.
    let mut request = host.into_client_request()?;
    request.headers_mut().insert(
        "X-Sender-Host",
        HeaderValue::from_str(&base_uri.to_string())
            .expect("Fatal: local url address is not encodable."),
    );

    // The following line shouldn't panic because `request` is converted from `host`
    // above.
    let domain = request.uri().host().expect("INV: missing host").to_string();
    if request.uri().scheme_str() != Some("wss") {
        bail!("Invalid protocol: Only secured websocket is supported.");
    }
    let port = request.uri().port_u16().unwrap_or(443);

    // Resolve remote domain name to IP address.
    let addr = {
        let domain = domain.clone();
        tokio::task::spawn_blocking(move || resolver.resolve(&domain, port))
            .await
            .expect("INV: DNS resolver panicked")?
    };

    // Connect to the remote node.
    let stream = TcpStream::connect(&*addr).await?;
    let stream: TlsStream<_> = connector
        .connect(ServerName::try_from(&*domain)?, stream)
        .await?
        .into();

    // Create a websocket stream from the TLS stream.
    let (stream, _) = client_async(request, stream).await?;
    Ok(stream)
}

/// Accept a remote node's connection.
#[tracing::instrument(skip(acceptor), fields(x_sender_host = field::Empty))]
pub async fn accept_ws(stream: TcpStream, acceptor: TlsAcceptor) -> Result<(Ws, Uri)> {
    // Accept the connection.
    let stream: TlsStream<_> = acceptor.accept(stream).await?.into();

    // Extract tls certificate.
    let raw_cert = stream
        .get_ref()
        .1
        .peer_certificates()
        .expect("TODO: error")
        .first()
        .expect("TODO: error")
        .clone();
    // No need to verify trust chain: root certificate is verified during handshake.
    // Extract end entity cert only.
    let cert = EndEntityCert::try_from(raw_cert.as_ref())?;

    // Due to API design of tungstenite, we need a callback to extract headers when
    // accepting a websocket connection.
    //
    // `X-Sender-Host`: remote node's host address.
    let (tx, rx) = oneshot::channel();
    let callback = move |req: &Request, resp| {
        let sender_host = req
            .headers()
            .get("X-Sender-Host")
            .ok_or_else(|| eyre!(MISSING_HEADER))
            .tap_ok(|v| {
                // Log header value.
                let span = Span::current();
                span.record("x_sender_host", &field::debug(v));
            })
            .and_then(|v| {
                // Ensure that the `X-Sender-Host` header is valid for given certificate chain
                validate_x_sender_host(&cert, v)
            });

        let resp = if let Err(e) = &sender_host {
            Err(Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Some(e.to_string()))
                .unwrap())
        } else {
            Ok(resp)
        };

        tx.send(sender_host).expect("INV: accept_hdr tx closed");
        resp
    };

    // Accept the websocket connection.
    let stream = accept_hdr_async(stream, callback).await?;
    // Retrieve remote address from the header.
    let sender_host = rx.await.expect("INV: accept_hdr rx closed")?;

    Ok((stream, sender_host))
}

/// Entry point for WebSocket-based Foca transport.
#[allow(clippy::missing_panics_doc)]
pub async fn ws_transport<R: DNSResolver>(
    listener: TcpListener,
    certificates: Certificates,
    base_uri: Uri,
    resolver: R,
) -> (WsGossipStream, WsGossipSink<R>) {
    let (tx_recv, rx_recv) = channel(1024);
    let conn_pool = Arc::new(ConnPool::default());

    let acceptor = certificates.clone().acceptor();
    let connector = certificates.connector();

    // Spawn acceptor task.
    let handle = {
        let conn_pool = conn_pool.clone();
        let tx_recv = tx_recv.clone();

        ScopedJoinHandle(tokio::spawn(async move {
            loop {
                // Try accept a new connection.
                if let Ok((socket, addr)) = listener.accept().await {
                    // If accept succeeds, spawn a task to handle the connection.
                    info!(addr = %addr, "Accepting connection.");
                    let tx_recv = tx_recv.clone();
                    let conn_pool = conn_pool.clone();
                    let acceptor = acceptor.clone();

                    tokio::spawn(async move {
                        // Try to handshake.
                        match accept_ws(socket, acceptor).await {
                            Ok((stream, sender_host)) => {
                                let (sink, stream) = stream.split();
                                // Websocket is duplex. Insert sender end to connection pool.
                                conn_pool
                                    .lock()
                                    .unwrap()
                                    .insert(sender_host, Arc::new(Mutex::new(Some(sink))));
                                // Start receiving loop.
                                recv_loop(stream, tx_recv).await;
                            }
                            Err(e) => {
                                error!("Failed to accept connection: {}", e);
                            }
                        }
                    });
                }
            }
        }))
    };

    let stream = WsGossipStream {
        rx: rx_recv,
        _handle: handle, // life of receiving task is bound to the stream object
    };
    let sink = WsGossipSink {
        pool: conn_pool,
        base_uri,
        tx_recv,
        tls_connector: connector,
        resolver,
    };
    (stream, sink)
}
