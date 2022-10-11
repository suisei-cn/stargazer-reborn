//! Certificate related types that supports the secured `WebSocket` transport.
use std::{fs::File, io, io::BufReader, path::PathBuf, sync::Arc};

use eyre::{bail, eyre, Result, WrapErr};
use rustls::{
    server::AllowAnyAuthenticatedClient,
    Certificate,
    ClientConfig,
    PrivateKey,
    RootCertStore,
    ServerConfig,
};
use rustls_pemfile::Item;
use serde::{de::Error, Deserialize, Deserializer};
use tokio_rustls::{TlsAcceptor, TlsConnector};
use tracing::{debug, warn};

/// Certificates used by a client or a server.
#[derive(Debug, Clone)]
pub struct Certificates {
    /// Trusted root certificates.
    pub(crate) root_certificates: RootCertStore,
    /// Public certificate chain.
    pub(crate) public_cert_chain: Vec<Certificate>,
    /// Private key.
    pub(crate) private_key: PrivateKey,
}

impl Certificates {
    /// Create a new `Certificates` instance with given PEM files.
    ///
    /// `root`: CA certificate in PEM format.
    /// `cert`: public certificate and private key in PEM format.
    ///
    /// # Errors
    /// Returns error if no certificate or key is found in given `cert` file.
    #[allow(clippy::cognitive_complexity)]
    pub fn from_pem(root: &mut impl io::BufRead, cert: &mut impl io::BufRead) -> Result<Self> {
        let mut root_certs = vec![];
        while let Some(section) =
            rustls_pemfile::read_one(root).wrap_err("Corrupt root PEM file.")?
        {
            if let Item::X509Certificate(cert) = section {
                root_certs.push(cert);
            } else {
                warn!("Section not handled in given PEM file.");
            }
        }

        let mut public_cert_chain = vec![];
        let mut private_key = None;
        while let Some(section) =
            rustls_pemfile::read_one(cert).wrap_err("Corrupt cert PEM file.")?
        {
            match section {
                Item::X509Certificate(cert) => public_cert_chain.push(Certificate(cert)),
                Item::PKCS8Key(key) => private_key = Some(PrivateKey(key)),
                _ => warn!("Section not handled in given PEM file."),
            }
        }

        let mut root_certificates = RootCertStore::empty();
        let (succ, _) = root_certificates.add_parsable_certificates(&root_certs);
        debug!("{} root certificates added", succ);

        if public_cert_chain.is_empty() {
            bail!("No public certificate found in given PEM file.");
        }
        let private_key =
            private_key.ok_or_else(|| eyre!("No private key found in given PEM file."))?;

        Ok(Self {
            root_certificates,
            public_cert_chain,
            private_key,
        })
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

/// Helper struct for deserializing a certificate from PEM files.
#[derive(Debug, Deserialize)]
struct CertificatesFromFile {
    /// Path to the client server TLS CA PEM file.
    ca: PathBuf,
    /// Path to the client server TLS certificate & key PEM file.
    cert: PathBuf,
}

/// Helper function for deserializing a certificate from PEM files.
pub fn deserialize<'de, D>(de: D) -> Result<Certificates, D::Error>
where
    D: Deserializer<'de>,
{
    let cert_from_file = CertificatesFromFile::deserialize(de)?;
    let mut ca = BufReader::new(File::open(cert_from_file.ca).map_err(D::Error::custom)?);
    let mut cert = BufReader::new(File::open(cert_from_file.cert).map_err(D::Error::custom)?);
    Certificates::from_pem(&mut ca, &mut cert).map_err(D::Error::custom)
}
