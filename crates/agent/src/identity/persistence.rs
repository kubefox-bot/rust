use super::{
    validation::{self, Candidate},
    IdentityBootstrap,
};
use crate::execution::AgentError;
use tokio_rustls::rustls::pki_types::{pem::PemObject, CertificateDer};
pub(super) const CERTIFICATE_FILE: &str = "identity.pem";
pub(super) fn current(
    bootstrap: &IdentityBootstrap,
    candidate: &Candidate,
) -> Result<bool, AgentError> {
    let bytes = crate::files::read(
        &bootstrap.directory.join(CERTIFICATE_FILE),
        validation::CERTIFICATE_BYTES as u64,
    )?;
    let certificate =
        CertificateDer::from_pem_slice(&bytes).map_err(|_| AgentError::Unavailable)?;
    let disk = validation::fingerprint(&certificate);
    let current = bootstrap
        .fingerprint
        .lock()
        .map_err(|_| AgentError::Unavailable)?;
    if disk != *current {
        return Err(AgentError::Unavailable);
    }
    Ok(disk == candidate.fingerprint)
}
pub(super) fn persist(bootstrap: &IdentityBootstrap, pem: &str) -> Result<(), AgentError> {
    crate::files::replace(&bootstrap.directory, CERTIFICATE_FILE, pem.as_bytes())?;
    Ok(())
}
