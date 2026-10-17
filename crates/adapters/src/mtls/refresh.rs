use super::{ConnectionTrust, RevokedSerials};
use rustls::ServerConfig;
use std::sync::{Arc, RwLock};

/// Owns a server's configuration and the lifetime of connections using it.
#[derive(Clone)]
pub struct ServerTls {
    config: Arc<RwLock<Arc<ServerConfig>>>,
    revoked: RevokedSerials,
}

impl ServerTls {
    pub fn new(config: ServerConfig, revoked: RevokedSerials) -> Self {
        Self {
            config: Arc::new(RwLock::new(Arc::new(config))),
            revoked,
        }
    }

    /// Capture before starting TLS; cancel the handshake and guard its plaintext
    /// with this lease. Holding the read lock prevents a stale-config/new-lease race.
    pub fn snapshot(&self) -> (Arc<ServerConfig>, ConnectionTrust) {
        let config = self.config.read().expect("TLS configuration poisoned");
        (Arc::clone(&config), self.revoked.capture())
    }

    pub fn replace(&self, config: ServerConfig) {
        let mut current = self.config.write().expect("TLS configuration poisoned");
        *current = Arc::new(config);
        self.revoked.invalidate_connections();
    }
}
