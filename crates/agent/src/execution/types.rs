use crate::jobs::JobStoreError;
use hmail_cleaner_core::cleanup::Cancellation;
use hmail_cleaner_protocol::ValidatedJob;
use hmail_cleaner_runner::{RunObserver, RunResult};
use std::future::Future;

pub trait Executor: Send + Sync + 'static {
    fn execute(
        &self,
        job: &ValidatedJob,
        observer: &impl RunObserver,
        cancel: &Cancellation,
    ) -> impl Future<Output = RunResult> + Send;
}
#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("job storage failed")]
    Storage(#[from] JobStoreError),
    #[error("invalid request")]
    Invalid,
    #[error("job conflicts with an existing receipt")]
    Conflict,
    #[error("another job is active")]
    Busy,
    #[error("agent requires attention before accepting work")]
    Unavailable,
    #[error("agent transport failed")]
    Transport(#[from] std::io::Error),
}
