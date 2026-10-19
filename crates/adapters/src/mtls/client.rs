use super::{
    identity::{self, Usage},
    types::HTTP_ALPN,
    ClientIdentity, PeerIdentity, RevokedSerials, TlsError, TrustPolicy,
};
use rustls::{
    client::{
        danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
        Resumption, WebPkiServerVerifier,
    },
    pki_types::{CertificateDer, ServerName, UnixTime},
    ClientConfig, DigitallySignedStruct, RootCertStore, SignatureScheme,
};
use std::sync::Arc;

#[derive(Debug)]
struct Verifier {
    delegate: Arc<WebPkiServerVerifier>,
    expected: PeerIdentity,
    revoked: RevokedSerials,
    pinned_serial: Option<Vec<u8>>,
}
impl ServerCertVerifier for Verifier {
    fn verify_server_cert(
        &self,
        leaf: &CertificateDer<'_>,
        chain: &[CertificateDer<'_>],
        name: &ServerName<'_>,
        ocsp: &[u8],
        now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        let verified = self
            .delegate
            .verify_server_cert(leaf, chain, name, ocsp, now)?;
        identity::verify(leaf, self.expected, Usage::Server, &self.revoked)?;
        if let Some(expected) = &self.pinned_serial {
            if identity::certificate_serial(leaf).map_err(|_| {
                rustls::Error::InvalidCertificate(
                    rustls::CertificateError::ApplicationVerificationFailure,
                )
            })? != *expected
            {
                return Err(rustls::Error::InvalidCertificate(
                    rustls::CertificateError::ApplicationVerificationFailure,
                ));
            }
        }
        Ok(verified)
    }
    fn verify_tls12_signature(
        &self,
        message: &[u8],
        certificate: &CertificateDer<'_>,
        signature: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        self.delegate
            .verify_tls12_signature(message, certificate, signature)
    }
    fn verify_tls13_signature(
        &self,
        message: &[u8],
        certificate: &CertificateDer<'_>,
        signature: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        self.delegate
            .verify_tls13_signature(message, certificate, signature)
    }
    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.delegate.supported_verify_schemes()
    }
}

pub fn client_config(
    identity: &ClientIdentity,
    trust: &TrustPolicy,
    expected: PeerIdentity,
    revoked: &RevokedSerials,
) -> Result<ClientConfig, TlsError> {
    build(identity, trust, expected, revoked, None)
}
pub struct PinnedPeer {
    pub identity: PeerIdentity,
    pub serial: Vec<u8>,
}
pub fn client_config_pinned(
    identity: &ClientIdentity,
    trust: &TrustPolicy,
    peer: &PinnedPeer,
    revoked: &RevokedSerials,
) -> Result<ClientConfig, TlsError> {
    if peer.serial.is_empty() {
        return Err(TlsError::InvalidTrust);
    }
    build(
        identity,
        trust,
        peer.identity,
        revoked,
        Some(peer.serial.clone()),
    )
}
fn build(
    identity: &ClientIdentity,
    trust: &TrustPolicy,
    expected: PeerIdentity,
    revoked: &RevokedSerials,
    pinned_serial: Option<Vec<u8>>,
) -> Result<ClientConfig, TlsError> {
    if trust.roots.is_empty() || expected.id().is_nil() {
        return Err(TlsError::InvalidTrust);
    }
    let mut roots = RootCertStore::empty();
    for root in &trust.roots {
        roots.add(root.clone())?;
    }
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let delegate = WebPkiServerVerifier::builder_with_provider(Arc::new(roots), provider.clone())
        .build()
        .map_err(|_| TlsError::InvalidTrust)?;
    let verifier = Verifier {
        delegate,
        expected,
        revoked: revoked.clone(),
        pinned_serial,
    };
    let mut config = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(verifier))
        .with_client_auth_cert(
            identity.certificates.clone(),
            identity.private_key.clone_key(),
        )?;
    config.resumption = Resumption::disabled();
    config.alpn_protocols = vec![HTTP_ALPN.to_vec()];
    Ok(config)
}
