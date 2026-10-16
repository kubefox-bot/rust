use super::*;
pub(super) async fn run<R: TreeRepository, P: Fn(Progress) + Send + Sync>(
    repository: &mut R,
    rows: &[StoredMessage],
    progress: &P,
    summary: &mut Summary,
) -> Result<(), CleanupError> {
    if rows.is_empty() {
        return Ok(());
    }
    workflow::emit(Stage::Database, *summary, progress);
    let deleted = match workflow::at(
        repository.delete_stored(rows).await,
        Stage::Database,
        *summary,
    )? {
        DeleteResult::Committed(rows) => rows,
        DeleteResult::CommitUncertain { reason } => {
            return workflow::at(
                Err(format!("Подтверждение COMMIT потеряно; повторите очистку: {reason}").into()),
                Stage::Database,
                *summary,
            )
        }
    };
    workflow::at(
        if deleted.messages == rows.len() as u64 {
            Ok(())
        } else {
            Err("Неожиданное количество удалённых строк".into())
        },
        Stage::Database,
        *summary,
    )?;
    workflow::emit(Stage::Verification, *summary, progress);
    workflow::at(
        repository
            .verify_deleted(&rows.iter().map(|r| r.candidate.id).collect::<Vec<_>>())
            .await,
        Stage::Verification,
        *summary,
    )?;
    summary.deleted_messages += deleted.messages;
    summary.deleted_metadata += deleted.metadata;
    summary.deleted_recipients += deleted.recipients;
    workflow::emit(Stage::Database, *summary, progress);
    Ok(())
}
