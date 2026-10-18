#![cfg(unix)]
pub mod command;
pub mod config;
pub mod delivery;
pub mod journal;
pub mod policy;
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid updater configuration or input")]
    Invalid,
    #[error("updater storage unavailable")]
    Storage,
    #[error("updater transport unavailable")]
    Transport,
    #[error("release verification failed")]
    Verification,
    #[error("host command failed")]
    Command,
    #[error("update requires attention")]
    Attention,
    #[error("operation conflict")]
    Conflict,
}

pub mod control;

pub mod discovery;
pub mod network;
pub mod verification;

pub mod podman;

pub mod backup;

pub mod apply;

pub mod disk;
pub mod run;

pub mod reboot;

pub mod retention;

pub mod staging;
