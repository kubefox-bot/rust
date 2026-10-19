//! Authenticated update timing policy, readable by the host without opening the vault.
mod http;
mod persistence;

pub use http::routes;
pub use persistence::{read_policy, PolicyError};
