//! Runs before center startup; never starts or waits on that dependent unit.
use crate::{
    journal::{Phase, Transaction},
    Error,
};
pub trait Host: Sync {
    fn select_previous(&self, tx: &Transaction) -> Result<(), Error>;
    fn reload(&self) -> impl std::future::Future<Output = Result<(), Error>> + Send;
    fn mark(&self, tx: &Transaction)
        -> impl std::future::Future<Output = Result<(), Error>> + Send;
    fn restore_offline(
        &self,
        tx: &mut Transaction,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send;
}
pub async fn prepare(host: &impl Host, tx: &mut Transaction) -> Result<(), Error> {
    match tx.phase {
        Phase::Committed | Phase::Failed | Phase::Quarantined | Phase::Staged | Phase::Exposing => {
            Ok(())
        }
        Phase::Draining | Phase::Stopped => {
            host.select_previous(tx)?;
            host.reload().await?;
            host.mark(tx).await
        }
        Phase::BackedUp | Phase::Starting | Phase::Verified | Phase::Restoring => {
            host.restore_offline(tx).await?;
            host.select_previous(tx)?;
            host.reload().await
        }
        Phase::RollbackExposing | Phase::PreviousExposing => {
            host.select_previous(tx)?;
            host.reload().await
        }
    }
}
