use super::CenterFixture;
use axum::{
    routing::{get, post},
    Json, Router,
};
use hmail_cleaner_adapters::mtls::{
    self, ClientIdentity, PeerIdentity, RevokedSerials, TrustPolicy,
};
use hmail_cleaner_center::{
    agents::{AgentCredentials, AgentEndpoint},
    identity::IdentityService,
};
use hmail_cleaner_protocol::{AgentHealth, AgentId, ObservedMailState, ProfileValidationResponse};
use rustls::pki_types::{pem::PemObject, CertificateDer, PrivatePkcs8KeyDer};
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc,
};
pub struct TlsAgent {
    pub credentials: AgentCredentials,
    pub received: Arc<AtomicUsize>,
    pub health_calls: Arc<AtomicUsize>,
    pub idle: Arc<AtomicBool>,
    handle: axum_server::Handle<std::net::SocketAddr>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for TlsAgent {
    fn drop(&mut self) {
        self.handle.shutdown();
        self.task.abort();
    }
}
pub async fn start(center: &CenterFixture, id: AgentId) -> TlsAgent {
    start_router(center, id, Router::new(), uuid::Uuid::new_v4()).await
}
pub(crate) async fn start_router(
    center: &CenterFixture,
    id: AgentId,
    extra: Router,
    epoch: uuid::Uuid,
) -> TlsAgent {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    let (key, csr) = super::csr();
    let issued = IdentityService::new(center.store.clone())
        .issue_agent(id, "127.0.0.1", &csr, &center.lease)
        .await
        .unwrap();
    let roots = CertificateDer::pem_slice_iter(issued.ca_pem.as_bytes())
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let certificates = CertificateDer::pem_slice_iter(issued.certificate_pem.as_bytes())
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let identity = ClientIdentity {
        certificates,
        private_key: PrivatePkcs8KeyDer::from(key.serialize_der()).into(),
    };
    let center_id = center
        .store
        .account()
        .await
        .unwrap()
        .unwrap()
        .material
        .vault_id();
    let tls = mtls::server_config(
        &identity,
        &TrustPolicy { roots },
        PeerIdentity::Center(center_id),
        &RevokedSerials::default(),
    )
    .unwrap();
    let received = Arc::new(AtomicUsize::new(0));
    let counter = received.clone();
    let health_calls = Arc::new(AtomicUsize::new(0));
    let health_counter = health_calls.clone();
    let idle = Arc::new(AtomicBool::new(true));
    let is_idle = idle.clone();
    let app = Router::new()
        .route(
            "/agent/v1/health",
            get(move || {
                let counter = health_counter.clone();
                let idle = is_idle.clone();
                async move {
                    counter.fetch_add(1, Ordering::SeqCst);
                    Json(AgentHealth {
                        agent_id: id,
                        version: "tls-fixture".into(),
                        protocol_min: 1,
                        protocol_max: 1,
                        stream_epoch: epoch,
                        revision: 1.into(),
                        healthy: true,
                        idle: idle.load(Ordering::SeqCst),
                    })
                }
            }),
        )
        .route(
            "/agent/v1/profile/validate",
            post(move |bytes: axum::body::Bytes| {
                let counter = counter.clone();
                async move {
                    counter.fetch_add(bytes.len(), Ordering::SeqCst);
                    Json(ProfileValidationResponse {
                        mail_state: ObservedMailState::Running,
                    })
                }
            }),
        )
        .merge(extra);
    let handle = axum_server::Handle::new();
    let task_handle = handle.clone();
    let task = tokio::spawn(async move {
        axum_server::from_tcp_rustls(
            listener,
            axum_server::tls_rustls::RustlsConfig::from_config(Arc::new(tls)),
        )
        .unwrap()
        .handle(task_handle)
        .serve(app.into_make_service())
        .await
        .unwrap();
    });
    TlsAgent {
        credentials: AgentCredentials {
            agent_id: id,
            center_id,
            endpoint: AgentEndpoint::try_from(format!("https://{address}")).unwrap(),
        },
        received,
        health_calls,
        idle,
        handle,
        task,
    }
}
pub struct NativeAgent {
    pub credentials: AgentCredentials,
    pub directory: tempfile::TempDir,
    pub data_directory: std::path::PathBuf,
    cancel: Arc<hmail_cleaner_core::cleanup::Cancellation>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for NativeAgent {
    fn drop(&mut self) {
        self.cancel.request();
        self.task.abort();
    }
}
pub async fn native(center: &CenterFixture) -> NativeAgent {
    let directory = tempfile::tempdir().unwrap();
    let data_directory = directory.path().join("hMailCleanerAgent");
    let agent_id = AgentId::new();
    let csr = hmail_cleaner_agent::enrollment::prepare(
        &hmail_cleaner_agent::enrollment::AgentBootstrap {
            data_directory: data_directory.clone(),
            agent_id,
        },
    )
    .unwrap();
    let issued = IdentityService::new(center.store.clone())
        .issue_agent(agent_id, "127.0.0.1", &csr.csr_pem, &center.lease)
        .await
        .unwrap();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let center_id = center
        .store
        .account()
        .await
        .unwrap()
        .unwrap()
        .material
        .vault_id();
    std::fs::write(data_directory.join("identity.pem"), issued.certificate_pem).unwrap();
    std::fs::write(data_directory.join("trust.pem"), issued.ca_pem).unwrap();
    let path = data_directory.join("agent.json");
    std::fs::write(
        &path,
        serde_json::to_vec(
            &serde_json::json!({"listen":address,"agent_id":agent_id,"center_id":center_id}),
        )
        .unwrap(),
    )
    .unwrap();
    let config =
        hmail_cleaner_agent::config::AgentConfig::load(&path, "native-renew-fixture").unwrap();
    let cancel = Arc::new(hmail_cleaner_core::cleanup::Cancellation::default());
    let task_cancel = cancel.clone();
    let task = tokio::spawn(async move {
        hmail_cleaner_agent::run_bound(config, task_cancel, listener)
            .await
            .unwrap();
    });
    let credentials = AgentCredentials {
        agent_id,
        center_id,
        endpoint: AgentEndpoint::try_from(format!("https://{address}")).unwrap(),
    };
    let client = hmail_cleaner_center::agents::AgentClient::new(
        &center.store,
        credentials.clone(),
        &center.lease,
    )
    .await
    .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        loop {
            if client.health().await.is_ok() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    NativeAgent {
        credentials,
        directory,
        data_directory,
        cancel,
        task,
    }
}
