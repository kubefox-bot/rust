mod control;
mod persistence;
mod validation;
pub(crate) use control::IdentityManager;
use hmail_cleaner_adapters::mtls::RevokedSerials;
use hmail_cleaner_protocol::AgentId;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tokio_rustls::rustls::pki_types::CertificateDer;
pub(crate) use validation::fingerprint;
pub(crate) struct IdentityBootstrap {
    pub directory: PathBuf,
    pub agent_id: AgentId,
    pub center_id: uuid::Uuid,
    pub roots: Vec<CertificateDer<'static>>,
    pub key: Arc<rcgen::KeyPair>,
    pub revoked: RevokedSerials,
    pub fingerprint: Mutex<[u8; 32]>,
}
#[cfg(test)]
mod tests;
