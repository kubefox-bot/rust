//! Local FleetLock coordinator: OS reboots share application-update admission.
mod coordinator;
mod grant;
mod http;
mod window;

use crate::{config::Config, journal, Error};
use chrono::{DateTime, Utc};
pub use coordinator::{ClientIdentity, Clock, Coordinator};
pub use http::router;
use std::sync::Arc;
use uuid::Uuid;

pub const CLIENT_FILENAME: &str = "reboot-client.json";
pub const GRANT_FILENAME: &str = "reboot-grant.json";
pub const LISTEN_ADDRESS: &str = "127.0.0.1:35355";
struct SystemClock;
impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}
pub async fn serve(config: &Config) -> Result<(), Error> {
    let client = journal::load::<ClientIdentity>(&config.state_dir.join(CLIENT_FILENAME))?
        .ok_or(Error::Invalid)?;
    let boot_id: Uuid = std::fs::read_to_string("/proc/sys/kernel/random/boot_id")
        .map_err(|_| Error::Storage)?
        .trim()
        .parse()
        .map_err(|_| Error::Invalid)?;
    let coordinator = Arc::new(Coordinator::new(
        config.state_dir.clone(),
        config.socket_path.clone(),
        client,
        boot_id,
        Arc::new(SystemClock),
    )?);
    let listener = tokio::net::TcpListener::bind(LISTEN_ADDRESS)
        .await
        .map_err(|_| Error::Transport)?;
    axum::serve(listener, router(coordinator))
        .await
        .map_err(|_| Error::Transport)
}
