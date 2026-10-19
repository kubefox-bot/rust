use hmail_cleaner_center::{
    auth::{SessionAuthority, SessionLease},
    enrollment::{
        remote::{Bootstrap, BootstrapIdentity, Connector, Remote},
        AgentPackage, EnrollmentError, PackageCatalog, SshCredentials, SshTarget,
    },
};
use hmail_cleaner_protocol::{
    browser::enrollment::{EnrollmentPhase, EnrollmentView, InstallRequest, LegacyService},
    AgentCsr, AgentId,
};
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc, Mutex,
};
pub const BUILD: &str = "fixture-agent-v1";
pub struct Packages {
    pub root: tempfile::TempDir,
    pub catalog: PackageCatalog,
}
pub fn packages() -> Packages {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join(BUILD);
    std::fs::create_dir(&path).unwrap();
    let mut bytes = vec![0u8; 100_000];
    bytes[..2].copy_from_slice(b"MZ");
    std::fs::write(path.join("hmail-cleaner-agent.exe"), &bytes).unwrap();
    let hash = ring::digest::digest(&ring::digest::SHA256, &bytes)
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    std::fs::write(path.join("manifest.json"),serde_json::to_vec(&serde_json::json!({"product":"hmail-cleaner-agent","build":BUILD,"target":"x86_64-pc-windows-msvc","protocol_min":1,"protocol_max":1,"sha256":hash})).unwrap()).unwrap();
    let catalog = PackageCatalog::new(root.path().to_owned());
    Packages { root, catalog }
}
#[derive(Clone)]
pub struct FakeConnector(pub Arc<Node>);
pub struct Node {
    pub root: tempfile::TempDir,
    pub data: std::path::PathBuf,
    pub id: AgentId,
    pub endpoint: String,
    csr: AgentCsr,
    listener: Mutex<Option<std::net::TcpListener>>,
    task: Mutex<Option<tokio::task::JoinHandle<()>>>,
    cancel: Mutex<Arc<hmail_cleaner_core::cleanup::Cancellation>>,
    needs_restart: AtomicBool,
    pub connects: AtomicUsize,
    pub uploads: AtomicUsize,
    pub installs: AtomicUsize,
    pub closes: AtomicUsize,
    pub fail_upload: AtomicBool,
    pub lost_install_response: AtomicBool,
    pub revoke_on_prepare: Mutex<Option<(Arc<SessionAuthority>, u64)>>,
    pub revoke_on_csr: Mutex<Option<(hmail_cleaner_center::storage::CenterStore, SessionLease)>>,
}
impl Drop for Node {
    fn drop(&mut self) {
        self.cancel.lock().unwrap().request();
        if let Some(task) = self.task.lock().unwrap().take() {
            task.abort();
        }
    }
}
pub fn connector() -> FakeConnector {
    let root = tempfile::tempdir().unwrap();
    let data = root.path().join("hMailCleanerAgent");
    let id = AgentId::new();
    let csr = hmail_cleaner_agent::enrollment::prepare(
        &hmail_cleaner_agent::enrollment::AgentBootstrap {
            data_directory: data.clone(),
            agent_id: id,
        },
    )
    .unwrap();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("https://{}", listener.local_addr().unwrap());
    FakeConnector(Arc::new(Node {
        root,
        data,
        id,
        endpoint,
        csr,
        listener: Mutex::new(Some(listener)),
        task: Mutex::new(None),
        cancel: Mutex::new(Arc::new(Default::default())),
        needs_restart: AtomicBool::new(false),
        connects: AtomicUsize::new(0),
        uploads: AtomicUsize::new(0),
        installs: AtomicUsize::new(0),
        closes: AtomicUsize::new(0),
        fail_upload: AtomicBool::new(false),
        lost_install_response: AtomicBool::new(false),
        revoke_on_prepare: Mutex::new(None),
        revoke_on_csr: Mutex::new(None),
    }))
}
impl Connector for FakeConnector {
    type Connection = Self;
    async fn connect(
        &self,
        _: SshTarget,
        _: String,
        credentials: SshCredentials,
        lease: &SessionLease,
    ) -> Result<Self, EnrollmentError> {
        if !lease.is_live() {
            return Err(EnrollmentError::Unauthorized);
        }
        self.0.connects.fetch_add(1, Ordering::SeqCst);
        drop(credentials);
        Ok(self.clone())
    }
}
impl FakeConnector {
    pub async fn stop(&self) {
        self.0.cancel.lock().unwrap().request();
        let task = self.0.task.lock().unwrap().take();
        if let Some(task) = task {
            task.await.unwrap();
        }
    }
    pub async fn restart(&self) {
        let existing = self.0.listener.lock().unwrap().take();
        let listener = existing.unwrap_or_else(|| {
            std::net::TcpListener::bind(self.0.endpoint.strip_prefix("https://").unwrap()).unwrap()
        });
        let config =
            hmail_cleaner_agent::config::AgentConfig::load(&self.0.data.join("agent.json"), BUILD)
                .unwrap();
        let stop = Arc::new(hmail_cleaner_core::cleanup::Cancellation::default());
        *self.0.cancel.lock().unwrap() = stop.clone();
        let task = tokio::spawn(async move {
            hmail_cleaner_agent::run_bound(config, stop, listener)
                .await
                .unwrap();
        });
        *self.0.task.lock().unwrap() = Some(task);
    }
}
impl Remote for FakeConnector {
    async fn prepare_recovery(&self, _: &Bootstrap) -> Result<(), EnrollmentError> {
        hmail_cleaner_agent::service::prepare_recovery(&self.0.data)
            .await
            .map_err(|_| EnrollmentError::Remote)
    }
    async fn recover_identity(&self, input: &BootstrapIdentity<'_>) -> Result<(), EnrollmentError> {
        std::fs::write(self.0.data.join("identity.pem"), input.certificate_pem).unwrap();
        std::fs::write(self.0.data.join("trust.pem"), input.ca_pem).unwrap();
        self.0.needs_restart.store(true, Ordering::SeqCst);
        Ok(())
    }
    async fn cleanup(&self, _: &Bootstrap) -> Result<(), EnrollmentError> {
        Ok(())
    }
    async fn preflight(&self) -> Result<(), EnrollmentError> {
        Ok(())
    }
    async fn prepare(&self, _: &Bootstrap) -> Result<String, EnrollmentError> {
        if let Some((authority, generation)) = self.0.revoke_on_prepare.lock().unwrap().take() {
            authority.revoke(generation);
        }
        Ok("C:/ProgramData/hMailCleanerAgentStage/fixture.exe".into())
    }
    async fn upload(&self, _: &str, _: &mut AgentPackage) -> Result<(), EnrollmentError> {
        self.0.uploads.fetch_add(1, Ordering::SeqCst);
        if self.0.fail_upload.load(Ordering::SeqCst) {
            Err(EnrollmentError::Transport)
        } else {
            Ok(())
        }
    }
    async fn enroll(&self, input: &Bootstrap) -> Result<AgentCsr, EnrollmentError> {
        assert_eq!(input.agent_id, self.0.id);
        let revoke = self.0.revoke_on_csr.lock().unwrap().take();
        if let Some((store, lease)) = revoke {
            hmail_cleaner_center::identity::IdentityService::new(store)
                .revoke_agent(input.agent_id, &lease)
                .await
                .unwrap();
        }
        Ok(AgentCsr {
            agent_id: self.0.id,
            csr_pem: self.0.csr.csr_pem.clone(),
        })
    }
    async fn configure(&self, input: &BootstrapIdentity<'_>) -> Result<(), EnrollmentError> {
        for (name,bytes) in [("identity.pem",input.certificate_pem.as_bytes().to_vec()),("trust.pem",input.ca_pem.as_bytes().to_vec()),("agent.json",serde_json::to_vec(&serde_json::json!({"listen":format!("127.0.0.1:{}",input.listen_port),"agent_id":input.bootstrap.agent_id,"center_id":input.bootstrap.center_id})).unwrap())]{std::fs::write(self.0.data.join(name),bytes).unwrap();}
        Ok(())
    }
    async fn install(
        &self,
        input: &Bootstrap,
        _: Option<&LegacyService>,
    ) -> Result<(), EnrollmentError> {
        self.0.installs.fetch_add(1, Ordering::SeqCst);
        std::fs::write(
            self.0.data.join("installation.json"),
            serde_json::to_vec(&hmail_cleaner_agent::service::InstallationReceipt {
                operation_id: Some(input.operation_id),
                executable: std::env::current_exe().unwrap(),
                sha256: input.sha256.clone(),
                previous_sha256: None,
                started: true,
            })
            .unwrap(),
        )
        .unwrap();
        if self.0.needs_restart.swap(false, Ordering::SeqCst) {
            self.stop().await;
        }
        let repository =
            hmail_cleaner_agent::jobs::JobRepository::open(&self.0.data.join("jobs.sqlite"))
                .await
                .unwrap();
        repository.end_maintenance().await.unwrap();
        repository.close().await;
        let stopped = self.0.task.lock().unwrap().is_none();
        if stopped {
            self.restart().await;
        }
        if self.0.lost_install_response.load(Ordering::SeqCst) {
            Err(EnrollmentError::Transport)
        } else {
            Ok(())
        }
    }
    async fn close(self) -> Result<(), EnrollmentError> {
        self.0.closes.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}
pub fn request(node: &FakeConnector) -> InstallRequest {
    InstallRequest {
        operation_id: uuid::Uuid::new_v4(),
        agent_id: node.0.id,
        server_id: hmail_cleaner_protocol::ServerId::new(),
        profile_id: hmail_cleaner_protocol::ProfileId::new(),
        name: "Почтовый сервер".into(),
        endpoint: node.0.endpoint.clone(),
        target: SshTarget {
            host: "127.0.0.1".into(),
            port: 22,
        },
        fingerprint: "SHA256:".to_owned() + &"A".repeat(43),
        credentials: SshCredentials::Password {
            username: "Админ; &literal".into(),
            password: zeroize::Zeroizing::new("OS-PASSWORD-NEVER-PERSIST".into()),
        },
        package_version: BUILD.into(),
        legacy: None,
    }
}
pub async fn settled<C: Connector>(
    service: &hmail_cleaner_center::enrollment::EnrollmentService<C>,
    id: uuid::Uuid,
    lease: &SessionLease,
) -> EnrollmentView {
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            let view = service.get(id, lease).await.unwrap();
            if matches!(
                view.phase,
                EnrollmentPhase::Ready | EnrollmentPhase::Failed | EnrollmentPhase::Unknown
            ) {
                break view;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap()
}
