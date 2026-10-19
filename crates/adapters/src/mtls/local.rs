use super::{PeerIdentity, RevokedSerials, TlsError, TrustPolicy};
use rustls::{
    client::{danger::ServerCertVerifier, WebPkiServerVerifier},
    pki_types::{ServerName, UnixTime},
    RootCertStore,
};
use std::sync::Arc;
/// Verify an installed or proposed local server certificate before activating it.
/// The caller also builds ServerConfig to verify the certificate/private-key pair.
pub fn verify_server_identity(
    certificates: &[rustls::pki_types::CertificateDer<'_>],
    trust: &TrustPolicy,
    expected: PeerIdentity,
    name: &ServerName<'_>,
) -> Result<(), TlsError> {
    let (leaf, chain) = certificates
        .split_first()
        .ok_or(TlsError::InvalidCertificate)?;
    let mut roots = RootCertStore::empty();
    for root in &trust.roots {
        roots.add(root.clone())?;
    }
    let verifier = WebPkiServerVerifier::builder_with_provider(
        Arc::new(roots),
        Arc::new(rustls::crypto::ring::default_provider()),
    )
    .build()
    .map_err(|_| TlsError::InvalidTrust)?;
    verifier.verify_server_cert(leaf, chain, name, &[], UnixTime::now())?;
    super::identity::verify(
        leaf,
        expected,
        super::identity::Usage::Server,
        &RevokedSerials::default(),
    )?;
    Ok(())
}
