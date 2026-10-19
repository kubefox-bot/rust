//! Explicit side-effect ordering. A crash after Exposing may never restore old data.
use crate::{
    control::State,
    journal::{Phase, Transaction},
    Error,
};
use hmail_cleaner_protocol::browser::updates::UpdatePolicy;
use std::time::Duration;
pub mod native;
pub mod recover;
pub trait Host: Sync {
    fn record(&self, tx: &Transaction) -> Result<(), Error>;
    fn graceful(&self) -> Duration;
    fn begin(
        &self,
        tx: &Transaction,
    ) -> impl std::future::Future<Output = Result<State, Error>> + Send;
    fn status(
        &self,
        tx: &Transaction,
    ) -> impl std::future::Future<Output = Result<State, Error>> + Send;
    fn abort(
        &self,
        tx: &Transaction,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send;
    fn stop(&self) -> impl std::future::Future<Output = Result<(), Error>> + Send;
    fn backup(
        &self,
        tx: &Transaction,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send;
    fn start_candidate(
        &self,
        tx: &Transaction,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send;
    fn verify(
        &self,
        tx: &Transaction,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send;
    fn expose(
        &self,
        tx: &Transaction,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send;
    fn restore(
        &self,
        tx: &Transaction,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send;
    fn restart_previous(
        &self,
        tx: &Transaction,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send;
    fn quarantine(&self, tx: &Transaction) -> Result<(), Error>;
    fn commit(&self, tx: &Transaction) -> Result<(), Error>;
}
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Committed,
    Deferred,
    Restored,
}
fn phase(host: &impl Host, tx: &mut Transaction, next: Phase) -> Result<(), Error> {
    tx.phase = next;
    host.record(tx)
}
pub async fn run(
    host: &impl Host,
    tx: &mut Transaction,
    policy: UpdatePolicy,
) -> Result<Outcome, Error> {
    policy.validate().map_err(|_| Error::Invalid)?;
    phase(host, tx, Phase::Draining)?;
    let result = tokio::time::timeout(host.graceful(), async {
        let mut status = host.begin(tx).await?;
        while status != State::Ready {
            tokio::time::sleep(Duration::from_secs(1)).await;
            status = host.status(tx).await?;
        }
        Ok::<(), Error>(())
    })
    .await;
    match result {
        Ok(Ok(())) => (),
        Ok(Err(error)) => {
            host.abort(tx).await?;
            phase(host, tx, Phase::Failed)?;
            return Err(error);
        }
        Err(_) => {
            host.abort(tx).await?;
            phase(host, tx, Phase::Failed)?;
            return Ok(Outcome::Deferred);
        }
    }
    // Stopped records intent first. Recovery must stop/confirm old process before inspecting data.
    phase(host, tx, Phase::Stopped)?;
    host.stop().await?;
    // No candidate ever started on backup failure; caller restarts the old image.
    host.backup(tx).await?;
    phase(host, tx, Phase::BackedUp)?;
    phase(host, tx, Phase::Starting)?;
    let result = async {
        host.start_candidate(tx).await?;
        host.verify(tx).await?;
        Ok::<(), Error>(())
    }
    .await;
    if result.is_err() {
        phase(host, tx, Phase::Restoring)?;
        host.stop().await?;
        host.restore(tx).await?;
        finish_rollback(host, tx).await?;
        return Ok(Outcome::Restored);
    }
    phase(host, tx, Phase::Verified)?;
    phase(host, tx, Phase::Exposing)?; // Persist before the first request that could reopen writes.
    host.expose(tx).await?;
    host.commit(tx)?;
    phase(host, tx, Phase::Committed)?;
    Ok(Outcome::Committed)
}

// Both exposure barriers precede gate release. Recovery must never restore after them.
async fn finish_rollback(host: &impl Host, tx: &mut Transaction) -> Result<(), Error> {
    phase(host, tx, Phase::RollbackExposing)?;
    host.abort(tx).await?;
    host.quarantine(tx)?;
    phase(host, tx, Phase::Quarantined)
}
async fn finish_previous(host: &impl Host, tx: &mut Transaction) -> Result<(), Error> {
    phase(host, tx, Phase::PreviousExposing)?;
    host.abort(tx).await?;
    phase(host, tx, Phase::Failed)
}

pub mod boot;
