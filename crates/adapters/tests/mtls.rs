use hmail_cleaner_adapters::mtls::{
    self, ClientIdentity, PeerIdentity, RevokedSerials, TrustPolicy,
};
use rcgen::{
    BasicConstraints, CertificateParams, CertifiedIssuer, ExtendedKeyUsagePurpose, IsCa, KeyPair,
    KeyUsagePurpose, SanType,
};
use rustls::{
    pki_types::{PrivatePkcs8KeyDer, ServerName},
    ClientConfig, ServerConfig,
};
use std::{sync::Arc, time::Duration};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_rustls::{TlsAcceptor, TlsConnector};
use uuid::Uuid;

const TEST_TIMEOUT: Duration = Duration::from_secs(5);

fn authority() -> CertifiedIssuer<'static, KeyPair> {
    let mut params = CertificateParams::new(Vec::<String>::new()).unwrap();
    params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    params.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];
    CertifiedIssuer::self_signed(params, KeyPair::generate().unwrap()).unwrap()
}

fn leaf(
    ca: &CertifiedIssuer<'_, KeyPair>,
    peer: PeerIdentity,
    eku: Option<ExtendedKeyUsagePurpose>,
    expired: bool,
) -> ClientIdentity {
    let mut params = CertificateParams::new(vec!["agent.internal".into()]).unwrap();
    params
        .subject_alt_names
        .push(SanType::URI(peer.uri().try_into().unwrap()));
    params.extended_key_usages = eku.into_iter().collect();
    params.serial_number = Some(42u64.into());
    if expired {
        params.not_before = rcgen::date_time_ymd(1999, 1, 1);
        params.not_after = rcgen::date_time_ymd(2000, 1, 1);
    }
    let key = KeyPair::generate().unwrap();
    let certificate = params.signed_by(&key, ca).unwrap();
    ClientIdentity {
        certificates: vec![certificate.der().clone()],
        private_key: PrivatePkcs8KeyDer::from(key.serialize_der()).into(),
    }
}

