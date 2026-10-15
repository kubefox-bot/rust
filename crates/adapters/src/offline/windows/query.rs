use hmail_cleaner_core::cleanup::PortResult;
use windows_service::{
    service::{ServiceAccess, ServiceState},
    service_manager::{ServiceManager, ServiceManagerAccess},
};

pub(in crate::offline) fn ensure_stopped(name: &str) -> PortResult<()> {
    let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;
    let service = manager.open_service(name, ServiceAccess::QUERY_STATUS)?;
    let status = service.query_status()?;
    if status.current_state != ServiceState::Stopped {
        return Err("hMailServer must be STOPPED before cleaning".into());
    }
    Ok(())
}
