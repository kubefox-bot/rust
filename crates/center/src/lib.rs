//! Control center. Mail execution belongs to separately authenticated agents.
pub mod account;
#[cfg(feature = "cli")]
pub mod cli;
pub mod config;
pub mod storage;
pub mod vault;
