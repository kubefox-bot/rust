#![cfg(windows)]
use std::{
    path::PathBuf,
    time::{Duration, Instant, SystemTime},
};
use windows_service::{
    service::{
        Service, ServiceAccess, ServiceErrorControl, ServiceExitCode, ServiceInfo,
        ServiceStartType, ServiceState, ServiceType,
    },
    service_manager::{ServiceManager, ServiceManagerAccess},
};
struct Owned {
    service: Service,
}
impl Drop for Owned {
    fn drop(&mut self) {
        let _ = self.service.stop();
        let _ = self.service.delete();
    }
}
fn create(mode: &str) -> Owned {
    // Explicit fixture path only: never a production CLI/service executable.
    let exe = PathBuf::from(
        std::env::var_os("HMAIL_SCM_FIXTURE_EXE")
            .expect("set HMAIL_SCM_FIXTURE_EXE to the built scm_fixture.exe"),
    );
    assert!(exe.is_absolute());
    assert_eq!(exe.file_name().unwrap(), "scm_fixture.exe");
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let name = format!("hMailCleanerFixture_{nanos}");
    let manager = ServiceManager::local_computer(
        None::<&str>,
        ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE,
    )
    .unwrap();
    let service = manager
        .create_service(
            &ServiceInfo {
                name: name.clone().into(),
                display_name: name.clone().into(),
                service_type: ServiceType::OWN_PROCESS,
                start_type: ServiceStartType::OnDemand,
                error_control: ServiceErrorControl::Normal,
                executable_path: exe,
                launch_arguments: vec![name.into(), mode.into()],
                dependencies: vec![],
                account_name: None,
                account_password: None,
            },
            ServiceAccess::QUERY_STATUS
                | ServiceAccess::START
                | ServiceAccess::STOP
                | ServiceAccess::DELETE,
        )
        .unwrap();
    Owned { service }
}
fn wait(service: &Service, state: ServiceState) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let status = service.query_status().unwrap();
        if status.current_state == state {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "SCM state: {:?}",
            status.current_state
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}
#[test]
#[ignore = "requires Windows administrator and explicit isolated SCM fixture executable"]
fn scm_starts_and_stops_owned_worker() {
    let owned = create("wait");
    owned.service.start::<&str>(&[]).unwrap();
    wait(&owned.service, ServiceState::Running);
    owned.service.stop().unwrap();
    wait(&owned.service, ServiceState::Stopped);
    assert_eq!(
        owned.service.query_status().unwrap().exit_code,
        ServiceExitCode::Win32(0)
    );
}
#[test]
#[ignore = "requires Windows administrator and explicit isolated SCM fixture executable"]
fn scm_reports_callback_error() {
    let owned = create("fail");
    let _ = owned.service.start::<&str>(&[]);
    wait(&owned.service, ServiceState::Stopped);
    assert_eq!(
        owned.service.query_status().unwrap().exit_code,
        ServiceExitCode::ServiceSpecific(1)
    );
}
