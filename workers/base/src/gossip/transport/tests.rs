use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};

use futures::StreamExt;
use once_cell::sync::Lazy;
use pki::KeyStore;
use tokio::{net::TcpListener, time::sleep};
use tokio_tungstenite::tungstenite::http::Uri;

use crate::gossip::{
    ident::ID,
    resolver::MockResolver,
    tests::{ca, certs, CA},
    transport::{
        websocket::{accept_ws, connect_ws, ws_transport},
        GossipSink,
    },
};

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
                    assert!(
                        accept_ws(socket, alice_acceptor)
                            .await
                            .unwrap_err()
                            .to_string()
                            .contains("UnknownIssuer")
                    );
                    rejected.fetch_add(1, Ordering::SeqCst);
                }
            }
        });
    }

    // Bob is malicious and sends a certificate signed by the fraud CA.
    let bob_certs = certs(&FRAUD_CA, "bob");
    let bob_connector = bob_certs.connector();
    // Must get rejected.
    assert!(
        connect_ws(
            &format!("wss://alice:{}", alice_port).parse().unwrap(),
            &"wss://bob".parse().unwrap(),
            bob_connector,
            MockResolver,
        )
        .await
        .unwrap_err()
        .to_string()
        .contains("HandshakeFailure")
    );

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
                    assert!(
                        dbg!(
                            accept_ws(socket, alice_acceptor)
                                .await
                                .unwrap_err()
                                .to_string()
                        )
                        .contains("BadCertificate")
                    );
                    rejected.fetch_add(1, Ordering::SeqCst);
                }
            }
        });
    }

    let bob_certs = certs(&CA, "bob");
    let bob_connector = bob_certs.connector();
    // Must reject connection.
    assert!(
        dbg!(
            connect_ws(
                &format!("wss://alice:{}", alice_port).parse().unwrap(),
                &"wss://bob".parse().unwrap(),
                bob_connector,
                MockResolver,
            )
            .await
            .unwrap_err()
            .to_string()
        )
        .contains("UnknownIssuer")
    );

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
                    assert!(
                        dbg!(
                            accept_ws(socket, alice_acceptor)
                                .await
                                .unwrap_err()
                                .to_string()
                        )
                        .contains("Bad Request")
                    );
                    rejected.fetch_add(1, Ordering::SeqCst);
                }
            }
        });
    }

    // Bob is malicious and claims to be Charlie.
    let bob_certs = certs(&CA, "bob");
    let bob_connector = bob_certs.connector();
    // Must reject connection.
    assert!(
        dbg!(
            connect_ws(
                &format!("wss://alice:{}", alice_port).parse().unwrap(),
                &"wss://charlie".parse().unwrap(), // Not bob!
                bob_connector,
                MockResolver,
            )
            .await
            .unwrap_err()
            .to_string()
        )
        .contains("Bad Request")
    );

    sleep(Duration::from_millis(100)).await;
    assert_eq!(rejected.load(Ordering::SeqCst), 1);
}
