//! Certificate related types that supports the secured `WebSocket` transport.
use std::iter;
use std::sync::Arc;

use rustls::server::AllowAnyAuthenticatedClient;
use rustls::{Certificate, ClientConfig, PrivateKey, RootCertStore, ServerConfig};
use rustls_pemfile::Item;
use tokio_rustls::{TlsAcceptor, TlsConnector};
use tracing::{debug, warn};

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
    pub(crate) fn acceptor(self) -> TlsAcceptor {
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
    pub(crate) fn connector(self) -> TlsConnector {
        let client_config = ClientConfig::builder()
            .with_safe_defaults()
            .with_root_certificates(self.root_certificates)
            .with_single_cert(self.public_cert_chain, self.private_key)
            .expect("CFG: invalid client certificate");
        TlsConnector::from(Arc::new(client_config))
    }
}
