use super::{
    batch::{self, emit},
    file_results,
    progress::at,
    *,
};

pub struct Dependencies<'a, R, F, G> {
    pub repository: &'a mut R,
    pub files: &'a F,
    pub guard: &'a G,
}
pub async fn clean<R: Repository, F: FileStore, G: OfflineGuard, P: Fn(Progress) + Send + Sync>(
    dependencies: Dependencies<'_, R, F, G>,
    options: &RunOptions,
    cancellation: &Cancellation,
    progress: P,
) -> Result<Summary, CleanupError> {
    let Dependencies {
        repository,
        files,
        guard,
    } = dependencies;
    let mut summary = Summary::default();
    at(batch::validate_options(options), Stage::Preflight, summary)?;
    cancelled(cancellation, summary)?;
    at(guard.ensure_stopped().await, Stage::Preflight, summary)?;
    let snapshot = at(repository.snapshot().await, Stage::Snapshot, summary)?;
    let selection = Selection {
        cutoff: options.cutoff,
        upper_id: snapshot.upper_id,
    };
    let mut cursor = 0;
    loop {
        cancelled(cancellation, summary)?;
        let batch = at(
            repository
                .next_batch(selection, cursor, options.batch_size.get())
                .await,
            Stage::Selection,
            summary,
        )?;
        if !at(
            batch::validate(&batch, cursor, selection.upper_id),
            Stage::Selection,
            summary,
        )? {
            break;
        }
        at(guard.ensure_stopped().await, Stage::Preflight, summary)?;
        cursor = batch.last_id;
        let planned = batch::resolve(batch, options, &mut summary, &progress);
        emit(Stage::Selection, summary, &progress);
        if planned.is_empty() {
            continue;
        }
        let targets = planned
            .iter()
            .map(|(id, (_, path))| FileTarget {
                id: *id,
                path: path.clone(),
            })
            .collect();
        let outcomes = at(
            files.delete(targets, options.workers).await,
            Stage::Files,
            summary,
        )?;
        let accepted = file_results::accept(planned, outcomes, &mut summary, &progress);
        let (eligible, parents) = at(accepted, Stage::Files, summary)?;
        emit(Stage::Files, summary, &progress);
        if eligible.is_empty() {
            continue;
        }
        let deleted = match at(
            repository.delete_batch(&eligible).await,
            Stage::Database,
            summary,
        )? {
            DeleteResult::Committed(rows) => rows,
            DeleteResult::CommitUncertain { reason } => {
                return Err(CleanupError {
                    stage: Stage::Database,
                    summary: Box::new(summary),
                    source: format!(
                        "COMMIT confirmation lost; repeat cleanup to reconcile: {reason}"
                    )
                    .into(),
                })
            }
        };
        at(
            if deleted.messages == eligible.len() as u64 {
                Ok(())
            } else {
                Err("database returned an unexpected committed message count".into())
            },
            Stage::Database,
            summary,
        )?;
        let ids = eligible.iter().map(|row| row.id).collect::<Vec<_>>();
        at(
            repository.verify_deleted(&ids).await,
            Stage::Verification,
            summary,
        )?;
        summary.deleted_messages += eligible.len() as u64;
        summary.deleted_metadata += deleted.metadata;
        summary.deleted_recipients += deleted.recipients;
        emit(Stage::Database, summary, &progress);
        let pruning = at(
            files.prune_empty(parents).await,
            Stage::Directories,
            summary,
        )?;
        summary.removed_directories += pruning.removed;
        for directory in pruning.failures {
            batch::failure(
                &mut summary,
                Stage::Directories,
                Failure::Directory {
                    path: directory.path,
                    reason: directory.reason,
                },
                &progress,
            );
        }
        emit(Stage::Directories, summary, &progress);
    }
    emit(Stage::Complete, summary, &progress);
    Ok(summary)
}
pub async fn preview<R: Repository, P: Fn(Progress) + Send + Sync>(
    repository: &mut R,
    options: &RunOptions,
    progress: P,
) -> Result<Summary, CleanupError> {
    let mut summary = Summary::default();
    at(batch::validate_options(options), Stage::Preflight, summary)?;
    let snapshot = at(repository.snapshot().await, Stage::Snapshot, summary)?;
    let selection = Selection {
        cutoff: options.cutoff,
        upper_id: snapshot.upper_id,
    };
    let mut cursor = 0;
    loop {
        let batch = at(
            repository
                .next_batch(selection, cursor, options.batch_size.get())
                .await,
            Stage::Selection,
            summary,
        )?;
        if !at(
            batch::validate(&batch, cursor, selection.upper_id),
            Stage::Selection,
            summary,
        )? {
            break;
        }
        cursor = batch.last_id;
        batch::resolve(batch, options, &mut summary, &progress);
        emit(Stage::Selection, summary, &progress);
    }
    emit(Stage::Complete, summary, &progress);
    Ok(summary)
}
fn cancelled(cancellation: &Cancellation, summary: Summary) -> Result<(), CleanupError> {
    if cancellation.is_requested() {
        return Err(CleanupError {
            stage: Stage::Cancelled,
            summary: Box::new(summary),
            source: "cancelled after completing the started batch".into(),
        });
    }
    Ok(())
}
