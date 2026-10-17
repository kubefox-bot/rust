mod install;
mod metadata;
mod package;
pub mod security;
mod types;
#[cfg(windows)]
mod windows;
pub use install::{install_with, SERVICE_NAME};
pub use types::{
    InstallError, InstallServiceInput, InstallationReceipt, Presence, ServiceRegistration,
    ServiceRegistry, ServiceState,
};

pub async fn install(input: InstallServiceInput) -> Result<InstallationReceipt, InstallError> {
    #[cfg(windows)]
    {
        install_with(input, &windows::WindowsRegistry::new()?).await
    }
    #[cfg(not(windows))]
    {
        let _ = input;
        Err(InstallError::Unsupported)
    }
}
