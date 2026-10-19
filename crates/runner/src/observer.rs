use crate::types::RunRequest;
use hmail_cleaner_core::cleanup::Progress;
use hmail_cleaner_core::maintenance::mail_service::MailState;

#[derive(Debug, Clone, Copy)]
pub enum MailObservation {
    State(MailState),
    Restoring,
}

/// Observes one run without owning its execution or loading configuration.
pub trait RunObserver: Send + Sync {
    fn selected_files(
        &self,
        _files: &[hmail_cleaner_core::maintenance::recursive::SelectedFile],
    ) -> impl std::future::Future<Output = hmail_cleaner_core::cleanup::PortResult<()>> + Send {
        async { Ok(()) }
    }
    fn begin(&self, _request: &RunRequest) {}
    fn progress(&self, _progress: Progress) {}
    fn server(&self, _version: &str, _cutoff: chrono::NaiveDateTime) {}
    fn action(&self, _message: &str) {}
    fn mail_state(&self, _state: &str) {}
    fn service_state(&self, _state: MailObservation) {}
    fn failed(&self) -> bool {
        false
    }
}
