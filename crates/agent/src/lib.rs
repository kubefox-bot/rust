//! Native mail agent. Accepted work is recorded before execution starts.
mod app;
#[cfg(feature = "cli")]
pub mod cli;
pub mod config;
pub mod cutoff;
pub mod enrollment;
pub mod execution;
pub mod http;
pub mod jobs;
mod runtime_lock;
pub mod service;
pub use app::run;
pub use execution::AgentError;
