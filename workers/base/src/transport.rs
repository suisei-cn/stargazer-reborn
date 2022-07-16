//! Transport implementations for Foca runtime.
use std::collections::HashMap;
use std::iter;
use std::pin::Pin;
use std::str::FromStr;
use std::sync::{Arc, Mutex as StdMutex};
use std::task::{Context, Poll};

use async_trait::async_trait;
use eyre::{bail, eyre, ContextCompat, Result, WrapErr};
use futures::stream::{SplitSink, SplitStream};
use futures::{SinkExt, Stream, StreamExt};
use rustls::server::AllowAnyAuthenticatedClient;
use rustls::{Certificate, ClientConfig, PrivateKey, RootCertStore, ServerConfig, ServerName};
use rustls_pemfile::Item;
use tap::TapFallible;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::sync::{oneshot, Mutex};
use tokio_rustls::{TlsAcceptor, TlsConnector, TlsStream};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::handshake::server::Request;
use tokio_tungstenite::tungstenite::handshake::server::Response;
use tokio_tungstenite::tungstenite::http::{HeaderValue, StatusCode, Uri};
use tokio_tungstenite::tungstenite::Message as WsMessage;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{accept_hdr_async, client_async, WebSocketStream};
use tracing::field;
use tracing::{debug, error, info, warn, Span};
use webpki::{DnsNameRef, EndEntityCert};

use sg_core::utils::ScopedJoinHandle;

use crate::ident::ID;
use crate::resolver::DNSResolver;

type Ws = WebSocketStream<TlsStream<TcpStream>>;
type ConnPool = StdMutex<HashMap<Uri, Arc<Mutex<Option<SplitSink<Ws, WsMessage>>>>>>;

/// Stream of gossip messages from other nodes.
pub trait GossipStream: Send + Stream<Item = Vec<u8>> + Unpin + 'static {}

/// Sink of gossip messages to other nodes.
#[async_trait]
pub trait GossipSink<Ident>: Send + Sync + Clone + 'static {
    /// Send a message to a node.
    async fn send(&self, target: Ident, payload: Vec<u8>) -> Result<()>;
}

/// Websocket stream of gossip messages.
pub struct WsGossipStream {
    /// Receiver of websocket messages. Real receiving logic is in the receiving task.
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
    /// Websocket is duplex so incoming messages should be relayed to receiving end.
    tx_recv: Sender<Vec<u8>>,
    /// Configured TLS connector for outgoing connections.
    tls_connector: TlsConnector,
    /// DNS resolver for outgoing connections.
    resolver: R,
}

/// Connect to a remote node.
#[tracing::instrument(skip(base_uri, connector, resolver))]
async fn connect_ws(
    host: &Uri,
    base_uri: &Uri,
    connector: TlsConnector,
    resolver: impl DNSResolver,
) -> Result<Ws> {
    // Append local host address to connection request.
    let mut request = host.into_client_request()?;
    request.headers_mut().insert(
        "X-Sender-Host",
        HeaderValue::from_str(&base_uri.to_string())
            .expect("Fatal: local url address is not encodable."),
    );

    // The following line shouldn't panic because `request` is converted from `host` above.
    let domain = request.uri().host().expect("INV: missing host").to_string();
    if request.uri().scheme_str() != Some("wss") {
        bail!("Invalid protocol: Only secured websocket is supported.");
    }
    let port = request.uri().port_u16().unwrap_or(443);

    let addr = {
        let domain = domain.clone();
        tokio::task::spawn_blocking(move || resolver.resolve(&domain, port))
            .await
            .expect("INV: DNS resolver panicked")?
    };

    let stream = TcpStream::connect(&*addr).await?;
    let stream: TlsStream<_> = connector
        .connect(ServerName::try_from(&*domain)?, stream)
        .await?
        .into();

    let (stream, _) = client_async(request, stream).await?;
    Ok(stream)
}

/// Receiving loop of incoming stream.
async fn recv_loop(mut stream: SplitStream<Ws>, tx_recv: Sender<Vec<u8>>) {
    while let Some(Ok(msg)) = stream.next().await {
        if let WsMessage::Binary(data) = msg {
            if tx_recv.send(data).await.is_err() {
                // Foca has stopped.
                break;
            }
        }
    }
}

