//! Worker config.

use std::net::SocketAddr;

use serde::Deserialize;
use serde_with::formats::CommaSeparator;
use serde_with::{serde_as, DisplayFromStr, StringWithSeparator};
use tokio_tungstenite::tungstenite::http::Uri;

use crate::gossip::transport::certificate::deserialize as deserialize_certificates;
use crate::Certificates;

/// Configuration for worker nodes.
///
/// # Deserialize Implementation
/// Note that all array fields are deserialized as comma-separated strings.
#[serde_as]
#[derive(Debug, Clone, Deserialize)]
pub struct NodeConfig {
    /// A list of peer URL to announce to the rest of the cluster.
    /// If empty, the node will start idle.
    #[serde_as(as = "StringWithSeparator::<CommaSeparator, Uri>")]
    pub announce: Vec<Uri>,
    /// Socket address to bind to for gossip protocol.
    #[serde_as(as = "StringWithSeparator::<CommaSeparator, SocketAddr>")]
    pub bind: Vec<SocketAddr>,
    /// URI of this node to announce to the rest of the cluster.
    #[serde_as(as = "DisplayFromStr")]
    pub base_uri: Uri,
    /// Kind of this node.
    pub kind: String,
    /// TLS certificates to use for the gossip protocol.
    #[serde(flatten)]
    #[serde(deserialize_with = "deserialize_certificates")]
    pub certificates: Certificates,
    /// MongoDB configuration.
    #[serde(flatten)]
    pub db: DBConfig,
}

/// Database configuration.
#[derive(Debug, Clone, Eq, PartialEq, Deserialize)]
pub struct DBConfig {
    /// MongoDB connection URI.
    pub mongo_uri: String,
    /// Database name.
    pub mongo_db: String,
    /// Collection name.
    pub mongo_collection: String,
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;
    use std::str;
    use std::str::FromStr;

    use figment::providers::Env;
    use figment::{Figment, Jail};
    use tokio_tungstenite::tungstenite::http::Uri;

    use crate::gossip::tests::{ca, cert};
    use crate::{DBConfig, NodeConfig};

    #[test]
    fn must_from_env() {
        Jail::expect_with(|jail| {
            let ca = ca();
            let cert = cert(&ca, "charlie");
            let ca_pem = ca.to_pkcs8().unwrap();
            let cert_pem = cert.to_pkcs8().unwrap();

            let _ca_file = jail
                .create_file("ca.pem", str::from_utf8(&*ca_pem).unwrap())
                .unwrap();
            let _cert_file = jail
                .create_file("cert.pem", str::from_utf8(&*cert_pem).unwrap())
                .unwrap();

            jail.set_env("CONF_ANNOUNCE", "http://alice:8080,http://bob:8080");
            jail.set_env("CONF_BIND", "0.0.0.0:8080,[::]:8080");
            jail.set_env("CONF_BASE_URI", "http://charlie:8080");
            jail.set_env("CONF_KIND", "test");
            jail.set_env("CONF_CA_FILE", "ca.pem");
            jail.set_env("CONF_CERT_FILE", "cert.pem");
            jail.set_env("CONF_MONGO_URI", "mongodb://localhost:27017");
            jail.set_env("CONF_MONGO_DB", "stargazer-reborn");
            jail.set_env("CONF_MONGO_COLLECTION", "tasks");

            let config: NodeConfig = Figment::from(Env::prefixed("CONF_")).extract().unwrap();
            let NodeConfig {
                announce,
                bind,
                base_uri,
                kind,
                certificates,
                db,
            } = config;
            assert_eq!(
                announce,
                vec![
                    Uri::from_str("http://alice:8080").unwrap(),
                    Uri::from_str("http://bob:8080").unwrap(),
                ]
            );
            assert_eq!(
                bind,
                vec![
                    SocketAddr::from_str("0.0.0.0:8080").unwrap(),
                    SocketAddr::from_str("[::]:8080").unwrap(),
                ]
            );
            assert_eq!(base_uri, Uri::from_str("http://charlie:8080").unwrap());
            assert_eq!(kind, "test".to_string());
            assert_eq!(
                db,
                DBConfig {
                    mongo_uri: "mongodb://localhost:27017".to_string(),
                    mongo_db: "stargazer-reborn".to_string(),
                    mongo_collection: "tasks".to_string(),
                }
            );
            assert!(!certificates.root_certificates.is_empty());
            assert!(!certificates.public_cert_chain.is_empty());
            assert!(!certificates.private_key.0.is_empty());
            Ok(())
        });
    }
}
