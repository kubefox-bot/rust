use super::VaultError;
use std::sync::Arc;
use tokio::sync::Semaphore;
const MAX_KDFS: usize = 1;
#[derive(Clone)]
pub struct KdfGate(Arc<Semaphore>);
#[derive(Debug, thiserror::Error)]
pub enum KdfError {
    #[error("another KDF is running")]
    Busy,
    #[error("KDF failed")]
    Vault(#[from] VaultError),
    #[error("KDF worker failed")]
    Worker,
}
impl Default for KdfGate {
    fn default() -> Self {
        Self::new()
    }
}
impl KdfGate {
    pub fn new() -> Self {
        Self(Arc::new(Semaphore::new(MAX_KDFS)))
    }
    pub async fn try_run<T: Send + 'static>(
        &self,
        job: impl FnOnce() -> Result<T, VaultError> + Send + 'static,
    ) -> Result<T, KdfError> {
        let permit = self
            .0
            .clone()
            .try_acquire_owned()
            .map_err(|_| KdfError::Busy)?;
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            job()
        })
        .await
        .map_err(|_| KdfError::Worker)?
        .map_err(Into::into)
    }
}
