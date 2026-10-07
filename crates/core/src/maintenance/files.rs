use super::{
    workflow::{self, Operation},
    *,
};
use crate::cleanup::*;
use std::{collections::HashMap, time::SystemTime};
pub(super) struct Request<'a, F, G, P> {
    pub files: &'a F,
    pub guard: &'a G,
    pub options: &'a RunOptions,
    pub cutoff: SystemTime,
    pub references: &'a FileReferences,
    pub cancel: &'a Cancellation,
    pub progress: &'a P,
    pub operation: Operation,
}
pub(super) async fn run<F: FullFileStore, G: OfflineGuard, P: Fn(Progress) + Send + Sync>(
    request: Request<'_, F, G, P>,
    summary: &mut Summary,
) -> Result<(), CleanupError> {
    let Request {
        files,
        guard,
        cancel,
        ..
    } = &request;
    workflow::cancelled(cancel, *summary)?;
    workflow::at(guard.ensure_stopped().await, Stage::Preflight, *summary)?;
    workflow::at(files.validate_tree().await, Stage::Scan, *summary)?;
    workflow::cancelled(cancel, *summary)?;
    let mut scan = workflow::at(files.open_scan().await, Stage::Scan, *summary)?;
    let result = iterate(&mut scan, &request, summary).await;
    let closed = scan.close().await;
    match (result, closed) {
        (Err(mut error), Err(close)) => {
            error.source = format!("{}; scan close also failed: {close}", error.source).into();
            Err(error)
        }
        (Err(error), _) => Err(error),
        (Ok(()), close) => workflow::at(close, Stage::Scan, *summary),
    }
}
async fn iterate<S: FileScan, F: FullFileStore, G: OfflineGuard, P: Fn(Progress) + Send + Sync>(
    scan: &mut S,
    request: &Request<'_, F, G, P>,
    summary: &mut Summary,
) -> Result<(), CleanupError> {
    let Request {
        files,
        guard,
        options,
        cutoff,
        references,
        cancel,
        progress,
        operation,
    } = request;
    loop {
        workflow::cancelled(cancel, *summary)?;
        let batch = workflow::at(
            scan.next_batch(options.batch_size.get()).await,
            Stage::Scan,
            *summary,
        )?;
        let valid = batch.visited_entries <= u64::from(options.batch_size.get())
            && batch.examined_files <= batch.visited_entries
            && batch.files.len() as u64 <= batch.examined_files
            && (batch.finished || batch.visited_entries > 0);
        workflow::at(
            if valid {
                Ok(())
            } else {
                Err("invalid scan batch".into())
            },
            Stage::Scan,
            *summary,
        )?;
        summary.scanned_files += batch.examined_files;
        let selected = batch
            .files
            .into_iter()
            .filter(|file| file.modified_at < *cutoff && !references.contains(&file.guid))
            .collect::<Vec<_>>();
        summary.selected_orphan_files += selected.len() as u64;
        if *operation == Operation::Clean && !selected.is_empty() {
            workflow::at(guard.ensure_stopped().await, Stage::Preflight, *summary)?;
            let expected = selected
                .iter()
                .map(|file| (file.path.clone(), file.clone()))
                .collect::<HashMap<_, _>>();
            workflow::at(
                if expected.len() == selected.len() {
                    Ok(())
                } else {
                    Err("duplicate orphan scan path".into())
                },
                Stage::Scan,
                *summary,
            )?;
            let outcomes = workflow::at(
                files.delete_orphans(selected, options.workers).await,
                Stage::Files,
                *summary,
            )?;
            let mut pending = expected;
            let valid = outcomes.len() == pending.len()
                && outcomes.iter().all(|outcome| {
                    pending.remove(&outcome.observation.path).as_ref() == Some(&outcome.observation)
                });
            workflow::at(
                if valid {
                    Ok(())
                } else {
                    Err("unexpected orphan file outcome".into())
                },
                Stage::Files,
                *summary,
            )?;
            let mut parents = Vec::new();
            for outcome in outcomes {
                match outcome.result {
                    DeletionResult::Removed { size_bytes } => {
                        summary.removed_files += 1;
                        summary.removed_orphan_files += 1;
                        summary.removed_bytes += size_bytes;
                    }
                    DeletionResult::AlreadyAbsent => summary.absent_files += 1,
                    DeletionResult::Failed(failure) => {
                        summary.failed += 1;
                        progress(Progress {
                            stage: Stage::Files,
                            summary: *summary,
                            failure: Some(Failure::OrphanFile {
                                path: outcome.observation.path,
                                reason: failure.message,
                            }),
                        });
                        continue;
                    }
                }
                if let Some(parent) = outcome.observation.path.parent() {
                    parents.push(parent.to_path_buf());
                }
            }
            let prune = workflow::at(
                files.prune_empty(parents).await,
                Stage::Directories,
                *summary,
            )?;
            summary.removed_directories += prune.removed;
            for failure in prune.failures {
                summary.failed += 1;
                progress(Progress {
                    stage: Stage::Directories,
                    summary: *summary,
                    failure: Some(Failure::Directory {
                        path: failure.path,
                        reason: failure.reason,
                    }),
                });
            }
        }
        workflow::emit(Stage::Scan, *summary, progress);
        if batch.finished {
            workflow::cancelled(cancel, *summary)?;
            return Ok(());
        }
    }
}
