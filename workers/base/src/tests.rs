use std::collections::{HashMap, HashSet};
use std::iter;
use std::net::{IpAddr, SocketAddr};
use std::num::{NonZeroU8, NonZeroUsize};
use std::ops::Add;
use std::time::{Duration, SystemTime};

use foca::Config;
use once_cell::sync::Lazy;
use pki::{CertName, CertUsage, CertificateBuilder, KeyStore, PrivateKey};
use tokio::net::TcpListener;
use tokio::sync::{mpsc, oneshot};
use tokio::time::sleep;
use tokio_tungstenite::tungstenite::http::Uri;

use sg_core::utils::ScopedJoinHandle;

use crate::ident::ID;
use crate::resolver::MockResolver;
use crate::runtime::start_foca;
use crate::transport::{ws_transport, Certificates};

pub static CA: Lazy<KeyStore> = Lazy::new(ca);

pub fn ca() -> KeyStore {
    CertificateBuilder::new()
        .subject(CertName::new([("CN", "Root CA")]).unwrap())
        .usage(CertUsage::CA)
        .not_after(SystemTime::now().add(Duration::from_secs(365 * 10 * 24 * 60 * 60)))
        .private_key(PrivateKey::new_rsa(2048).unwrap())
        .build()
        .unwrap()
}

pub fn cert(ca: &KeyStore, hostname: &str) -> KeyStore {
    CertificateBuilder::new()
        .subject(CertName::new([("CN", hostname)]).unwrap())
        .signer(ca)
        .usage(CertUsage::TlsServerAndClient)
        .alt_names([hostname])
        .private_key(PrivateKey::new_rsa(2048).unwrap())
        .build()
        .unwrap()
}

pub fn certs(ca: &KeyStore, hostname: &str) -> Certificates {
    Certificates::from_pem(
        &CA.to_pkcs8().unwrap(),
        &cert(ca, hostname).to_pkcs8().unwrap(),
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test() {
    let mut hosts = HashMap::new();

    eprintln!("Starting root");
    let root = start_rt("root", None).await;
    let root_uri = root.1.clone();
    hosts.insert(String::from("root"), root);
    assert_cluster(&hosts).await;

    for client in ["alice", "bob", "charlie"] {
        eprintln!("Starting {}", client);
        hosts.insert(
            client.to_string(),
            start_rt(client, Some(root_uri.clone())).await,
        );
    }
    sleep(Duration::from_secs(2)).await;
    assert_cluster(&hosts).await;

    for client in ["david", "edward"] {
        eprintln!("Starting {}", client);
        hosts.insert(
            client.to_string(),
            start_rt(client, Some(root_uri.clone())).await,
        );
    }
    sleep(Duration::from_secs(2)).await;
    assert_cluster(&hosts).await;

    for client in ["bob", "david"] {
        eprintln!("Stopping {}", client);
        hosts.remove(client);
    }
    sleep(Duration::from_secs(4)).await;
    assert_cluster(&hosts).await;

    for client in ["matchy", "commelina"] {
        eprintln!("Starting {}", client);
        hosts.insert(
            client.to_string(),
            start_rt(client, Some(root_uri.clone())).await,
        );
    }
    sleep(Duration::from_secs(2)).await;
    assert_cluster(&hosts).await;

    for client in ["root", "edward"] {
        eprintln!("Stopping {}", client);
        hosts.remove(client);
    }
    sleep(Duration::from_secs(4)).await;
    assert_cluster(&hosts).await;
}

async fn assert_cluster(hosts: &HashMap<String, (ScopedJoinHandle<()>, Uri, mpsc::Sender<Cmd>)>) {
    let expected_members: HashSet<_> = hosts.keys().cloned().collect();
    for (_, _, sender) in hosts.values() {
        let (tx, rx) = oneshot::channel();
        sender.send(Cmd::Members(tx)).await.unwrap();
        let members = rx.await.unwrap();
        assert_eq!(members, expected_members);
    }
}

#[derive(Debug)]
enum Cmd {
    Members(oneshot::Sender<HashSet<String>>),
}

async fn start_rt(
    hostname: &str,
    announce: Option<Uri>,
) -> (ScopedJoinHandle<()>, Uri, mpsc::Sender<Cmd>) {
    let socket = TcpListener::bind(SocketAddr::new(IpAddr::from([127, 0, 0, 1]), 0))
        .await
        .unwrap();
    let port = socket.local_addr().unwrap().port();
    let base_uri: Uri = format!("wss://{}:{}", hostname, port).parse().unwrap();

    let certificates = certs(&*CA, hostname);

    let (stream, sink) = ws_transport(socket, certificates, base_uri.clone(), MockResolver).await;

    let ident = ID::new(base_uri.clone(), String::from("test"));

    let test_config = Config {
        probe_period: Duration::from_millis(300),
        probe_rtt: Duration::from_millis(100),
        num_indirect_probes: NonZeroUsize::new(3).unwrap(),
        max_transmissions: NonZeroU8::new(10).unwrap(),
        suspect_to_down_after: Duration::from_millis(600),
        remove_down_after: Duration::from_secs(15),
        max_packet_size: NonZeroUsize::new(1400).unwrap(),
    };
    let foca = start_foca(ident, stream, sink, test_config);
    if let Some(announce) = announce {
        let id = ID::new(announce, String::from("test"));
        foca.announce(id);
    }

    let (tx, mut rx) = mpsc::channel(10);

    (
        ScopedJoinHandle(tokio::spawn(async move {
            while let Some(cmd) = rx.recv().await {
                match cmd {
                    Cmd::Members(tx) => {
                        let members = *foca
                            .with(|foca| {
                                foca.iter_members()
                                    .chain(iter::once(foca.identity()))
                                    .map(|id| id.addr().host().unwrap().to_string())
                                    .collect()
                            })
                            .await;
                        tx.send(members).unwrap();
                    }
                }
            }
        })),
        base_uri,
        tx,
    )
}
