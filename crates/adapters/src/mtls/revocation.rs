use super::guard::RevocableIo;
use std::{
    collections::HashSet,
    sync::{Arc, RwLock},
};
use tokio_util::sync::CancellationToken;

struct State {
    serials: HashSet<Vec<u8>>,
    connections: CancellationToken,
}

#[derive(Clone)]
pub struct RevokedSerials(Arc<RwLock<State>>);

/// Capture before the handshake; guard the negotiated plaintext stream with it.
#[derive(Clone)]
pub struct ConnectionTrust(CancellationToken);
impl ConnectionTrust {
    pub fn guard<S>(&self, io: S) -> RevocableIo<S> {
        RevocableIo::new(io, self.0.clone())
    }
    pub async fn cancelled(&self) {
        self.0.cancelled().await;
    }
}
impl Default for RevokedSerials {
    fn default() -> Self {
        Self(Arc::new(RwLock::new(State {
            serials: HashSet::new(),
            connections: CancellationToken::new(),
        })))
    }
}
impl std::fmt::Debug for RevokedSerials {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RevokedSerials")
            .finish_non_exhaustive()
    }
}
impl RevokedSerials {
    pub fn revoke(&self, serial: Vec<u8>) -> bool {
        let mut state = self.0.write().expect("revocation state poisoned");
        if !state.serials.insert(serial) {
            return false;
        }
        state.connections.cancel();
        state.connections = CancellationToken::new();
        true
    }
    pub fn invalidate_connections(&self) {
        let mut state = self.0.write().expect("revocation state poisoned");
        state.connections.cancel();
        state.connections = CancellationToken::new();
    }
    /// The caller also cancels pending handshakes when this lease is invalidated.
    pub fn capture(&self) -> ConnectionTrust {
        let token = self
            .0
            .read()
            .expect("revocation state poisoned")
            .connections
            .clone();
        ConnectionTrust(token)
    }
    pub(super) fn contains(&self, serial: &[u8]) -> bool {
        self.0
            .read()
            .expect("revocation state poisoned")
            .serials
            .contains(serial)
    }
}
