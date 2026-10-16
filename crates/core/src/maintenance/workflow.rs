use super::*;
use crate::cleanup::{self, *};
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    Preview,
    Clean,
}
pub(super) fn at<T>(
    result: PortResult<T>,
    stage: Stage,
    summary: Summary,
) -> Result<T, CleanupError> {
    result.map_err(|source| CleanupError {
        stage,
        summary: Box::new(summary),
        source,
    })
}
pub(super) fn cancelled(cancel: &Cancellation, summary: Summary) -> Result<(), CleanupError> {
    at(
        if cancel.is_requested() {
            Err("Операция отменена после завершения текущего пакета".into())
        } else {
            Ok(())
        },
        Stage::Cancelled,
        summary,
    )
}
pub(super) fn emit(stage: Stage, summary: Summary, progress: &(impl Fn(Progress) + Send + Sync)) {
    progress(Progress {
        stage,
        summary,
        failure: None,
    });
}
pub async fn clean<
    R: FullRepository,
    F: FullFileStore,
    G: OfflineGuard,
    P: Fn(Progress) + Send + Sync,
>(
    deps: Dependencies<'_, R, F, G>,
    options: &RunOptions,
    scan: ScanOptions,
    cancel: &Cancellation,
    progress: P,
) -> Result<Summary, CleanupError> {
    run(deps, options, scan, cancel, progress, Operation::Clean).await
}
pub async fn preview<
    R: FullRepository,
    F: FullFileStore,
    G: OfflineGuard,
    P: Fn(Progress) + Send + Sync,
>(
    deps: Dependencies<'_, R, F, G>,
    options: &RunOptions,
    scan: ScanOptions,
    cancel: &Cancellation,
    progress: P,
) -> Result<Summary, CleanupError> {
    run(deps, options, scan, cancel, progress, Operation::Preview).await
}
async fn run<
    R: FullRepository,
    F: FullFileStore,
    G: OfflineGuard,
    P: Fn(Progress) + Send + Sync,
>(
    deps: Dependencies<'_, R, F, G>,
    options: &RunOptions,
    scan: ScanOptions,
    cancel: &Cancellation,
    progress: P,
    operation: Operation,
) -> Result<Summary, CleanupError> {
    let ScanOptions::Full = scan else {
        return match operation {
            Operation::Clean => cleanup::clean(deps, options, cancel, progress).await,
            Operation::Preview => {
                cleanup::preview(deps.repository, deps.files, options, progress).await
            }
        };
    };
    cancelled(cancel, Summary::default())?;
    at(
        deps.guard.ensure_stopped().await,
        Stage::Preflight,
        Summary::default(),
    )?;
    let prefix = |event: Progress| {
        if event.stage != Stage::Complete {
            progress(event)
        }
    };
    let mut summary = match operation {
        Operation::Clean => {
            cleanup::clean(
                Dependencies {
                    repository: deps.repository,
                    files: deps.files,
                    guard: deps.guard,
                },
                options,
                cancel,
                prefix,
            )
            .await?
        }
        Operation::Preview => {
            cleanup::preview(deps.repository, deps.files, options, prefix).await?
        }
    };
    database::run(
        deps.repository,
        deps.guard,
        options,
        cancel,
        &progress,
        operation,
        &mut summary,
    )
    .await?;
    cancelled(cancel, summary)?;
    let references = at(
        deps.repository.file_references(MAX_REFERENCES).await,
        Stage::References,
        summary,
    )?;
    at(
        if references.len() <= MAX_REFERENCES {
            Ok(())
        } else {
            Err("reference set exceeds memory limit".into())
        },
        Stage::References,
        summary,
    )?;
    emit(Stage::References, summary, &progress);
    files::run(
        files::Request {
            files: deps.files,
            guard: deps.guard,
            options,
            cutoff: options.file_cutoff,
            references: &references,
            cancel,
            progress: &progress,
            operation,
        },
        &mut summary,
    )
    .await?;
    emit(Stage::Complete, summary, &progress);
    Ok(summary)
}
