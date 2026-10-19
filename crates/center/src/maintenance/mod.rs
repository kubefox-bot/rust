//! Local update admission. The marker is loaded before serving any browser requests.
mod gate;
mod http;
mod marker;
#[cfg(unix)]
mod socket;
pub use gate::{MaintenanceError, MaintenanceGate, MaintenanceStatus, Snapshot, WritePermit};
pub(crate) use http::admission;
#[cfg(unix)]
pub(crate) use socket::{ControlServer, DIRECTORY as SOCKET_DIRECTORY};
