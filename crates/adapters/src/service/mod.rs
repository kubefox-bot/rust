use hmail_cleaner_core::cleanup::{Cancellation, PortResult};
use std::sync::Arc;
#[cfg(windows)]
mod handler;
#[cfg(windows)]
mod status;
#[cfg(windows)]
mod windows;
pub type ServiceJob = Box<dyn Fn(Arc<Cancellation>) -> PortResult<()> + Send + Sync>;
pub fn host(name: &str, job: ServiceJob) -> PortResult<()> {
    #[cfg(windows)]
    {
        windows::host(name, job)
    }
    #[cfg(not(windows))]
    {
        let _ = (name, job);
        Err("Windows service hosting requires Windows".into())
    }
}
