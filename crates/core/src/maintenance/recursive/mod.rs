//! Recursive file-first maintenance. The database map contains rows, never file contents.
mod batch;
mod commit;
mod missing;
mod validation;
mod verification;
pub use super::workflow::Operation as Action;
use super::{
    workflow::{self, Operation},
    FileScan, FullFileStore, FullRepository, ScanBatch, ScanOptions, StoredMessage, MAX_REFERENCES,
};
use crate::cleanup::*;
use std::{collections::HashMap, future::Future, path::Path};

pub trait TreeRepository: FullRepository {
    fn stored_messages(
        &mut self,
        root: &Path,
        limit: usize,
    ) -> impl Future<Output = PortResult<Vec<StoredMessage>>> + Send;
    fn delete_stored(
        &mut self,
        rows: &[StoredMessage],
    ) -> impl Future<Output = PortResult<DeleteResult>> + Send;
}
pub(super) fn path_key(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/").to_lowercase()
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
    mut deps: Dependencies<'_, R, F, G>,
    options: &RunOptions,
    scan: ScanOptions,
    cancel: &Cancellation,
    progress: P,
    operation: Action,
) -> Result<Summary, CleanupError> {
    let mut summary = Summary::default();
    workflow::emit(Stage::Preflight, summary, &progress);
    workflow::cancelled(cancel, summary)?;
    // Fast preview is read-only and remains usable while mail is running.
    if scan.requires_stopped(operation) {
        workflow::at(deps.guard.ensure_stopped().await, Stage::Preflight, summary)?;
    }
    workflow::emit(Stage::References, summary, &progress);
    let rows = workflow::at(
        deps.repository
            .stored_messages(&options.data_directory, MAX_REFERENCES)
            .await,
        Stage::References,
        summary,
    )?;
    workflow::at(
        validation::inventory(&rows, &options.data_directory),
        Stage::References,
        summary,
    )?;
    let mut map: HashMap<String, Vec<StoredMessage>> = HashMap::new();
    for row in rows {
        map.entry(path_key(&row.path)).or_default().push(row);
    }
    if matches!(scan, ScanOptions::Full) {
        workflow::emit(Stage::Verification, summary, &progress);
        summary.total_scan_files = Some(
            workflow::at(
                deps.files.validate_tree().await,
                Stage::Verification,
                summary,
            )?
            .examined_files,
        );
    }
    workflow::emit(Stage::Scan, summary, &progress);
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
                &progress,
                &mut map,
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
        map,
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
