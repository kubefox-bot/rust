//! Shared one-shot execution for the CLI and native mail agent.
pub mod lifecycle;
mod observer;
mod report;
mod run;
mod service_observer;
mod types;

pub use lifecycle::RunResult;
pub use observer::{MailObservation, RunObserver};
pub use report::{Report, Status};
pub use run::run_once;
pub use types::{Mode, RunRequest, ServicePolicy};