async fn roundtrip(
    server: ServerConfig,
    client: ClientConfig,
    name: &'static str,
) -> (bool, usize) {
    let (client_io, server_io) = tokio::io::duplex(16 * 1024);
    let server_task = async {
        let Ok(mut connection) = TlsAcceptor::from(Arc::new(server)).accept(server_io).await else {
            return 0;
        };
        let mut profile = [0; 7];
        if connection.read_exact(&mut profile).await.is_err() {
            return 0;
        }
        if profile == *b"PROFILE" {
            let _ = connection.write_all(b"OK").await;
            return profile.len();
        }
        0
    };
    let client_task = async {
        let connector = TlsConnector::from(Arc::new(client));
        let Ok(mut connection) = connector
            .connect(ServerName::try_from(name).unwrap(), client_io)
            .await
        else {
            return false;
        };
        if connection.write_all(b"PROFILE").await.is_err() {
            return false;
        }
        let mut acknowledgment = [0; 2];
        connection.read_exact(&mut acknowledgment).await.is_ok() && acknowledgment == *b"OK"
    };
    tokio::time::timeout(TEST_TIMEOUT, async {
        let (bytes, success) = tokio::join!(server_task, client_task);
        (success, bytes)
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn valid_center_and_expected_agent_exchange_a_profile() {
    let ca = authority();
    let agent = PeerIdentity::Agent(Uuid::new_v4());
    let center = PeerIdentity::Center(Uuid::new_v4());
    let trust = TrustPolicy {
        roots: vec![ca.der().clone()],
    };
    let server = leaf(&ca, agent, Some(ExtendedKeyUsagePurpose::ServerAuth), false);
    let client = leaf(
        &ca,
        center,
        Some(ExtendedKeyUsagePurpose::ClientAuth),
        false,
    );
    let revoked = RevokedSerials::default();
    let outcome = roundtrip(
        mtls::server_config(&server, &trust, center, &revoked).unwrap(),
        mtls::client_config(&client, &trust, agent, &revoked).unwrap(),
        "agent.internal",
    )
    .await;
    assert_eq!(outcome, (true, 7));
}

#[tokio::test]
async fn wrong_node_role_hostname_expiry_and_eku_are_rejected_before_profile_bytes() {
    let ca = authority();
    let agent = PeerIdentity::Agent(Uuid::new_v4());
    let center = PeerIdentity::Center(Uuid::new_v4());
    let trust = TrustPolicy {
        roots: vec![ca.der().clone()],
    };
    for (peer, eku, expired, name) in [
        (
            PeerIdentity::Agent(Uuid::new_v4()),
            Some(ExtendedKeyUsagePurpose::ServerAuth),
            false,
            "agent.internal",
        ),
        (
            PeerIdentity::Center(agent.id()),
            Some(ExtendedKeyUsagePurpose::ServerAuth),
            false,
            "agent.internal",
        ),
        (
            agent,
            Some(ExtendedKeyUsagePurpose::ServerAuth),
            false,
            "other.internal",
        ),
        (
            agent,
            Some(ExtendedKeyUsagePurpose::ServerAuth),
            true,
            "agent.internal",
        ),
        (
            agent,
            Some(ExtendedKeyUsagePurpose::ClientAuth),
            false,
            "agent.internal",
        ),
        (agent, None, false, "agent.internal"),
    ] {
        let server = leaf(&ca, peer, eku, expired);
        let client = leaf(
            &ca,
            center,
            Some(ExtendedKeyUsagePurpose::ClientAuth),
            false,
        );
        let revoked = RevokedSerials::default();
        assert_eq!(
            roundtrip(
                mtls::server_config(&server, &trust, center, &revoked).unwrap(),
                mtls::client_config(&client, &trust, agent, &revoked).unwrap(),
                name
            )
            .await,
            (false, 0)
        );
    }
}

#[tokio::test]
async fn missing_wrong_center_and_revoked_certificates_are_rejected() {
    let ca = authority();
    let agent = PeerIdentity::Agent(Uuid::new_v4());
    let center = PeerIdentity::Center(Uuid::new_v4());
    let trust = TrustPolicy {
        roots: vec![ca.der().clone()],
    };
    for peer in [
        PeerIdentity::Center(Uuid::new_v4()),
        PeerIdentity::Agent(center.id()),
    ] {
        let revoked = RevokedSerials::default();
        let server = leaf(&ca, agent, Some(ExtendedKeyUsagePurpose::ServerAuth), false);
        let client = leaf(&ca, peer, Some(ExtendedKeyUsagePurpose::ClientAuth), false);
        assert_eq!(
            roundtrip(
                mtls::server_config(&server, &trust, center, &revoked).unwrap(),
                mtls::client_config(&client, &trust, agent, &revoked).unwrap(),
                "agent.internal"
            )
            .await,
            (false, 0)
        );
    }
    let server = leaf(&ca, agent, Some(ExtendedKeyUsagePurpose::ServerAuth), false);
    let client = leaf(
        &ca,
        center,
        Some(ExtendedKeyUsagePurpose::ClientAuth),
        false,
    );
    let revoked = RevokedSerials::default();
    let config = mtls::client_config(&client, &trust, agent, &revoked).unwrap();
    revoked.revoke(mtls::certificate_serial(&server.certificates[0]).unwrap());
    assert_eq!(
        roundtrip(
            mtls::server_config(&server, &trust, center, &revoked).unwrap(),
            config,
            "agent.internal"
        )
        .await,
        (false, 0)
    );
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut roots = rustls::RootCertStore::empty();
    roots.add(ca.der().clone()).unwrap();
    let anonymous = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_root_certificates(roots)
        .with_no_client_auth();
    assert_eq!(
        roundtrip(
            mtls::server_config(&server, &trust, center, &RevokedSerials::default()).unwrap(),
            anonymous,
            "agent.internal"
        )
        .await,
        (false, 0)
    );
}

#[tokio::test]
async fn untrusted_ca_is_rejected_even_when_identity_matches() {
    let trusted = authority();
    let other = authority();
    let agent = PeerIdentity::Agent(Uuid::new_v4());
    let center = PeerIdentity::Center(Uuid::new_v4());
    let trust = TrustPolicy {
        roots: vec![trusted.der().clone()],
    };
    let server = leaf(
        &other,
        agent,
        Some(ExtendedKeyUsagePurpose::ServerAuth),
        false,
    );
    let client = leaf(
        &trusted,
        center,
        Some(ExtendedKeyUsagePurpose::ClientAuth),
        false,
    );
    let revoked = RevokedSerials::default();
    assert_eq!(
        roundtrip(
            mtls::server_config(&server, &trust, center, &revoked).unwrap(),
            mtls::client_config(&client, &trust, agent, &revoked).unwrap(),
            "agent.internal"
        )
        .await,
        (false, 0)
    );
}

#[tokio::test]
async fn revocation_wakes_and_closes_existing_idle_connections() {
    let revoked = RevokedSerials::default();
    let (socket, _other) = tokio::io::duplex(16);
    let lease = revoked.capture();
    let mut connection = lease.guard(socket);
    let pending = tokio::spawn(async move { connection.read_exact(&mut [0u8; 1]).await });
    tokio::task::yield_now().await;
    revoked.revoke(vec![42]);
    assert!(tokio::time::timeout(TEST_TIMEOUT, pending)
        .await
        .unwrap()
        .unwrap()
        .is_err());
}

#[tokio::test]
async fn refresh_invalidates_old_leases_and_uses_the_new_certificate() {
    let ca = authority();
    let agent = PeerIdentity::Agent(Uuid::new_v4());
    let center = PeerIdentity::Center(Uuid::new_v4());
    let trust = TrustPolicy {
        roots: vec![ca.der().clone()],
    };
    let revoked = RevokedSerials::default();
    let original = leaf(&ca, agent, Some(ExtendedKeyUsagePurpose::ServerAuth), false);
    let config = mtls::server_config(&original, &trust, center, &revoked).unwrap();
    let live = mtls::ServerTls::new(config, revoked.clone());
    let (_, old_lease) = live.snapshot();
    let replacement = leaf(&ca, agent, Some(ExtendedKeyUsagePurpose::ServerAuth), false);
    live.replace(mtls::server_config(&replacement, &trust, center, &revoked).unwrap());
    tokio::time::timeout(TEST_TIMEOUT, old_lease.cancelled())
        .await
        .unwrap();
    let (config, lease) = live.snapshot();
    let client = leaf(
        &ca,
        center,
        Some(ExtendedKeyUsagePurpose::ClientAuth),
        false,
    );
    assert_eq!(
        roundtrip(
            (*config).clone(),
            mtls::client_config(&client, &trust, agent, &revoked).unwrap(),
            "agent.internal"
        )
        .await,
        (true, 7)
    );
    let (socket, mut other) = tokio::io::duplex(16);
    other.write_all(b"x").await.unwrap();
    let mut guarded = lease.guard(socket);
    revoked.invalidate_connections();
    assert!(
        guarded.read_exact(&mut [0; 1]).await.is_err(),
        "even buffered plaintext must be rejected"
    );
}

#[tokio::test]
async fn invalid_client_usage_expiry_and_revocation_reject_profile_bytes() {
    let ca = authority();
    let agent = PeerIdentity::Agent(Uuid::new_v4());
    let center = PeerIdentity::Center(Uuid::new_v4());
    let trust = TrustPolicy {
        roots: vec![ca.der().clone()],
    };
    for (eku, expired, revoke) in [
        (Some(ExtendedKeyUsagePurpose::ServerAuth), false, false),
        (None, false, false),
        (Some(ExtendedKeyUsagePurpose::ClientAuth), true, false),
        (Some(ExtendedKeyUsagePurpose::ClientAuth), false, true),
    ] {
        let server = leaf(&ca, agent, Some(ExtendedKeyUsagePurpose::ServerAuth), false);
        let client = leaf(&ca, center, eku, expired);
        let server_revoked = RevokedSerials::default();
        let server_config = mtls::server_config(&server, &trust, center, &server_revoked).unwrap();
        if revoke {
            server_revoked.revoke(mtls::certificate_serial(&client.certificates[0]).unwrap());
        }
        assert_eq!(
            roundtrip(
                server_config,
                mtls::client_config(&client, &trust, agent, &RevokedSerials::default()).unwrap(),
                "agent.internal"
            )
            .await,
            (false, 0)
        );
    }
}

#[test]
fn identities_are_bound_to_their_explicit_role() {
    let id = Uuid::parse_str("20000000-0000-4000-8000-000000000002").unwrap();
    assert_eq!(
        PeerIdentity::Agent(id).uri(),
        "urn:hmail-cleaner:agent:20000000-0000-4000-8000-000000000002"
    );
}
