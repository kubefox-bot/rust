//! Download preparation must finish before acquiring the center maintenance gate.
use crate::{config::Config, delivery::Descriptor, disk, podman, Error};
const STAGING_HEADROOM_BYTES: u64 = 256 * 1024 * 1024;
pub trait Host: Sync {
    fn preflight(&self) -> impl std::future::Future<Output = Result<(), Error>> + Send;
    fn pull(&self) -> impl std::future::Future<Output = Result<(), Error>> + Send;
}
pub async fn run(host: &impl Host) -> Result<(), Error> {
    host.preflight().await?;
    host.pull().await?;
    host.preflight().await
}
pub struct NativeHost<'a> {
    pub config: &'a Config,
    pub descriptor: &'a Descriptor,
}
impl Host for NativeHost<'_> {
    async fn preflight(&self) -> Result<(), Error> {
        let mountpoint = podman::mountpoint().await?;
        let budget = disk::required(disk::tree_bytes(&mountpoint)?)?;
        disk::ensure(&self.config.backup_dir, budget)?;
        disk::ensure(&self.config.staging_dir, STAGING_HEADROOM_BYTES)
    }
    async fn pull(&self) -> Result<(), Error> {
        podman::pull(self.config, self.descriptor).await
    }
}
