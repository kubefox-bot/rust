use hmail_cleaner_core::{
    cleanup::PortResult,
    maintenance::mail_service::{MailControl, MailState},
};
use std::time::Duration;
pub struct WindowsMailControl {
    #[cfg(windows)]
    native: super::windows::Control,
}
impl WindowsMailControl {
    pub fn new(name: &str) -> PortResult<Self> {
        #[cfg(windows)]
        {
            Ok(Self {
                native: super::windows::Control::new(name)?,
            })
        }
        #[cfg(not(windows))]
        {
            let _ = name;
            Err("managed mail service control requires Windows".into())
        }
    }
}
impl MailControl for WindowsMailControl {
    async fn state(&self) -> PortResult<MailState> {
        #[cfg(windows)]
        {
            self.native.state()
        }
        #[cfg(not(windows))]
        {
            Ok(MailState::Unsupported)
        }
    }
    async fn stop(&self) -> PortResult<()> {
        #[cfg(windows)]
        {
            self.native.stop()
        }
        #[cfg(not(windows))]
        {
            Err("managed mail service control requires Windows".into())
        }
    }
    async fn wait_for_stopped(&self, timeout: Duration) -> PortResult<()> {
        #[cfg(windows)]
        {
            self.native.wait_for_stopped(timeout).await
        }
        #[cfg(not(windows))]
        {
            let _ = timeout;
            Err("managed mail service control requires Windows".into())
        }
    }
    async fn restore_running(&self, timeout: Duration) -> PortResult<()> {
        #[cfg(windows)]
        {
            self.native.restore_running(timeout).await
        }
        #[cfg(not(windows))]
        {
            let _ = timeout;
            Err("managed mail service control requires Windows".into())
        }
    }
}
