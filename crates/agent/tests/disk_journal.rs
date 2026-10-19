#[path = "fixtures/bootstrap.rs"]
mod bootstrap;
use hmail_cleaner_agent::config::AgentConfig;
use hmail_cleaner_core::cleanup::Cancellation;
use std::sync::Arc;

#[tokio::test]
async fn unusable_journal_fails_startup_before_opening_the_ledger() {
    let root = tempfile::tempdir().unwrap();
    let (path, _, _) = bootstrap::signed(root.path());
    let mut config = AgentConfig::load(&path, "fixture").unwrap();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    config.listen = listener.local_addr().unwrap();
    std::fs::create_dir(config.data_directory.join("agent.ndjson")).unwrap();
    let database = config.data_directory.join("jobs.sqlite");
    let cancel = Arc::new(Cancellation::default());
    cancel.request();
    let result = hmail_cleaner_agent::run_bound(config, cancel, listener).await;
    assert!(
        result.is_err(),
        "the service must reject an unusable disk journal"
    );
    assert!(
        !database.exists(),
        "journal startup must precede ledger opening"
    );
}

#[path = "fixtures/journal_cleanup.rs"]
mod cleanup;
use hmail_cleaner_adapters::journal::RuntimeJournal;
use hmail_cleaner_agent::{execution::Worker, jobs::JobRepository};
use hmail_cleaner_protocol::{AgentId, ErrorCode, JobPhase, ObservedMailState};
use std::{
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};
#[derive(Clone, Copy)]
enum JournalMode {
    Healthy,
    RotationFault,
}
async fn cleanup_case(mode: JournalMode, restore: cleanup::Restore) {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("agent.ndjson");
    if matches!(mode, JournalMode::RotationFault) {
        std::fs::File::create(&path)
            .unwrap()
            .set_len(10 * 1024 * 1024)
            .unwrap();
    }
    let journal = RuntimeJournal::open(&path).unwrap();
    if matches!(mode, JournalMode::RotationFault) {
        std::fs::create_dir(root.path().join("agent.ndjson.4")).unwrap();
    }
    let repo = Arc::new(
        JobRepository::open(&root.path().join("jobs.sqlite"))
            .await
            .unwrap(),
    );
    let agent = AgentId::new();
    let restorations = Arc::new(AtomicUsize::new(0));
    let worker = Worker::new(
        agent,
        repo.clone(),
        cleanup::Cleanup {
            restorations: restorations.clone(),
            restore,
        },
    )
    .with_journal(journal.clone())
    .unwrap();
    let mut command: hmail_cleaner_protocol::JobCommand =
        serde_json::from_str(include_str!("../../protocol/tests/fixtures/job-v1.json")).unwrap();
    command.agent_id = agent;
    let job = hmail_cleaner_protocol::validate_job(command).unwrap();
    let id = job.command().job_id;
    worker.submit(job).await.unwrap();
    let snapshot = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let snapshot = repo.lookup(id).await.unwrap().unwrap();
            if !snapshot.phase.is_active() {
                break snapshot;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    if matches!(mode, JournalMode::RotationFault) {
        assert!(!worker.is_healthy());
    }
    worker.shutdown().await;
    assert_eq!(snapshot.phase, JobPhase::Failed);
    assert_eq!(snapshot.summary.failed.get(), 1);
    assert_eq!(restorations.load(Ordering::SeqCst), 1);
    match restore {
        cleanup::Restore::Succeeds => {
            assert_eq!(snapshot.mail_state, ObservedMailState::Running);
            assert_eq!(snapshot.error.unwrap().code(), ErrorCode::CleanupFailed);
        }
        cleanup::Restore::Fails => {
            assert_eq!(snapshot.mail_state, ObservedMailState::Unknown);
            assert_eq!(snapshot.error.unwrap().code(), ErrorCode::RestoreFailed);
        }
    }
    drop(worker);
    drop(repo);
    let reopened = JobRepository::open(&root.path().join("jobs.sqlite"))
        .await
        .unwrap();
    assert_eq!(
        reopened.lookup(id).await.unwrap().unwrap().phase,
        JobPhase::Failed
    );
    if matches!(mode, JournalMode::RotationFault) {
        assert!(!journal.is_healthy());
        assert_eq!(std::fs::metadata(path).unwrap().len(), 10 * 1024 * 1024);
        return;
    }
    assert!(journal.is_healthy());
    let text = std::fs::read_to_string(path).unwrap();
    assert!(!text.contains("CANARY"));
    assert!(!text.contains("mysql://"));
    assert!(!text.contains("password"));
    let events = text
        .lines()
        .map(|line| {
            assert!(line.len() < 2048);
            serde_json::from_str::<serde_json::Value>(line).unwrap()["event"].clone()
        })
        .collect::<Vec<_>>();
    assert!(events.iter().all(|event| event["job_id"] == id.to_string()));
    let failures = events
        .iter()
        .filter(|event| event["code"] == "cleanup_failure")
        .collect::<Vec<_>>();
    assert_eq!(
        failures.len(),
        1,
        "safe failure must survive the subsequent coalesced counter update"
    );
    assert_eq!(failures[0]["stage"], "selection");
    assert_eq!(failures[0]["failure"], "message");
    assert_eq!(failures[0]["message_id"], 1);
    assert_eq!(failures[0]["failed"], 1);
    assert_eq!(
        events
            .iter()
            .filter(|event| event["code"] == "stage_changed")
            .count(),
        1,
        "per-file counters must not flood the disk journal"
    );
    let result = events
        .iter()
        .find(|event| event["code"] == "execution_result")
        .unwrap();
    assert_eq!(result["status"], "failed");
    assert_eq!(result["stage"], "selection");
    assert!(events
        .iter()
        .any(|event| event["code"] == "mail_state" && event["state"] == "restoring"));
    assert_eq!(events.last().unwrap()["code"], "receipt_persisted");
}
#[tokio::test]
async fn cleanup_failure_survives_counter_coalescing_and_keeps_safe_typed_identity() {
    cleanup_case(JournalMode::Healthy, cleanup::Restore::Succeeds).await;
}
#[tokio::test]
async fn failed_restore_keeps_original_cleanup_diagnostic_and_durable_receipt() {
    cleanup_case(JournalMode::Healthy, cleanup::Restore::Fails).await;
}
#[tokio::test]
async fn runtime_write_failure_never_bypasses_restoration_or_terminal_receipt() {
    cleanup_case(JournalMode::RotationFault, cleanup::Restore::Succeeds).await;
}
#[tokio::test]
async fn production_agent_owns_appendable_journal_across_restart() {
    let root = tempfile::tempdir().unwrap();
    let (path, _, _) = bootstrap::signed(root.path());
    for _ in 0..2 {
        let mut config = AgentConfig::load(&path, "fixture").unwrap();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        config.listen = listener.local_addr().unwrap();
        let cancel = Arc::new(Cancellation::default());
        cancel.request();
        tokio::time::timeout(
            Duration::from_secs(5),
            hmail_cleaner_agent::run_bound(config, cancel, listener),
        )
        .await
        .unwrap()
        .unwrap();
    }
    let text = std::fs::read_to_string(path.parent().unwrap().join("agent.ndjson")).unwrap();
    assert_eq!(text.lines().count(), 4);
    assert_eq!(text.matches("service_starting").count(), 2);
    assert_eq!(text.matches("service_stopped").count(), 2);
    assert!(!text.contains("PRIVATE KEY"));
}
