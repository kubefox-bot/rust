use crate::{MailObservation, RunObserver};
use hmail_cleaner_core::{
    cleanup::PortResult,
    maintenance::mail_service::{MailControl, MailState},
};
use std::time::Duration;

pub(super) struct ObservedControl<'a, C, O> {
    pub control: &'a C,
    pub observer: &'a O,
}
impl<C: MailControl + Sync, O: RunObserver> MailControl for ObservedControl<'_, C, O> {
    async fn state(&self) -> PortResult<MailState> {
        let state = self.control.state().await?;
        self.observer.service_state(MailObservation::State(state));
        Ok(state)
    }
    async fn stop(&self) -> PortResult<()> {
        self.control.stop().await
    }
    async fn wait_for_stopped(&self, timeout: Duration) -> PortResult<()> {
        self.control.wait_for_stopped(timeout).await?;
        self.observer
            .service_state(MailObservation::State(MailState::Stopped));
        Ok(())
    }
    async fn restore_running(&self, timeout: Duration) -> PortResult<()> {
        self.observer.service_state(MailObservation::Restoring);
        self.control.restore_running(timeout).await?;
        self.observer
            .service_state(MailObservation::State(MailState::Running));
        Ok(())
    }
}
