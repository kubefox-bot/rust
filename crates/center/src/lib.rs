//! Control center. Mail execution belongs to separately authenticated agents.
pub mod account;
pub mod app;
pub mod auth;
#[cfg(feature = "cli")]
pub mod cli;
pub mod config;
pub mod http;
pub mod identity;
pub mod storage;
pub mod vault;
pub use app::{run, CenterError};
pub mod agents;
pub mod history;
pub mod jobs;
pub mod profiles;
