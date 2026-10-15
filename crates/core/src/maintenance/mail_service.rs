use crate::cleanup::{Cancellation, PortResult};
use std::{error::Error, future::Future, time::Duration};
const STOP_TIMEOUT: Duration = Duration::from_secs(60);
const RESTORE_TIMEOUT: Duration = Duration::from_secs(60);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MailState {
    Running,
    Stopped,
    Transitioning,
    Unsupported,
}
pub trait MailControl {
    fn state(&self) -> impl Future<Output = PortResult<MailState>> + Send;
    fn stop(&self) -> impl Future<Output = PortResult<()>> + Send;
    fn wait_for_stopped(&self, timeout: Duration) -> impl Future<Output = PortResult<()>> + Send;
    fn restore_running(&self, timeout: Duration) -> impl Future<Output = PortResult<()>> + Send;
}
pub struct ManagedResult<T> {
    pub result: PortResult<T>,
    pub restore_error: Option<Box<dyn Error + Send + Sync>>,
}
pub async fn with_stopped<
    C: MailControl,
    F: FnOnce() -> Fut,
    Fut: Future<Output = PortResult<T>>,
    T,
>(
    control: &C,
    cancellation: &Cancellation,
    work: F,
) -> ManagedResult<T> {
    if cancellation.is_requested() {
        return ManagedResult {
            result: Err("cancelled before mail service stop".into()),
            restore_error: None,
        };
    }
    let state = match control.state().await {
        Ok(state) => state,
        Err(error) => {
            return ManagedResult {
                result: Err(error),
                restore_error: None,
            }
        }
    };
    match state {
        MailState::Stopped => ManagedResult {
            result: work().await,
            restore_error: None,
        },
        MailState::Running => {
            if let Err(error) = control.stop().await {
                return ManagedResult {
                    result: Err(error),
                    restore_error: None,
                };
            }
            let result = match control.wait_for_stopped(STOP_TIMEOUT).await {
                Err(error) => Err(error),
                Ok(()) if cancellation.is_requested() => {
                    Err("cancelled after mail service stop".into())
                }
                Ok(()) => work().await,
            };
            let restore_error = control.restore_running(RESTORE_TIMEOUT).await.err();
            ManagedResult {
                result,
                restore_error,
            }
        }
        MailState::Transitioning | MailState::Unsupported => ManagedResult {
            result: Err("mail service is transitioning or unsupported; no Stop sent".into()),
            restore_error: None,
        },
    }
}
