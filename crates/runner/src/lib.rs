//! Shared one-shot execution for the CLI and native mail agent.
pub mod lifecycle;
mod observer;
mod report;
mod run;
mod types;

pub use lifecycle::RunResult;
pub use observer::RunObserver;
pub use report::{Report, Status};
pub use run::run_once;
pub use types::{Mode, RunRequest, ServicePolicy};
