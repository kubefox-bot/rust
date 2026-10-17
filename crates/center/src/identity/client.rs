use super::IdentityError;
use crate::{agents::AgentCredentials, auth::SessionLease, storage::CenterStore};
use hmail_cleaner_adapters::mtls::{self, ClientIdentity, PeerIdentity, PinnedPeer, TrustPolicy};
use rustls::pki_types::{pem::PemObject, CertificateDer, PrivateKeyDer};
pub(crate) async fn configuration(
    store: &CenterStore,
    credentials: &AgentCredentials,
    lease: &SessionLease,
) -> Result<rustls::ClientConfig, IdentityError> {
    let serial = super::pending::current(store, credentials.agent_id).await?;
    build(
        store,
        credentials,
        lease,
        Pin::Current(super::serial_bytes(&serial)?),
    )
    .await
}
pub(crate) async fn pending_configuration(
    store: &CenterStore,
    credentials: &AgentCredentials,
    lease: &SessionLease,
) -> Result<rustls::ClientConfig, IdentityError> {
    let current = super::pending::current(store, credentials.agent_id).await?;
    let pending = super::pending::load(store, credentials.agent_id)
        .await?
        .ok_or(IdentityError)?;
    if pending.expected_serial != current {
        return Err(IdentityError);
    }
    build(store, credentials, lease, Pin::Renewal(pending.issued)).await
}
enum Pin {
    Current(Vec<u8>),
    Renewal(super::IssuedAgent),
}
async fn build(
    store: &CenterStore,
    credentials: &AgentCredentials,
    lease: &SessionLease,
    pin: Pin,
) -> Result<rustls::ClientConfig, IdentityError> {
    let stored = super::client_material::load(store, lease).await?;
    if credentials.center_id != stored.material.vault_id() {
        return Err(IdentityError);
    }
    let secret = stored.secret;
    let certificates = CertificateDer::pem_slice_iter(secret.certificate_pem.as_bytes())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| IdentityError)?;
    let private_key = PrivateKeyDer::from_pem_slice(secret.private_key_pem.as_bytes())
        .map_err(|_| IdentityError)?;
    let roots = CertificateDer::pem_slice_iter(secret.ca_pem.as_bytes())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| IdentityError)?;
    let serial = match pin {
        Pin::Current(serial) => serial,
        Pin::Renewal(candidate) => {
            let candidate_chain =
                CertificateDer::pem_slice_iter(candidate.certificate_pem.as_bytes())
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|_| IdentityError)?;
            let host = credentials.endpoint.host().map_err(|_| IdentityError)?;
            let name = rustls::pki_types::ServerName::try_from(host).map_err(|_| IdentityError)?;
            mtls::verify_server_identity(
                &candidate_chain,
                &TrustPolicy {
                    roots: roots.clone(),
                },
                PeerIdentity::Agent(credentials.agent_id.uuid()),
                &name,
            )
            .map_err(|_| IdentityError)?;
            let leaf = candidate_chain.first().ok_or(IdentityError)?;
            let (_, parsed) =
                x509_parser::parse_x509_certificate(leaf).map_err(|_| IdentityError)?;
            if mtls::certificate_serial(leaf).map_err(|_| IdentityError)? != candidate.serial
                || parsed.validity().not_after.timestamp() != candidate.expires_at_unix
            {
                return Err(IdentityError);
            }
            candidate.serial
        }
    };
    let revoked: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM revoked_serials WHERE serial=?)")
            .bind(super::serial_text(&serial))
            .fetch_one(&store.pool)
            .await
            .map_err(|_| IdentityError)?;
    if revoked {
        return Err(IdentityError);
    }
    mtls::client_config_pinned(
        &ClientIdentity {
            certificates,
            private_key,
        },
        &TrustPolicy { roots },
        &PinnedPeer {
            identity: PeerIdentity::Agent(credentials.agent_id.uuid()),
            serial,
        },
        &store.revoked,
    )
    .map_err(|_| IdentityError)
}
