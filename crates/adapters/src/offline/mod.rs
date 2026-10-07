mod data_folder;
#[cfg(windows)]
mod windows;
pub use data_folder::verify_data_folder;

use hmail_cleaner_core::cleanup::{OfflineGuard, PortResult};

pub struct WindowsOffline {
    service_name: String,
}
impl WindowsOffline {
    pub fn new(service_name: String) -> Self {
        Self { service_name }
    }
}
impl OfflineGuard for WindowsOffline {
    async fn ensure_stopped(&self) -> PortResult<()> {
        #[cfg(windows)]
        {
            windows::ensure_stopped(&self.service_name)
        }
        #[cfg(not(windows))]
        {
            let _ = &self.service_name;
            Err("clean requires Windows and a STOPPED hMailServer service".into())
        }
    }
}
