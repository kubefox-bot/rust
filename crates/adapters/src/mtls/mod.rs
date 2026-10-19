mod client;
mod guard;
mod identity;
mod refresh;
mod revocation;
mod server;
mod types;

pub use client::{client_config, client_config_pinned, PinnedPeer};
pub use guard::RevocableIo;
pub use identity::certificate_serial;
pub use refresh::ServerTls;
pub use revocation::{ConnectionTrust, RevokedSerials};
pub use server::server_config;
pub use types::{ClientIdentity, PeerIdentity, ServerIdentity, TlsError, TrustPolicy};
mod local;
pub use local::verify_server_identity;
