use crate::types::RunRequest;
use hmail_cleaner_core::cleanup::Progress;

/// Observes one run without owning its execution or loading configuration.
pub trait RunObserver: Send + Sync {
    fn begin(&self, _request: &RunRequest) {}
    fn progress(&self, _progress: Progress) {}
    fn server(&self, _version: &str, _cutoff: chrono::NaiveDateTime) {}
    fn action(&self, _message: &str) {}
    fn mail_state(&self, _state: &str) {}
    fn failed(&self) -> bool {
        false
    }
}
