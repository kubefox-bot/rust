use super::{
    identity::{self, Usage},
    types::HTTP_ALPN,
    PeerIdentity, RevokedSerials, ServerIdentity, TlsError, TrustPolicy,
};
use rustls::{
    client::danger::HandshakeSignatureValid,
    pki_types::{CertificateDer, UnixTime},
    server::{
        danger::{ClientCertVerified, ClientCertVerifier},
        WebPkiClientVerifier,
    },
    DigitallySignedStruct, DistinguishedName, RootCertStore, ServerConfig, SignatureScheme,
};
use std::sync::Arc;

#[derive(Debug)]
struct Verifier {
    delegate: Arc<dyn ClientCertVerifier>,
    expected: PeerIdentity,
    revoked: RevokedSerials,
}
impl ClientCertVerifier for Verifier {
    fn root_hint_subjects(&self) -> &[DistinguishedName] {
        self.delegate.root_hint_subjects()
    }
    fn verify_client_cert(
        &self,
        leaf: &CertificateDer<'_>,
        chain: &[CertificateDer<'_>],
        now: UnixTime,
    ) -> Result<ClientCertVerified, rustls::Error> {
        let verified = self.delegate.verify_client_cert(leaf, chain, now)?;
        identity::verify(leaf, self.expected, Usage::Client, &self.revoked)?;
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

pub fn server_config(
    identity: &ServerIdentity,
    trust: &TrustPolicy,
    expected: PeerIdentity,
    revoked: &RevokedSerials,
) -> Result<ServerConfig, TlsError> {
    if trust.roots.is_empty() || expected.id().is_nil() {
        return Err(TlsError::InvalidTrust);
    }
    let mut roots = RootCertStore::empty();
    for root in &trust.roots {
        roots.add(root.clone())?;
    }
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let delegate = WebPkiClientVerifier::builder_with_provider(Arc::new(roots), provider.clone())
        .build()
        .map_err(|_| TlsError::InvalidTrust)?;
    let mut config = ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()?
        .with_client_cert_verifier(Arc::new(Verifier {
            delegate,
            expected,
            revoked: revoked.clone(),
        }))
        .with_single_cert(
            identity.certificates.clone(),
            identity.private_key.clone_key(),
        )?;
    config.session_storage = Arc::new(rustls::server::NoServerSessionStorage {});
    config.send_tls13_tickets = 0;
    config.alpn_protocols = vec![HTTP_ALPN.to_vec()];
    Ok(config)
}
