use hmail_cleaner_protocol::AgentId;
use std::path::PathBuf;

pub struct AgentBootstrap {
    pub data_directory: PathBuf,
    pub agent_id: AgentId,
}

pub use hmail_cleaner_protocol::AgentCsr;

#[derive(Debug, thiserror::Error)]
pub enum EnrollmentError {
    #[error("agent enrollment file operation failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("agent enrollment cryptography failed: {0}")]
    Crypto(#[from] rcgen::Error),
    #[error("invalid agent enrollment metadata")]
    Metadata,
    #[error("agent directory or private key permissions are insufficient")]
    Permissions,
    #[error("agent identity differs from the requested identity")]
    IdentityConflict,
    #[error("partial enrollment requires operator attention")]
    Partial,
}
