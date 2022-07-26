//! Worker config.

use std::net::SocketAddr;

use serde::Deserialize;
use serde_with::{formats::CommaSeparator, serde_as, DisplayFromStr, StringWithSeparator};
use sg_core::utils::Config;
use tokio_tungstenite::tungstenite::http::Uri;

use crate::{
    gossip::transport::certificate::deserialize as deserialize_certificates,
    Certificates,
};

/// Configuration for worker nodes.
///
/// # Deserialize Implementation
/// Note that all array fields are deserialized as comma-separated strings.
#[serde_as]
#[derive(Debug, Clone, Deserialize, Config)]
pub struct NodeConfig {
    /// A list of peer URL to announce to the rest of the cluster.
    /// If empty, the node will start idle.
    #[serde_as(as = "StringWithSeparator::<CommaSeparator, Uri>")]
    pub announce: Vec<Uri>,
    /// Socket address to bind to for gossip protocol.
    #[serde_as(as = "StringWithSeparator::<CommaSeparator, SocketAddr>")]
    #[config(default_str = "127.0.0.1:8001,[::1]:8001")]
    pub bind: Vec<SocketAddr>,
    /// URI of this node to announce to the rest of the cluster.
    #[serde_as(as = "DisplayFromStr")]
    pub base_uri: Uri,
    /// Kind of this node.
    pub kind: String,
    /// TLS certificates to use for the gossip protocol.
    #[serde(deserialize_with = "deserialize_certificates")]
    #[config(default = r#"{"ca": "ca.pem", "cert": "cert.pem"}"#)]
    pub cert: Certificates,
    /// MongoDB configuration.
    #[config(inherit)]
    pub mongo: DBConfig,
}

/// Database configuration.
#[derive(Debug, Clone, Eq, PartialEq, Deserialize, Config)]
pub struct DBConfig {
    /// MongoDB connection URI.
    #[config(default_str = "mongodb://localhost:27017")]
    pub uri: String,
    /// Database name.
    #[config(default_str = "stargazer-reborn")]
    pub db: String,
    /// Collection name.
    #[config(default_str = "tasks")]
    pub collection: String,
}

#[cfg(test)]
mod tests {
    use std::{net::SocketAddr, str, str::FromStr};

    use figment::Jail;
    use sg_core::utils::FigmentExt;
    use tokio_tungstenite::tungstenite::http::Uri;

    use crate::{
        gossip::tests::{ca, cert},
        DBConfig,
        NodeConfig,
    };

    #[test]
    fn must_default() {
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
            jail.set_env("CONF_BASE_URI", "http://charlie:8080");
            jail.set_env("CONF_KIND", "test");

            let config = NodeConfig::from_env("CONF_").unwrap();
            let NodeConfig {
                announce,
                bind,
                base_uri,
                kind,
                cert,
                mongo,
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
                    SocketAddr::from_str("127.0.0.1:8001").unwrap(),
                    SocketAddr::from_str("[::1]:8001").unwrap(),
                ]
            );
            assert_eq!(base_uri, Uri::from_str("http://charlie:8080").unwrap());
            assert_eq!(kind, "test".to_string());
            assert_eq!(
                mongo,
                DBConfig {
                    uri: "mongodb://localhost:27017".to_string(),
                    db: "stargazer-reborn".to_string(),
                    collection: "tasks".to_string(),
                }
            );
            assert!(!cert.root_certificates.is_empty());
            assert!(!cert.public_cert_chain.is_empty());
            assert!(!cert.private_key.0.is_empty());
            Ok(())
        });
    }

    #[test]
    fn must_from_env() {
        Jail::expect_with(|jail| {
            let ca = ca();
            let cert = cert(&ca, "charlie");
            let ca_pem = ca.to_pkcs8().unwrap();
            let cert_pem = cert.to_pkcs8().unwrap();

            let _ca_file = jail
                .create_file("ca.crt", str::from_utf8(&*ca_pem).unwrap())
                .unwrap();
            let _cert_file = jail
                .create_file("cert.crt", str::from_utf8(&*cert_pem).unwrap())
                .unwrap();

            jail.set_env("CONF_ANNOUNCE", "http://alice:8080,http://bob:8080");
            jail.set_env("CONF_BIND", "0.0.0.0:8080,[::]:8080");
            jail.set_env("CONF_BASE_URI", "http://charlie:8080");
            jail.set_env("CONF_KIND", "test");
            jail.set_env("CONF_CERT__CA", "ca.crt");
            jail.set_env("CONF_CERT__CERT", "cert.crt");
            jail.set_env("CONF_MONGO__URI", "mongodb://localhost:27017");
            jail.set_env("CONF_MONGO__DB", "stargazer-reborn");
            jail.set_env("CONF_MONGO__COLLECTION", "tasks");

            let config = NodeConfig::from_env("CONF_").unwrap();
            let NodeConfig {
                announce,
                bind,
                base_uri,
                kind,
                cert,
                mongo,
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
                mongo,
                DBConfig {
                    uri: "mongodb://localhost:27017".to_string(),
                    db: "stargazer-reborn".to_string(),
                    collection: "tasks".to_string(),
                }
            );
            assert!(!cert.root_certificates.is_empty());
            assert!(!cert.public_cert_chain.is_empty());
            assert!(!cert.private_key.0.is_empty());
            Ok(())
        });
    }
}
