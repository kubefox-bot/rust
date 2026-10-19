use hmail_cleaner_agent::service::{
    install_with, InstallError, InstallServiceInput, Presence, ServiceRegistration,
    ServiceRegistry, ServiceState,
};
use std::{path::PathBuf, sync::Mutex};

struct Registry {
    agent: Presence,
    legacy: Presence,
    trace: Mutex<Vec<&'static str>>,
}
impl ServiceRegistry for Registry {
    fn inspect(&self, registration: &ServiceRegistration) -> Result<Presence, InstallError> {
        Ok(if registration.name == "hMailCleanerAgent" {
            self.agent
        } else {
            self.legacy
        })
    }
    fn stop_and_wait(&self, registration: &ServiceRegistration) -> Result<(), InstallError> {
        self.trace
            .lock()
            .unwrap()
            .push(if registration.name == "hMailCleanerAgent" {
                "agent stopped"
            } else {
                "legacy stopped"
            });
        Ok(())
    }
    fn disable(&self, _: &ServiceRegistration) -> Result<(), InstallError> {
        self.trace.lock().unwrap().push("legacy disabled");
        Ok(())
    }
    fn configure_and_start(&self, registration: &ServiceRegistration) -> Result<(), InstallError> {
        let config = PathBuf::from(registration.arguments.last().unwrap());
        let receipt: serde_json::Value = serde_json::from_slice(
            &std::fs::read(config.parent().unwrap().join("installation.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(
            receipt["started"], false,
            "metadata must not claim SCM start before it happens"
        );
        self.trace.lock().unwrap().push("agent enabled");
        Ok(())
    }
}
fn registry(agent: Presence, legacy: Presence) -> Registry {
    Registry {
        agent,
        legacy,
        trace: Mutex::new(Vec::new()),
    }
}
fn input(root: &std::path::Path) -> InstallServiceInput {
    let source = root.join("source.exe");
    std::fs::write(&source, b"fixture agent package").unwrap();
    let data_directory = root.join("state/hMailCleanerAgent");
    std::fs::create_dir_all(&data_directory).unwrap();
    let config = data_directory.join("agent.json");
    std::fs::write(&config, b"fixture config").unwrap();
    use sha2::Digest;
    InstallServiceInput {
        operation_id: Some(uuid::Uuid::new_v4()),
        package: source,
        executable: root.join("program/hMailCleanerAgent/hmail-cleaner-agent.exe"),
        config,
        data_directory,
        expected_sha256: sha2::Sha256::digest(b"fixture agent package").into(),
        legacy: Some(ServiceRegistration {
            name: "hMailCleaner".into(),
            executable: PathBuf::from("/trusted/old-cleaner.exe"),
            arguments: vec![
                "service".into(),
                "run".into(),
                "--env-file".into(),
                "/trusted/cleaner.env".into(),
            ],
        }),
    }
}
#[tokio::test]
async fn trust_recovery_refuses_an_active_receipt_and_never_creates_a_missing_ledger() {
    let root = tempfile::tempdir().unwrap();
    let request = input(root.path());
    assert!(
        hmail_cleaner_agent::service::prepare_recovery(&request.data_directory)
            .await
            .is_err()
    );
    assert!(!request.data_directory.join("jobs.sqlite").exists());
    let repo =
        hmail_cleaner_agent::jobs::JobRepository::open(&request.data_directory.join("jobs.sqlite"))
            .await
            .unwrap();
    let job = hmail_cleaner_protocol::validate_job(
        serde_json::from_str(include_str!("../../protocol/tests/fixtures/job-v1.json")).unwrap(),
    )
    .unwrap();
    repo.accept(&job, repo.fingerprint(&job)).await.unwrap();
    assert!(matches!(
        hmail_cleaner_agent::service::prepare_recovery(&request.data_directory).await,
        Err(hmail_cleaner_agent::service::InstallError::Busy)
    ));
    assert!(!repo.in_maintenance().await.unwrap());
    repo.cancel(job.command().job_id).await.unwrap();
    hmail_cleaner_agent::service::prepare_recovery(&request.data_directory)
        .await
        .unwrap();
    assert!(repo.in_maintenance().await.unwrap());
    repo.close().await;
}
#[tokio::test]
async fn unrelated_service_collision_has_no_modifications() {
    let dir = tempfile::tempdir().unwrap();
    let input = input(dir.path());
    let registry = registry(Presence::Collision, Presence::Absent);
    assert!(matches!(
        install_with(input, &registry).await,
        Err(InstallError::Collision)
    ));
    assert!(registry.trace.lock().unwrap().is_empty());
    assert!(!dir
        .path()
        .join("program/hMailCleanerAgent/hmail-cleaner-agent.exe")
        .exists());
}
#[tokio::test]
async fn legacy_is_stopped_and_disabled_before_agent_activation() {
    let dir = tempfile::tempdir().unwrap();
    let registry = registry(Presence::Absent, Presence::Owned(ServiceState::Running));
    let receipt = install_with(input(dir.path()), &registry).await.unwrap();
    assert!(receipt.started);
    assert_eq!(
        *registry.trace.lock().unwrap(),
        ["legacy stopped", "legacy disabled", "agent enabled"]
    );
    assert!(dir
        .path()
        .join("program/hMailCleanerAgent/hmail-cleaner-agent.exe")
        .is_file());
}
#[tokio::test]
async fn active_or_restoring_receipt_blocks_binary_replacement() {
    use hmail_cleaner_agent::jobs::{JobRepository, JobUpdate};
    use hmail_cleaner_protocol::{
        validate_job, JobCommand, JobPhase, ObservedMailState, SummaryDto,
    };
    for phase in [JobPhase::Accepted, JobPhase::Running, JobPhase::Restoring] {
        let dir = tempfile::tempdir().unwrap();
        let input = input(dir.path());
        std::fs::create_dir_all(&input.data_directory).unwrap();
        let repo = JobRepository::open(&input.data_directory.join("jobs.sqlite"))
            .await
            .unwrap();
        let command: JobCommand =
            serde_json::from_str(include_str!("../../protocol/tests/fixtures/job-v1.json"))
                .unwrap();
        let job = validate_job(command).unwrap();
        let id = job.command().job_id;
        repo.accept(&job, repo.fingerprint(&job)).await.unwrap();
        if phase != JobPhase::Accepted {
            repo.transition(
                id,
                JobUpdate {
                    phase: JobPhase::Running,
                    stage: None,
                    summary: SummaryDto::default(),
                    mail_state: ObservedMailState::Unknown,
                    error: None,
                },
            )
            .await
            .unwrap();
        }
        if phase == JobPhase::Restoring {
            repo.transition(
                id,
                JobUpdate {
                    phase,
                    stage: None,
                    summary: SummaryDto::default(),
                    mail_state: ObservedMailState::Restoring,
                    error: None,
                },
            )
            .await
            .unwrap();
        }
        let registry = registry(Presence::Owned(ServiceState::Running), Presence::Absent);
        assert!(matches!(
            install_with(input, &registry).await,
            Err(InstallError::Busy)
        ));
        assert!(registry.trace.lock().unwrap().is_empty());
        assert!(!dir
            .path()
            .join("program/hMailCleanerAgent/hmail-cleaner-agent.exe")
            .exists());
        repo.close().await;
    }
}

#[cfg(windows)]
mod native {
    use std::{
        path::PathBuf,
        time::{Duration, Instant},
    };
    use windows_service::{
        service::{
            Service, ServiceAccess, ServiceErrorControl, ServiceInfo, ServiceStartType,
            ServiceState, ServiceType,
        },
        service_manager::{ServiceManager, ServiceManagerAccess},
    };
    struct Owned(Service);
    impl Drop for Owned {
        fn drop(&mut self) {
            let _ = self.0.stop();
            let _ = self.0.delete();
        }
    }
    fn wait(predicate: impl Fn() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(30);
        while !predicate() {
            assert!(Instant::now() < deadline, "SCM fixture deadline");
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    #[test]
    #[ignore = "requires Windows administrator and HMAIL_AGENT_SCM_FIXTURE_EXE; run separately on Server 2016"]
    fn service_waits_for_manual_command_and_stop_waits_for_restore() {
        // This fixture is the sole owner of environment access in the native test.
        let executable = PathBuf::from(
            std::env::var_os("HMAIL_AGENT_SCM_FIXTURE_EXE")
                .expect("set the isolated service_fixture.exe path"),
        );
        assert!(executable.is_absolute());
        assert_eq!(executable.file_name().unwrap(), "service_fixture.exe");
        let directory = tempfile::tempdir().unwrap();
        let name = format!("hMailCleanerAgentFixture_{}", uuid::Uuid::new_v4().simple());
        let manager = ServiceManager::local_computer(
            None::<&str>,
            ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE,
        )
        .unwrap();
        let owned = Owned(
            manager
                .create_service(
                    &ServiceInfo {
                        name: name.clone().into(),
                        display_name: name.clone().into(),
                        service_type: ServiceType::OWN_PROCESS,
                        start_type: ServiceStartType::OnDemand,
                        error_control: ServiceErrorControl::Normal,
                        executable_path: executable,
                        launch_arguments: vec![name.into(), directory.path().as_os_str().into()],
                        dependencies: vec![],
                        account_name: None,
                        account_password: None,
                    },
                    ServiceAccess::QUERY_STATUS
                        | ServiceAccess::START
                        | ServiceAccess::STOP
                        | ServiceAccess::DELETE,
                )
                .unwrap(),
        );
        owned.0.start::<&str>(&[]).unwrap();
        let trace = directory.path().join("trace.log");
        let read = || std::fs::read_to_string(&trace).unwrap_or_default();
        wait(|| read().contains("service idle"));
        assert!(!read().contains("runner started"));
        std::fs::write(directory.path().join("manual.command"), b"run fixture").unwrap();
        wait(|| read().contains("runner started"));
        owned.0.stop().unwrap();
        wait(|| read().contains("restoring"));
        assert_ne!(
            owned.0.query_status().unwrap().current_state,
            ServiceState::Stopped
        );
        wait(|| owned.0.query_status().unwrap().current_state == ServiceState::Stopped);
        let events = read();
        assert!(events.find("restored").unwrap() < events.find("database closed").unwrap());
    }
}

#[tokio::test]
async fn repeated_updates_keep_the_immediately_previous_executable_and_verify_it() {
    use sha2::Digest;
    let dir = tempfile::tempdir().unwrap();
    install_with(
        input(dir.path()),
        &registry(Presence::Absent, Presence::Absent),
    )
    .await
    .unwrap();
    for (new, previous) in [
        (
            b"second agent".as_slice(),
            b"fixture agent package".as_slice(),
        ),
        (b"third agent".as_slice(), b"second agent".as_slice()),
    ] {
        let mut request = input(dir.path());
        std::fs::write(&request.package, new).unwrap();
        request.expected_sha256 = sha2::Sha256::digest(new).into();
        let target = request.executable.clone();
        let receipt = install_with(
            request,
            &registry(Presence::Owned(ServiceState::Running), Presence::Absent),
        )
        .await
        .unwrap();
        assert!(receipt.started);
        assert_eq!(std::fs::read(&target).unwrap(), new);
        assert_eq!(
            std::fs::read(target.with_extension("exe.previous")).unwrap(),
            previous
        );
    }
    let request = input(dir.path());
    let target = request.executable.clone();
    std::fs::write(
        target.with_extension("exe.previous"),
        b"unrecognized backup",
    )
    .unwrap();
    let registry = registry(Presence::Owned(ServiceState::Running), Presence::Absent);
    assert!(matches!(
        install_with(request, &registry).await,
        Err(InstallError::Invalid)
    ));
    assert!(registry.trace.lock().unwrap().is_empty());
    assert_eq!(std::fs::read(target).unwrap(), b"third agent");
}
