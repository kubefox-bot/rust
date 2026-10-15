use super::wait;
use hmail_cleaner_core::{cleanup::PortResult, maintenance::mail_service::MailState};
use std::time::Duration;
use windows_service::{
    service::{Service, ServiceAccess, ServiceState},
    service_manager::{ServiceManager, ServiceManagerAccess},
};
pub(in crate::offline) struct Control {
    pub(super) service: Service,
}
impl Control {
    pub(in crate::offline) fn new(name: &str) -> PortResult<Self> {
        let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;
        let service = manager.open_service(
            name,
            ServiceAccess::QUERY_STATUS | ServiceAccess::STOP | ServiceAccess::START,
        )?;
        Ok(Self { service })
    }
    pub(in crate::offline) fn state(&self) -> PortResult<MailState> {
        Ok(match self.service.query_status()?.current_state {
            ServiceState::Running => MailState::Running,
            ServiceState::Stopped => MailState::Stopped,
            ServiceState::StartPending | ServiceState::StopPending => MailState::Transitioning,
            ServiceState::PausePending | ServiceState::ContinuePending | ServiceState::Paused => {
                MailState::Unsupported
            }
        })
    }
    pub(in crate::offline) fn stop(&self) -> PortResult<()> {
        if self.state()? != MailState::Running {
            return Err("mail service changed state before Stop".into());
        }
        self.service.stop()?;
        Ok(())
    }
    pub(in crate::offline) async fn wait_for_stopped(&self, timeout: Duration) -> PortResult<()> {
        wait::stopped(&self.service, timeout).await
    }
    pub(in crate::offline) async fn restore_running(&self, timeout: Duration) -> PortResult<()> {
        wait::running(&self.service, timeout).await
    }
}
