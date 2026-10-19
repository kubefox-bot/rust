//! Recursive file-first maintenance with bounded inventory and filesystem batches.
mod batch;
mod commit;
mod inventory;
mod missing;
mod validation;
mod verification;
pub use super::workflow::Operation as Action;
use super::{
    workflow::{self, Operation},
    FileScan, FullFileStore, FullRepository, ScanBatch, ScanOptions, StoredMessage,
};
use crate::cleanup::*;
use crate::paths::path_key;
pub use inventory::{StoredGroup, TreeRepository};
use std::{future::Future, path::Path};

/// Awaited at the physical-file selection boundary; failures stop the preview.
pub trait SelectionObserver: Send + Sync {
    fn selected(&self, files: &[SelectedFile]) -> impl Future<Output = PortResult<()>> + Send;
}
#[derive(Debug, Clone)]
pub struct SelectedFile {
    pub file: super::FileObservation,
    pub orphan: bool,
}
struct IgnoreSelection;
impl SelectionObserver for IgnoreSelection {
    async fn selected(&self, _: &[SelectedFile]) -> PortResult<()> {
        Ok(())
    }
}

pub(super) fn fail(
    summary: &mut Summary,
    path: std::path::PathBuf,
    reason: String,
    progress: &(impl Fn(Progress) + Send + Sync),
) {
    summary.failed += 1;
    progress(Progress {
        stage: Stage::Files,
        summary: *summary,
        failure: Some(Failure::OrphanFile { path, reason }),
    });
}
pub async fn run<
    R: TreeRepository,
    F: FullFileStore,
    G: OfflineGuard,
    P: Fn(Progress) + Send + Sync,
>(
    deps: Dependencies<'_, R, F, G>,
    options: &RunOptions,
    scan: ScanOptions,
    cancel: &Cancellation,
    progress: P,
    operation: Action,
) -> Result<Summary, CleanupError> {
    run_with_details(
        deps,
        options,
        scan,
        cancel,
        progress,
        operation,
        &IgnoreSelection,
    )
    .await
}

pub async fn run_with_details<
    R: TreeRepository,
    F: FullFileStore,
    G: OfflineGuard,
    P: Fn(Progress) + Send + Sync,
>(
    mut deps: Dependencies<'_, R, F, G>,
    options: &RunOptions,
    scan: ScanOptions,
    cancel: &Cancellation,
    progress: P,
    operation: Action,
    observer: &impl SelectionObserver,
) -> Result<Summary, CleanupError> {
    let mut summary = Summary::default();
    workflow::emit(Stage::Preflight, summary, &progress);
    workflow::cancelled(cancel, summary)?;
    // Fast preview is read-only and remains usable while mail is running.
    if scan.requires_stopped(operation) {
        workflow::at(deps.guard.ensure_stopped().await, Stage::Preflight, summary)?;
    }
    workflow::emit(Stage::References, summary, &progress);
    workflow::cancelled(cancel, summary)?;
    let inventory = deps
        .repository
        .prepare_inventory(&options.data_directory, cancel)
        .await;
    workflow::cancelled(cancel, summary)?;
    workflow::at(inventory, Stage::References, summary)?;
    if matches!(scan, ScanOptions::Full) {
        workflow::emit(Stage::Verification, summary, &progress);
        workflow::cancelled(cancel, summary)?;
        let checked = deps.files.validate_tree(cancel).await;
        workflow::cancelled(cancel, summary)?;
        summary.total_scan_files =
            Some(workflow::at(checked, Stage::Verification, summary)?.examined_files);
    }
    workflow::emit(Stage::Scan, summary, &progress);
    workflow::cancelled(cancel, summary)?;
    let mut session = workflow::at(deps.files.open_scan().await, Stage::Scan, summary)?;
    let result: Result<(), CleanupError> = async {
        loop {
            workflow::cancelled(cancel, summary)?;
            workflow::emit(Stage::Scan, summary, &progress);
            let scanned = workflow::at(
                session.next_batch(options.batch_size.get()).await,
                Stage::Scan,
                summary,
            )?;
            workflow::at(validation::scan(&scanned, options), Stage::Scan, summary)?;
            let finished = scanned.finished;
            summary.scanned_files += scanned.examined_files;
            batch::run(
                &mut deps,
                options,
                operation,
                batch::Observers {
                    progress: &progress,
                    selection: observer,
                },
                scanned,
                &mut summary,
            )
            .await?;
            if finished {
                break;
            }
        }
        Ok(())
    }
    .await;
    let close = session.close().await;
    match (result, close) {
        (Err(mut error), Err(close)) => {
            error.source = format!("{}; закрытие обхода: {close}", error.source).into();
            return Err(error);
        }
        (Err(error), _) => return Err(error),
        (Ok(()), close) => workflow::at(close, Stage::Scan, summary)?,
    }
    missing::run(
        &mut deps,
        options,
        operation,
        cancel,
        &progress,
        &mut summary,
    )
    .await?;
    if matches!(scan, ScanOptions::Full) {
        super::database::run(
            deps.repository,
            deps.guard,
            options,
            cancel,
            &progress,
            operation,
            &mut summary,
        )
        .await?;
        if operation == Operation::Clean {
            verification::run(deps.files, options, cancel, &progress, &mut summary).await?;
        }
    }
    workflow::emit(Stage::Complete, summary, &progress);
    Ok(summary)
}