#[async_trait]
impl<R> GossipSink<ID> for WsGossipSink<R>
where
    R: DNSResolver,
{
    async fn send(&self, target: ID, payload: Vec<u8>) -> Result<()> {
        let target = target.addr();
        let payload = Message::binary(payload);

        // Lock the pool, find the cell of the target node, and create one if it doesn't exist.
        // The lock of the pool is released immediately.
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

const MISSING_HEADER: &str = "Missing `X-Sender-Host` header.";
const INVALID_HEADER: &str = "Invalid `X-Sender-Host` header.";

fn validate_x_sender_host(cert: &EndEntityCert, value: &HeaderValue) -> Result<Uri> {
    let s = value.to_str().wrap_err(INVALID_HEADER)?;
    let uri = Uri::from_str(s).wrap_err(INVALID_HEADER)?;
    let host = uri.host().wrap_err(INVALID_HEADER)?;
    let dns_name = DnsNameRef::try_from_ascii_str(host).wrap_err(INVALID_HEADER)?;
    cert.verify_is_valid_for_dns_name(dns_name)
        .wrap_err("Client's certificate is invalid for its claimed hostname.")?;
    Ok(uri)
}

/// Accept a remote node's connection.
///
/// Note that the connection is not closed even if the handshake fails.
#[tracing::instrument(skip(acceptor), fields(x_sender_host = field::Empty))]
async fn accept_ws(stream: TcpStream, acceptor: TlsAcceptor) -> Result<(Ws, Uri)> {
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

    // Due to API design of tungstenite, we need a callback to extract headers when accepting
    // a websocket connection.
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
            .and_then(|v| validate_x_sender_host(&cert, v));

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

    // Accept connection.
    let stream = accept_hdr_async(stream, callback).await?;
    // Retrieve remote address from the header.
    let sender_host = rx.await.expect("INV: accept_hdr rx closed")?;

    Ok((stream, sender_host))
}

/// Certificates used by a client or a server.
#[derive(Clone)]
pub struct Certificates {
    /// Trusted root certificates.
    root_certificates: RootCertStore,
    /// Public certificate chain.
    public_cert_chain: Vec<Certificate>,
    /// Private key.
    private_key: PrivateKey,
}

impl Certificates {
    /// Create a new `Certificates` instance with given pem files.
    ///
    /// `root`: CA certificate in PEM format.
    /// `cert`: public certificate and private key in PEM format.
    #[must_use]
    #[allow(clippy::missing_panics_doc, clippy::cognitive_complexity)]
    pub fn from_pem(mut root: &[u8], mut cert: &[u8]) -> Self {
        let mut root_certs = vec![];
        for section in
            iter::from_fn(|| rustls_pemfile::read_one(&mut root).expect("CFG: Corrupt PEM file"))
        {
            if let Item::X509Certificate(cert) = section {
                root_certs.push(cert);
            } else {
                warn!("Section not handled in given pem file.");
            }
        }

        let mut public_cert_chain = vec![];
        let mut private_key = None;
        for section in
            iter::from_fn(|| rustls_pemfile::read_one(&mut cert).expect("CFG: Corrupt PEM file"))
        {
            match section {
                Item::X509Certificate(cert) => public_cert_chain.push(Certificate(cert)),
                Item::PKCS8Key(key) => private_key = Some(PrivateKey(key)),
                _ => warn!("Section not handled in given pem file."),
            }
        }

        let mut root_certificates = RootCertStore::empty();
        let (succ, _) = root_certificates.add_parsable_certificates(&root_certs);
        debug!("{} root certificates added", succ);

        Self {
            root_certificates,
            public_cert_chain,
            private_key: private_key.expect("CFG: missing private key"),
        }
    }
    /// Return a TLS acceptor configured with the given certificates.
    ///
    /// # Panics
    /// Panics if the private key is invalid.
    fn acceptor(self) -> TlsAcceptor {
        let server_config = ServerConfig::builder()
            .with_safe_defaults()
            .with_client_cert_verifier(AllowAnyAuthenticatedClient::new(self.root_certificates))
            .with_single_cert(self.public_cert_chain, self.private_key)
            .expect("CFG: invalid server certificate");
        TlsAcceptor::from(Arc::new(server_config))
    }
    /// Return a TLS connector configured with the given certificates.
    ///
    /// # Panics
    /// Panics if the private key is invalid.
    fn connector(self) -> TlsConnector {
        let client_config = ClientConfig::builder()
            .with_safe_defaults()
            .with_root_certificates(self.root_certificates)
            .with_single_cert(self.public_cert_chain, self.private_key)
            .expect("CFG: invalid client certificate");
        TlsConnector::from(Arc::new(client_config))
    }
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

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    use futures::StreamExt;
    use once_cell::sync::Lazy;
    use pki::KeyStore;
    use tokio::net::TcpListener;
    use tokio::time::sleep;
    use tokio_tungstenite::tungstenite::http::Uri;

    use crate::ident::ID;
    use crate::resolver::MockResolver;
    use crate::tests::{ca, certs, CA};
    use crate::transport::{accept_ws, connect_ws, ws_transport, GossipSink};

    static FRAUD_CA: Lazy<KeyStore> = Lazy::new(ca);

    #[tokio::test]
    async fn must_stream_sink() {
        let alice_certs = certs(&CA, "alice");
        let socket = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let alice_port = socket.local_addr().unwrap().port();
        let alice_uri: Uri = format!("wss://alice:{}", alice_port).parse().unwrap();
        let (mut alice_stream, alice_sink) =
            ws_transport(socket, alice_certs, alice_uri.clone(), MockResolver).await;

        let bob_certs = certs(&CA, "bob");
        let socket = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let bob_port = socket.local_addr().unwrap().port();
        let bob_uri: Uri = format!("wss://bob:{}", bob_port).parse().unwrap();
        let (mut bob_stream, bob_sink) =
            ws_transport(socket, bob_certs, bob_uri.clone(), MockResolver).await;

        alice_sink
            .send(
                ID::new(bob_uri, String::from("test")),
                b"Hello Bob!".to_vec(),
            )
            .await
            .unwrap();
        bob_sink
            .send(
                ID::new(alice_uri, String::from("test")),
                b"Hello Alice!".to_vec(),
            )
            .await
            .unwrap();
        assert_eq!(alice_stream.next().await.unwrap(), b"Hello Alice!".to_vec());
        assert_eq!(bob_stream.next().await.unwrap(), b"Hello Bob!".to_vec());
    }

    #[tokio::test]
    async fn must_reject_fraud_ca_client() {
        let rejected = Arc::new(AtomicUsize::new(0));

        let alice_certs = certs(&CA, "alice");
        let socket = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let alice_port = socket.local_addr().unwrap().port();
        let alice_acceptor = alice_certs.acceptor();
        {
            let rejected = rejected.clone();
            tokio::spawn(async move {
                loop {
                    if let Ok((socket, _)) = socket.accept().await {
                        let alice_acceptor = alice_acceptor.clone();
                        // Must reject connection.
                        assert!(accept_ws(socket, alice_acceptor)
                            .await
                            .unwrap_err()
                            .to_string()
                            .contains("UnknownIssuer"));
                        rejected.fetch_add(1, Ordering::SeqCst);
                    }
                }
            });
        }

        // Bob is malicious and sends a certificate signed by the fraud CA.
        let bob_certs = certs(&FRAUD_CA, "bob");
        let bob_connector = bob_certs.connector();
        // Must get rejected.
        assert!(connect_ws(
            &format!("wss://alice:{}", alice_port).parse().unwrap(),
            &"wss://bob".parse().unwrap(),
            bob_connector,
            MockResolver,
        )
        .await
        .unwrap_err()
        .to_string()
        .contains("HandshakeFailure"));

        assert_eq!(rejected.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn must_reject_fraud_ca_server() {
        let rejected = Arc::new(AtomicUsize::new(0));

        // Alice is malicious and sends a certificate signed by the fraud CA.
        let alice_certs = certs(&FRAUD_CA, "alice");
        let socket = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let alice_port = socket.local_addr().unwrap().port();
        let alice_acceptor = alice_certs.acceptor();
        {
            let rejected = rejected.clone();
            tokio::spawn(async move {
                loop {
                    if let Ok((socket, _)) = socket.accept().await {
                        eprintln!("Accepting connection.");
                        let alice_acceptor = alice_acceptor.clone();
                        // Must get rejected.
                        assert!(dbg!(accept_ws(socket, alice_acceptor)
                            .await
                            .unwrap_err()
                            .to_string())
                        .contains("BadCertificate"));
                        rejected.fetch_add(1, Ordering::SeqCst);
                    }
                }
            });
        }

        let bob_certs = certs(&CA, "bob");
        let bob_connector = bob_certs.connector();
        // Must reject connection.
        assert!(dbg!(connect_ws(
            &format!("wss://alice:{}", alice_port).parse().unwrap(),
            &"wss://bob".parse().unwrap(),
            bob_connector,
            MockResolver,
        )
        .await
        .unwrap_err()
        .to_string())
        .contains("UnknownIssuer"));

        sleep(Duration::from_millis(100)).await;
        assert_eq!(rejected.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn must_reject_host_cert_mismatch() {
        let rejected = Arc::new(AtomicUsize::new(0));

        let alice_certs = certs(&CA, "alice");
        let socket = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let alice_port = socket.local_addr().unwrap().port();
        let alice_acceptor = alice_certs.acceptor();
        {
            let rejected = rejected.clone();
            tokio::spawn(async move {
                loop {
                    if let Ok((socket, _)) = socket.accept().await {
                        eprintln!("Accepting connection.");
                        let alice_acceptor = alice_acceptor.clone();
                        // Must get rejected.
                        assert!(dbg!(accept_ws(socket, alice_acceptor)
                            .await
                            .unwrap_err()
                            .to_string())
                        .contains("Bad Request"));
                        rejected.fetch_add(1, Ordering::SeqCst);
                    }
                }
            });
        }

        // Bob is malicious and claims to be Charlie.
        let bob_certs = certs(&CA, "bob");
        let bob_connector = bob_certs.connector();
        // Must reject connection.
        assert!(dbg!(connect_ws(
            &format!("wss://alice:{}", alice_port).parse().unwrap(),
            &"wss://charlie".parse().unwrap(), // Not bob!
            bob_connector,
            MockResolver,
        )
        .await
        .unwrap_err()
        .to_string())
        .contains("Bad Request"));

        sleep(Duration::from_millis(100)).await;
        assert_eq!(rejected.load(Ordering::SeqCst), 1);
    }
}
