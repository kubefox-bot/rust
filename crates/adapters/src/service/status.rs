use hmail_cleaner_core::cleanup::PortResult;
use std::time::Duration;
use windows_service::{
    service::{ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus, ServiceType},
    service_control_handler::ServiceStatusHandle,
};
pub(super) fn set(
    handle: &ServiceStatusHandle,
    state: ServiceState,
    checkpoint: u32,
    failed: bool,
) -> PortResult<()> {
    let controls_accepted = if state == ServiceState::Running {
        ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN
    } else {
        ServiceControlAccept::empty()
    };
    handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: state,
        controls_accepted,
        exit_code: if failed {
            ServiceExitCode::ServiceSpecific(1)
        } else {
            ServiceExitCode::Win32(0)
        },
        checkpoint,
        wait_hint: if checkpoint > 0 {
            Duration::from_secs(10)
        } else {
            Duration::ZERO
        },
        process_id: None,
    })?;
    Ok(())
}
