use super::{
    workflow::{self, Operation},
    *,
};
use crate::cleanup::*;
use std::collections::HashSet;
pub(super) async fn run<R: FullRepository, G: OfflineGuard, P: Fn(Progress) + Send + Sync>(
    repository: &mut R,
    guard: &G,
    options: &RunOptions,
    cancel: &Cancellation,
    progress: &P,
    operation: Operation,
    summary: &mut Summary,
) -> Result<(), CleanupError> {
    for table in [OrphanTable::Metadata, OrphanTable::Recipients] {
        workflow::cancelled(cancel, *summary)?;
        let upper_id = workflow::at(
            repository.orphan_upper_id(table).await,
            Stage::OrphanDatabase,
            *summary,
        )?;
        workflow::at(
            if upper_id >= 0 {
                Ok(())
            } else {
                Err("negative orphan watermark".into())
            },
            Stage::OrphanDatabase,
            *summary,
        )?;
        let selection = OrphanSelection { table, upper_id };
        let mut cursor = 0;
        loop {
            workflow::cancelled(cancel, *summary)?;
            let batch = workflow::at(
                repository
                    .orphan_batch(selection, cursor, options.batch_size.get())
                    .await,
                Stage::OrphanDatabase,
                *summary,
            )?;
            if batch.rows.is_empty() {
                workflow::at(
                    if batch.last_id == cursor {
                        Ok(())
                    } else {
                        Err("empty orphan batch advanced cursor".into())
                    },
                    Stage::OrphanDatabase,
                    *summary,
                )?;
                break;
            }
            let ids = batch.rows.iter().map(|row| row.id).collect::<Vec<_>>();
            let unique = ids.iter().copied().collect::<HashSet<_>>();
            let valid = batch.rows.len() <= options.batch_size.get() as usize
                && unique.len() == ids.len()
                && ids.iter().all(|id| *id > cursor && *id <= upper_id)
                && ids.windows(2).all(|pair| pair[0] < pair[1])
                && ids.last() == Some(&batch.last_id);
            workflow::at(
                if valid {
                    Ok(())
                } else {
                    Err("invalid orphan selection batch".into())
                },
                Stage::OrphanDatabase,
                *summary,
            )?;
            cursor = batch.last_id;
            if operation == Operation::Clean {
                workflow::at(guard.ensure_stopped().await, Stage::Preflight, *summary)?;
                let rows = match workflow::at(
                    repository.delete_orphans(table, &batch.rows).await,
                    Stage::OrphanDatabase,
                    *summary,
                )? {
                    DeleteResult::Committed(rows) => rows,
                    DeleteResult::CommitUncertain { reason } => {
                        return workflow::at(
                            Err(format!(
                                "COMMIT confirmation lost; repeat cleanup to reconcile: {reason}"
                            )
                            .into()),
                            Stage::OrphanDatabase,
                            *summary,
                        )
                    }
                };
                let expected = match table {
                    OrphanTable::Metadata => DeletedRows {
                        metadata: ids.len() as u64,
                        ..DeletedRows::default()
                    },
                    OrphanTable::Recipients => DeletedRows {
                        recipients: ids.len() as u64,
                        ..DeletedRows::default()
                    },
                };
                workflow::at(
                    if rows == expected {
                        Ok(())
                    } else {
                        Err("unexpected orphan deletion count".into())
                    },
                    Stage::OrphanDatabase,
                    *summary,
                )?;
                workflow::at(
                    repository.verify_orphans_deleted(table, &ids).await,
                    Stage::Verification,
                    *summary,
                )?;
            }
            match table {
                OrphanTable::Metadata => summary.orphan_metadata += ids.len() as u64,
                OrphanTable::Recipients => summary.orphan_recipients += ids.len() as u64,
            };
            workflow::emit(Stage::OrphanDatabase, *summary, progress);
        }
    }
    Ok(())
}
