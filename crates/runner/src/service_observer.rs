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
        self.observer
            .service_state(MailObservation::State(MailState::Transitioning));
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
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    struct FailingStop;
    impl MailControl for FailingStop {
        async fn state(&self) -> PortResult<MailState> {
            Ok(MailState::Running)
        }
        async fn stop(&self) -> PortResult<()> {
            Err("ambiguous stop failure".into())
        }
        async fn wait_for_stopped(&self, _: Duration) -> PortResult<()> {
            Ok(())
        }
        async fn restore_running(&self, _: Duration) -> PortResult<()> {
            Ok(())
        }
    }
    struct Observer(Mutex<Vec<MailObservation>>);
    impl RunObserver for Observer {
        fn service_state(&self, state: MailObservation) {
            self.0.lock().unwrap().push(state);
        }
    }
    #[tokio::test]
    async fn failed_stop_does_not_retain_the_previous_running_observation() {
        let observer = Observer(Mutex::new(Vec::new()));
        let control = ObservedControl {
            control: &FailingStop,
            observer: &observer,
        };
        assert_eq!(control.state().await.unwrap(), MailState::Running);
        assert!(control.stop().await.is_err());
        assert!(matches!(
            observer.0.lock().unwrap().last(),
            Some(MailObservation::State(MailState::Transitioning))
        ));
    }
}
