//! Native mail agent. Accepted work is recorded before execution starts.
mod app;
pub mod config;
pub mod cutoff;
pub mod enrollment;
pub mod execution;
pub mod http;
pub mod jobs;
pub use app::run;
pub use execution::AgentError;
