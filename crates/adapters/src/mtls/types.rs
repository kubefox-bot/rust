use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use uuid::Uuid;

pub(super) const HTTP_ALPN: &[u8] = b"http/1.1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerIdentity {
    Center(Uuid),
    Agent(Uuid),
}
impl PeerIdentity {
    pub fn id(self) -> Uuid {
        match self {
            Self::Center(id) | Self::Agent(id) => id,
        }
    }
    pub fn uri(self) -> String {
        match self {
            Self::Center(id) => format!("urn:hmail-cleaner:center:{id}"),
            Self::Agent(id) => format!("urn:hmail-cleaner:agent:{id}"),
        }
    }
}

pub struct ClientIdentity {
    pub certificates: Vec<CertificateDer<'static>>,
    pub private_key: PrivateKeyDer<'static>,
}
pub type ServerIdentity = ClientIdentity;

pub struct TrustPolicy {
    pub roots: Vec<CertificateDer<'static>>,
}

#[derive(Debug, thiserror::Error)]
pub enum TlsError {
    #[error("TLS configuration failed: {0}")]
    Configuration(#[from] rustls::Error),
    #[error("invalid trust anchors or peer identity")]
    InvalidTrust,
    #[error("invalid certificate encoding")]
    InvalidCertificate,
}
