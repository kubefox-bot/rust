pub use super::boot::prepare;
use super::{native::NativeHost, Host};
use crate::{
    journal::{self, Phase, Transaction},
    Error,
};
pub async fn resume(host: &impl Host, tx: &mut Transaction) -> Result<(), Error> {
    match tx.phase {
        Phase::Committed | Phase::Failed | Phase::Quarantined | Phase::Staged => Ok(()),
        Phase::Draining => {
            host.abort(tx).await?;
            tx.phase = Phase::Failed;
            host.record(tx)
        }
        Phase::Stopped => {
            host.stop().await?;
            host.restart_previous(tx).await?;
            super::finish_previous(host, tx).await
        }
        Phase::BackedUp | Phase::Starting | Phase::Verified | Phase::Restoring => {
            super::phase(host, tx, Phase::Restoring)?;
            host.stop().await?;
            host.restore(tx).await?;
            super::finish_rollback(host, tx).await
        }
        Phase::RollbackExposing => {
            host.restart_previous(tx).await?;
            super::finish_rollback(host, tx).await
        }
        Phase::PreviousExposing => {
            host.restart_previous(tx).await?;
            super::finish_previous(host, tx).await
        }
        Phase::Exposing => {
            host.verify(tx).await?;
            host.expose(tx).await?;
            host.commit(tx)?;
            tx.phase = Phase::Committed;
            host.record(tx)
        }
    }
}
pub fn load(host: &NativeHost<'_>) -> Result<Option<Transaction>, Error> {
    let tx = journal::load::<Transaction>(&host.tx_path())?;
    if tx.as_ref().is_some_and(|value| {
        value.version != 1
            || value.id.is_nil()
            || value.backup
                != host
                    .config
                    .backup_dir
                    .join(value.id.to_string())
                    .join("snapshot")
            || value.previous_unit
                != host
                    .config
                    .state_dir
                    .join(format!("previous-{}.container", value.id))
            || value.candidate_export
                != host
                    .config
                    .backup_dir
                    .join(value.id.to_string())
                    .join("candidate.tar")
    }) {
        return Err(Error::Invalid);
    }
    Ok(tx)
}
