//! DNS resolver implementations.

use std::io;
use std::net::{SocketAddr, ToSocketAddrs};

/// A DNS resolver resolves a hostname to a list of addresses.
pub trait DNSResolver: Send + Sync + Clone + 'static {
    /// Resolve a hostname to a list of addresses.
    ///
    /// This method should not panic.
    ///
    /// # Errors
    /// Return an error if the hostname cannot be resolved.
    fn resolve(&self, domain: &str, port: u16) -> Result<Vec<SocketAddr>, io::Error>;
}

/// A DNS resolver that uses the system's resolver.
#[derive(Copy, Clone)]
pub struct StdResolver;

impl DNSResolver for StdResolver {
    fn resolve(&self, domain: &str, port: u16) -> Result<Vec<SocketAddr>, io::Error> {
        Ok((domain, port).to_socket_addrs()?.collect())
    }
}

/// A mock DNS resolver that resolves all domains to localhost.
///
/// Only used for testing.
#[derive(Copy, Clone)]
pub struct MockResolver;

impl DNSResolver for MockResolver {
    fn resolve(&self, _: &str, port: u16) -> Result<Vec<SocketAddr>, io::Error> {
        Ok(vec![SocketAddr::from(([127, 0, 0, 1], port))])
    }
}
