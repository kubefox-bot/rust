use super::wait;
use hmail_cleaner_core::{cleanup::PortResult, maintenance::mail_service::MailState};
use std::time::Duration;
use windows_service::{
    service::{Service, ServiceAccess, ServiceState},
    service_manager::{ServiceManager, ServiceManagerAccess},
};
pub(in crate::offline) struct Control {
    name: String,
}
fn open(name: &str, access: ServiceAccess) -> PortResult<Service> {
    let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;
    Ok(manager.open_service(name, access)?)
}
impl Control {
    pub(in crate::offline) fn new(name: &str) -> PortResult<Self> {
        // Verify all rights before any Stop. Native !Send handles stay inside synchronous operations.
        drop(open(
            name,
            ServiceAccess::QUERY_STATUS | ServiceAccess::STOP | ServiceAccess::START,
        )?);
        Ok(Self {
            name: name.to_string(),
        })
    }
    pub(super) fn raw_state(&self) -> PortResult<ServiceState> {
        Ok(open(&self.name, ServiceAccess::QUERY_STATUS)?
            .query_status()?
            .current_state)
    }
    pub(super) fn start(&self) -> PortResult<()> {
        open(&self.name, ServiceAccess::START)?.start::<&str>(&[])?;
        Ok(())
    }
    pub(in crate::offline) fn state(&self) -> PortResult<MailState> {
        Ok(match self.raw_state()? {
            ServiceState::Running => MailState::Running,
            ServiceState::Stopped => MailState::Stopped,
            ServiceState::StartPending | ServiceState::StopPending => MailState::Transitioning,
            ServiceState::PausePending | ServiceState::ContinuePending | ServiceState::Paused => {
                MailState::Unsupported
            }
        })
    }
    pub(in crate::offline) fn stop(&self) -> PortResult<()> {
        let service = open(
            &self.name,
            ServiceAccess::QUERY_STATUS | ServiceAccess::STOP,
        )?;
        if service.query_status()?.current_state != ServiceState::Running {
            return Err("mail service changed state before Stop".into());
        }
        service.stop()?;
        Ok(())
    }
    pub(in crate::offline) async fn wait_for_stopped(&self, timeout: Duration) -> PortResult<()> {
        wait::stopped(self, timeout).await
    }
    pub(in crate::offline) async fn restore_running(&self, timeout: Duration) -> PortResult<()> {
        wait::running(self, timeout).await
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn control_is_send_and_sync() {
        fn shareable<T: Send + Sync>() {}
        shareable::<super::Control>();
    }
}
