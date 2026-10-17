use super::IdentityBootstrap;
use crate::execution::AgentError;
use hmail_cleaner_adapters::mtls::{self, ClientIdentity, PeerIdentity, TrustPolicy};
use hmail_cleaner_protocol::{AgentIdentityInstall, IdentityReceipt, PROTOCOL_VERSION};
use tokio_rustls::rustls::{
    pki_types::{pem::PemObject, CertificateDer, PrivatePkcs8KeyDer, ServerName},
    ServerConfig,
};
pub(super) const CERTIFICATE_BYTES: usize = 64 * 1024;
pub(super) struct Candidate {
    pub tls: ServerConfig,
    pub receipt: IdentityReceipt,
    pub fingerprint: [u8; 32],
}
pub(crate) fn fingerprint(certificate: &CertificateDer<'_>) -> [u8; 32] {
    let mut output = [0; 32];
    output.copy_from_slice(
        ring::digest::digest(&ring::digest::SHA256, certificate.as_ref()).as_ref(),
    );
    output
}
pub(super) fn prepare(
    bootstrap: &IdentityBootstrap,
    input: &AgentIdentityInstall,
) -> Result<Candidate, AgentError> {
    if input.protocol_version != PROTOCOL_VERSION
        || input.agent_id != bootstrap.agent_id
        || input.certificate_pem.len() > CERTIFICATE_BYTES
        || input.server_name.len() > 253
    {
        return Err(AgentError::Invalid);
    }
    let certificates = CertificateDer::pem_slice_iter(input.certificate_pem.as_bytes())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| AgentError::Invalid)?;
    let identity = ClientIdentity {
        certificates,
        private_key: PrivatePkcs8KeyDer::from(bootstrap.key.serialize_der()).into(),
    };
    let trust = TrustPolicy {
        roots: bootstrap.roots.clone(),
    };
    let name = ServerName::try_from(input.server_name.as_str()).map_err(|_| AgentError::Invalid)?;
    mtls::verify_server_identity(
        &identity.certificates,
        &trust,
        PeerIdentity::Agent(bootstrap.agent_id.uuid()),
        &name,
    )
    .map_err(|_| AgentError::Invalid)?;
    let tls = mtls::server_config(
        &identity,
        &trust,
        PeerIdentity::Center(bootstrap.center_id),
        &bootstrap.revoked,
    )
    .map_err(|_| AgentError::Invalid)?;
    let certificate = identity.certificates.first().ok_or(AgentError::Invalid)?;
    Ok(Candidate {
        tls,
        receipt: IdentityReceipt {
            agent_id: bootstrap.agent_id,
            serial: mtls::certificate_serial(certificate).map_err(|_| AgentError::Invalid)?,
        },
        fingerprint: fingerprint(certificate),
    })
}
