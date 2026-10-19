use std::{ffi::OsString, path::PathBuf};
#[derive(Debug, Clone)]
pub struct ServiceRegistration {
    pub name: String,
    pub executable: PathBuf,
    pub arguments: Vec<OsString>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceState {
    Running,
    Stopped,
    Transitioning,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Presence {
    Absent,
    Owned(ServiceState),
    Collision,
}
pub struct InstallServiceInput {
    pub operation_id: Option<uuid::Uuid>,
    pub package: PathBuf,
    pub executable: PathBuf,
    pub config: PathBuf,
    pub data_directory: PathBuf,
    pub expected_sha256: [u8; 32],
    /// Exact trusted legacy executable and arguments; absence never authorizes disabling it.
    pub legacy: Option<ServiceRegistration>,
}
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstallationReceipt {
    #[serde(default)]
    pub operation_id: Option<uuid::Uuid>,
    pub executable: PathBuf,
    pub sha256: String,
    #[serde(default)]
    pub previous_sha256: Option<String>,
    pub started: bool,
}
#[derive(Debug, thiserror::Error)]
pub enum InstallError {
    #[error("job or installation is active")]
    Busy,
    #[error("service name belongs to another installation")]
    Collision,
    #[error("invalid installation input or package checksum")]
    Invalid,
    #[error("installation requires Windows")]
    Unsupported,
    #[error("installation filesystem operation failed")]
    Io(#[from] std::io::Error),
    #[error("agent ledger failed")]
    Storage(#[from] crate::jobs::JobStoreError),
    #[error("native service operation failed")]
    Native,
}
pub trait ServiceRegistry {
    fn inspect(&self, registration: &ServiceRegistration) -> Result<Presence, InstallError>;
    fn stop_and_wait(&self, registration: &ServiceRegistration) -> Result<(), InstallError>;
    fn disable(&self, registration: &ServiceRegistration) -> Result<(), InstallError>;
    fn configure_and_start(&self, registration: &ServiceRegistration) -> Result<(), InstallError>;
}
