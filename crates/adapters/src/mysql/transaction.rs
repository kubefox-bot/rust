use super::RepositoryError;
use hmail_cleaner_core::cleanup::{DeleteResult, DeletedRows, PortResult};
use sqlx::{MySql, Transaction};
use std::error::Error;

pub(super) fn commit_result(result: Result<(), sqlx::Error>, rows: DeletedRows) -> DeleteResult {
    match result {
        Ok(()) => DeleteResult::Committed(rows),
        Err(error) => DeleteResult::CommitUncertain {
            reason: error.to_string(),
        },
    }
}
pub(super) async fn commit(transaction: Transaction<'_, MySql>, rows: DeletedRows) -> DeleteResult {
    commit_result(transaction.commit().await, rows)
}
pub(super) async fn rollback(
    transaction: Transaction<'_, MySql>,
    original: Box<dyn Error + Send + Sync>,
) -> PortResult<DeleteResult> {
    match transaction.rollback().await {
        Ok(()) => Err(original),
        Err(rollback) => Err(RepositoryError::Rollback {
            original: original.to_string(),
            rollback,
        }
        .into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmail_cleaner_core::cleanup::{DeleteResult, DeletedRows};
    #[test]
    fn lost_commit_acknowledgement_is_not_confirmed_counts() {
        let error = sqlx::Error::Io(std::io::Error::new(
            std::io::ErrorKind::ConnectionReset,
            "fixture connection reset",
        ));
        let outcome = commit_result(
            Err(error),
            DeletedRows {
                messages: 0,
                metadata: 7,
                recipients: 0,
            },
        );
        assert!(
            matches!(outcome, DeleteResult::CommitUncertain { reason } if reason.contains("fixture connection reset"))
        );
    }
}
