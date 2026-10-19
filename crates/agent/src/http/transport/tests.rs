use super::*;
use crate::{
    execution::{NativeExecutor, Worker},
    http::router,
    jobs::JobRepository,
};
use hmail_cleaner_adapters::mtls::{
    self, ClientIdentity, PeerIdentity, RevokedSerials, TrustPolicy,
};
use hmail_cleaner_protocol::AgentId;
use rcgen::{
    BasicConstraints, CertificateParams, CertifiedIssuer, ExtendedKeyUsagePurpose, IsCa, KeyPair,
    KeyUsagePurpose, SanType,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_rustls::{
    rustls::pki_types::{PrivatePkcs8KeyDer, ServerName},
    TlsConnector,
};
use uuid::Uuid;
const TEST_TIMEOUT: Duration = Duration::from_secs(5);

fn leaf(
    ca: &CertifiedIssuer<'_, KeyPair>,
    peer: PeerIdentity,
    usage: ExtendedKeyUsagePurpose,
) -> ClientIdentity {
    let mut params = CertificateParams::new(vec!["agent.internal".into()]).unwrap();
    params
        .subject_alt_names
        .push(SanType::URI(peer.uri().try_into().unwrap()));
    params.extended_key_usages = vec![usage];
    let key = KeyPair::generate().unwrap();
    ClientIdentity {
        certificates: vec![params.signed_by(&key, ca).unwrap().der().clone()],
        private_key: PrivatePkcs8KeyDer::from(key.serialize_der()).into(),
    }
}
#[tokio::test]
async fn http_health_is_available_over_the_actual_mutually_authenticated_acceptor() {
    let mut params = CertificateParams::new(Vec::<String>::new()).unwrap();
    params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    params.key_usages = vec![KeyUsagePurpose::KeyCertSign];
    let ca = CertifiedIssuer::self_signed(params, KeyPair::generate().unwrap()).unwrap();
    let agent = AgentId::new();
    let agent_peer = PeerIdentity::Agent(agent.uuid());
    let center = PeerIdentity::Center(Uuid::new_v4());
    let trust = TrustPolicy {
        roots: vec![ca.der().clone()],
    };
    let server_identity = leaf(&ca, agent_peer, ExtendedKeyUsagePurpose::ServerAuth);
    let client_identity = leaf(&ca, center, ExtendedKeyUsagePurpose::ClientAuth);
    let revoked = RevokedSerials::default();
    let tls = ServerTls::new(
        mtls::server_config(&server_identity, &trust, center, &revoked).unwrap(),
        revoked.clone(),
    );
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let repo = Arc::new(
        JobRepository::open(&dir.path().join("jobs.sqlite"))
            .await
            .unwrap(),
    );
    let worker = Worker::new(agent, repo, NativeExecutor);
    let handle = axum_server::Handle::new();
    let server = server(listener, tls, handle.clone()).unwrap();
    let task = tokio::spawn(server.serve(router(worker).into_make_service()));
    let result = tokio::time::timeout(TEST_TIMEOUT, async {
        let tcp = TcpStream::connect(address).await.unwrap();
        let client = mtls::client_config(&client_identity, &trust, agent_peer, &revoked).unwrap();
        let mut tls = TlsConnector::from(Arc::new(client))
            .connect(ServerName::try_from("agent.internal").unwrap(), tcp)
            .await
            .unwrap();
        tls.write_all(
            b"GET /agent/v1/health HTTP/1.1\r\nHost: agent.internal\r\nConnection: close\r\n\r\n",
        )
        .await
        .unwrap();
        let mut response = Vec::new();
        tls.read_to_end(&mut response).await.unwrap();
        String::from_utf8(response).unwrap()
    })
    .await;
    handle.shutdown();
    task.await.unwrap().unwrap();
    let response = result.unwrap();
    assert!(response.starts_with("HTTP/1.1 200"), "{response}");
    assert!(response.contains(&agent.to_string()));
    assert!(response.contains("\"healthy\":true"));
}
