pub(crate) const JOURNAL_FILE: &str = "agent.ndjson";
use hmail_cleaner_adapters::journal::RuntimeJournal;
use hmail_cleaner_core::cleanup::{Failure, Stage, Summary};
use hmail_cleaner_protocol::{JobId, JobPhase, ObservedMailState};
use hmail_cleaner_runner::Status;
use serde::Serialize;
#[derive(Serialize)]
#[serde(tag = "code", rename_all = "snake_case")]
pub(crate) enum Event {
    ServiceStarting,
    ServiceStopped,
    ServiceFailed,
    JobStarted {
        job_id: JobId,
    },
    StageChanged {
        job_id: JobId,
        stage: Stage,
    },
    CleanupFailure {
        job_id: JobId,
        stage: Stage,
        failure: FailureCode,
        message_id: Option<i64>,
        failed: u64,
    },
    MailState {
        job_id: JobId,
        state: ObservedMailState,
    },
    ExecutionResult {
        job_id: JobId,
        status: Status,
        stage: Option<Stage>,
        summary: Summary,
        fatal: bool,
    },
    ReceiptPersisted {
        job_id: JobId,
        phase: JobPhase,
    },
    ReceiptFailed {
        job_id: JobId,
    },
}
#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum FailureCode {
    Message,
    Directory,
    OrphanFile,
}
pub(super) fn failure(value: &Failure) -> (FailureCode, Option<i64>) {
    match value {
        Failure::Message { id, .. } => (FailureCode::Message, Some(*id)),
        Failure::Directory { .. } => (FailureCode::Directory, None),
        Failure::OrphanFile { .. } => (FailureCode::OrphanFile, None),
    }
}
pub(crate) fn record(journal: Option<&RuntimeJournal>, event: Event) {
    if let Some(journal) = journal {
        let _ = journal.record(&event);
    }
}
