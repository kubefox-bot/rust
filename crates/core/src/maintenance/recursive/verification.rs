use super::*;
pub(super) async fn run<F: FullFileStore, P: Fn(Progress) + Send + Sync>(
    files: &F,
    options: &RunOptions,
    cancel: &Cancellation,
    progress: &P,
    summary: &mut Summary,
) -> Result<(), CleanupError> {
    workflow::emit(Stage::Verification, *summary, progress);
    let mut scan = workflow::at(files.open_scan().await, Stage::Verification, *summary)?;
    let result: Result<u64, CleanupError> = async {
        let mut remaining = 0;
        loop {
            workflow::cancelled(cancel, *summary)?;
            let batch = workflow::at(
                scan.next_batch(options.batch_size.get()).await,
                Stage::Verification,
                *summary,
            )?;
            workflow::at(
                validation::scan(&batch, options),
                Stage::Verification,
                *summary,
            )?;
            summary.verified_files += batch.examined_files;
            remaining += batch
                .files
                .iter()
                .filter(|file| file.created_at < options.file_cutoff)
                .count() as u64;
            workflow::emit(Stage::Verification, *summary, progress);
            if batch.finished {
                return Ok(remaining);
            }
        }
    }
    .await;
    let close = scan.close().await;
    let remaining = match result {
        Ok(remaining) => remaining,
        Err(mut error) => {
            if let Err(close) = close {
                error.source = format!("{}; закрытие проверки: {close}", error.source).into();
            }
            return Err(error);
        }
    };
    workflow::at(close, Stage::Verification, *summary)?;
    summary.remaining_old_files = Some(remaining);
    if remaining > 0 {
        summary.failed += 1;
    }
    Ok(())
}
