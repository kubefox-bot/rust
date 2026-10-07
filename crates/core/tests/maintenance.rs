use hmail_cleaner_core::{
    cleanup::*,
    maintenance::{self, *},
};
use std::{
    collections::HashSet,
    num::NonZeroU32,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime},
};
use uuid::Uuid;
#[derive(Default)]
struct State {
    trace: Vec<&'static str>,
    metadata: bool,
    recipients: bool,
    files: Vec<FileObservation>,
    refs: FileReferences,
    fail: Option<&'static str>,
    uncertain: bool,
    bad_outcome: bool,
    closed: bool,
    cancel_after_commit: Option<Arc<Cancellation>>,
}
type Shared = Arc<Mutex<State>>;
struct Repo(Shared);
struct Files(Shared);
struct Scan(Shared);
struct Guard;
impl OfflineGuard for Guard {
    async fn ensure_stopped(&self) -> PortResult<()> {
        Ok(())
    }
}
impl Repository for Repo {
    async fn snapshot(&mut self) -> PortResult<Snapshot> {
        self.0.lock().unwrap().trace.push("messages");
        Ok(Snapshot { upper_id: 0 })
    }
    async fn next_batch(&mut self, _: Selection, c: i64, _: u32) -> PortResult<Batch> {
        Ok(Batch {
            last_id: c,
            candidates: vec![],
            rejected: vec![],
        })
    }
    async fn delete_batch(&mut self, _: &[Candidate]) -> PortResult<DeleteResult> {
        panic!("no messages")
    }
    async fn verify_deleted(&mut self, _: &[i64]) -> PortResult<()> {
        Ok(())
    }
}
impl FullRepository for Repo {
    async fn orphan_upper_id(&mut self, t: OrphanTable) -> PortResult<i64> {
        self.0.lock().unwrap().trace.push(match t {
            OrphanTable::Metadata => "metadata",
            OrphanTable::Recipients => "recipients",
        });
        Ok(1)
    }
    async fn orphan_batch(
        &mut self,
        t: OrphanSelection,
        c: i64,
        _: u32,
    ) -> PortResult<OrphanBatch> {
        let s = self.0.lock().unwrap();
        let present = match t.table {
            OrphanTable::Metadata => s.metadata,
            OrphanTable::Recipients => s.recipients,
        };
        Ok(OrphanBatch {
            last_id: if c == 0 && present { 1 } else { c },
            rows: if c == 0 && present {
                vec![OrphanRow {
                    id: 1,
                    parent_message_id: 99,
                }]
            } else {
                vec![]
            },
        })
    }
    async fn delete_orphans(
        &mut self,
        t: OrphanTable,
        _: &[OrphanRow],
    ) -> PortResult<DeleteResult> {
        let mut s = self.0.lock().unwrap();
        if s.uncertain {
            return Ok(DeleteResult::CommitUncertain {
                reason: "lost ack".into(),
            });
        }
        match t {
            OrphanTable::Metadata => s.metadata = false,
            OrphanTable::Recipients => s.recipients = false,
        };
        if let Some(cancel) = &s.cancel_after_commit {
            cancel.request();
        }
        Ok(DeleteResult::Committed(DeletedRows {
            metadata: u64::from(t == OrphanTable::Metadata),
            recipients: u64::from(t == OrphanTable::Recipients),
            messages: 0,
        }))
    }
    async fn verify_orphans_deleted(&mut self, _: OrphanTable, _: &[i64]) -> PortResult<()> {
        Ok(())
    }
    async fn file_references(&mut self, _: usize) -> PortResult<FileReferences> {
        let mut s = self.0.lock().unwrap();
        s.trace.push("references");
        if s.fail == Some("references") {
            return Err("references unavailable".into());
        }
        Ok(s.refs.clone())
    }
}
impl FileStore for Files {
    async fn delete(&self, _: Vec<FileTarget>, _: u8) -> PortResult<Vec<FileOutcome>> {
        panic!("no messages")
    }
    async fn prune_empty(&self, _: Vec<PathBuf>) -> PortResult<PruneOutcome> {
        self.0.lock().unwrap().trace.push("prune");
        Ok(PruneOutcome::default())
    }
}
impl FullFileStore for Files {
    type Scan = Scan;
    async fn validate_tree(&self) -> PortResult<ScanStats> {
        let mut s = self.0.lock().unwrap();
        s.trace.push("precheck");
        if s.fail == Some("precheck") {
            return Err("precheck denied".into());
        }
        Ok(ScanStats {
            examined_files: s.files.len() as u64,
        })
    }
    async fn open_scan(&self) -> PortResult<Scan> {
        self.0.lock().unwrap().trace.push("scan");
        Ok(Scan(self.0.clone()))
    }
    async fn delete_orphans(
        &self,
        files: Vec<FileObservation>,
        _: u8,
    ) -> PortResult<Vec<OrphanFileOutcome>> {
        let mut s = self.0.lock().unwrap();
        s.trace.push("delete files");
        let bad = s.bad_outcome;
        Ok(files
            .into_iter()
            .map(|mut observation| {
                s.files.retain(|f| f != &observation);
                if bad {
                    observation.path = PathBuf::from("/unexpected")
                };
                OrphanFileOutcome {
                    observation,
                    result: DeletionResult::Removed { size_bytes: 7 },
                }
            })
            .collect())
    }
}
impl FileScan for Scan {
    async fn next_batch(&mut self, _: u32) -> PortResult<ScanBatch> {
        let s = self.0.lock().unwrap();
        if s.fail == Some("scan") {
            return Err("scan failed".into());
        }
        Ok(ScanBatch {
            visited_entries: s.files.len() as u64,
            examined_files: s.files.len() as u64,
            files: s.files.clone(),
            finished: true,
        })
    }
    async fn close(self) -> PortResult<()> {
        let mut s = self.0.lock().unwrap();
        s.closed = true;
        if s.fail == Some("scan") {
            return Err("close also failed".into());
        }
        Ok(())
    }
}
fn observation(id: u128, age: u64) -> FileObservation {
    FileObservation {
        path: PathBuf::from(format!("/Data/domain/account/00/{{{:036}}}.eml", id)),
        guid: Uuid::from_u128(id),
        size_bytes: 7,
        modified_at: SystemTime::UNIX_EPOCH + Duration::from_secs(age),
    }
}
fn setup() -> Shared {
    Arc::new(Mutex::new(State {
        metadata: true,
        recipients: true,
        files: vec![observation(1, 10), observation(2, 100)],
        ..State::default()
    }))
}
async fn run(
    state: &Shared,
    is_preview: bool,
    scan: ScanOptions,
    cancel: &Cancellation,
) -> Result<Summary, CleanupError> {
    let mut repo = Repo(state.clone());
    let files = Files(state.clone());
    let options = RunOptions {
        cutoff: chrono::NaiveDate::from_ymd_opt(2026, 9, 7)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap(),
        batch_size: NonZeroU32::new(1000).unwrap(),
        workers: 2,
        data_directory: PathBuf::from("/Data"),
    };
    let deps = Dependencies {
        repository: &mut repo,
        files: &files,
        guard: &Guard,
    };
    if is_preview {
        maintenance::preview(deps, &options, scan, cancel, |_| {}).await
    } else {
        maintenance::clean(deps, &options, scan, cancel, |_| {}).await
    }
}
fn full() -> ScanOptions {
    ScanOptions::Full {
        file_cutoff: SystemTime::UNIX_EPOCH + Duration::from_secs(50),
    }
}
#[tokio::test]
async fn cancel_finishes_current_orphan_batch() {
    let s = setup();
    let cancel = Arc::new(Cancellation::default());
    s.lock().unwrap().cancel_after_commit = Some(cancel.clone());
    let error = run(&s, false, full(), &cancel).await.unwrap_err();
    assert_eq!(error.stage, Stage::Cancelled);
    assert_eq!(error.summary.orphan_metadata, 1);
    let s = s.lock().unwrap();
    assert!(!s.metadata);
    assert!(s.recipients);
    assert!(!s.trace.contains(&"references"));
}
#[tokio::test]
async fn fast_never_opens_scan() {
    let s = setup();
    run(&s, false, ScanOptions::Fast, &Cancellation::default())
        .await
        .unwrap();
    assert_eq!(s.lock().unwrap().trace, vec!["messages"]);
}
#[tokio::test]
async fn full_orders_sql_then_references_then_files() {
    let s = setup();
    let summary = run(&s, false, full(), &Cancellation::default())
        .await
        .unwrap();
    assert_eq!((summary.orphan_metadata, summary.orphan_recipients), (1, 1));
    assert_eq!(
        (summary.selected_orphan_files, summary.removed_orphan_files),
        (1, 1)
    );
    let s = s.lock().unwrap();
    assert!(!s.metadata && !s.recipients);
    assert_eq!(s.files, vec![observation(2, 100)]);
    assert_eq!(
        s.trace,
        vec![
            "messages",
            "metadata",
            "recipients",
            "references",
            "precheck",
            "scan",
            "delete files",
            "prune"
        ]
    );
    assert!(s.closed);
}
#[tokio::test]
async fn failed_reference_load_prevents_orphan_file_removal() {
    let s = setup();
    s.lock().unwrap().fail = Some("references");
    assert!(run(&s, false, full(), &Cancellation::default())
        .await
        .is_err());
    let s = s.lock().unwrap();
    assert_eq!(s.files.len(), 2);
    assert!(!s.trace.contains(&"scan"));
}
#[tokio::test]
async fn failed_tree_precheck_prevents_removal() {
    let s = setup();
    s.lock().unwrap().fail = Some("precheck");
    assert!(run(&s, false, full(), &Cancellation::default())
        .await
        .is_err());
    assert_eq!(s.lock().unwrap().files.len(), 2);
}
#[tokio::test]
async fn new_queue_and_public_references_protect_files() {
    let s = setup();
    s.lock().unwrap().refs = HashSet::from([Uuid::from_u128(1)]);
    let sum = run(&s, false, full(), &Cancellation::default())
        .await
        .unwrap();
    assert_eq!(sum.removed_orphan_files, 0);
    assert_eq!(s.lock().unwrap().files.len(), 2);
}
#[tokio::test]
async fn full_preview_never_deletes() {
    let s = setup();
    let sum = run(&s, true, full(), &Cancellation::default())
        .await
        .unwrap();
    assert_eq!(
        (
            sum.orphan_metadata,
            sum.orphan_recipients,
            sum.selected_orphan_files
        ),
        (1, 1, 1)
    );
    assert_eq!(sum.removed_files, 0);
    let s = s.lock().unwrap();
    assert!(s.metadata && s.recipients);
    assert_eq!(s.files.len(), 2);
    assert!(!s.trace.contains(&"delete files"));
}
#[tokio::test]
async fn commit_uncertain_prevents_later_phases() {
    let s = setup();
    s.lock().unwrap().uncertain = true;
    let err = run(&s, false, full(), &Cancellation::default())
        .await
        .unwrap_err();
    assert_eq!(err.summary.orphan_metadata, 0);
    assert!(!s.lock().unwrap().trace.contains(&"references"));
}
#[tokio::test]
async fn scan_close_failure_preserves_primary_error() {
    let s = setup();
    s.lock().unwrap().fail = Some("scan");
    let err = run(&s, false, full(), &Cancellation::default())
        .await
        .unwrap_err();
    assert!(err.to_string().contains("scan failed"));
    assert!(err.to_string().contains("close also failed"));
    assert!(s.lock().unwrap().closed);
}
#[tokio::test]
async fn unexpected_orphan_outcome_blocks_pruning() {
    let s = setup();
    s.lock().unwrap().bad_outcome = true;
    let err = run(&s, false, full(), &Cancellation::default())
        .await
        .unwrap_err();
    assert_eq!(err.summary.removed_orphan_files, 0);
    assert!(!s.lock().unwrap().trace.contains(&"prune"));
    assert!(s.lock().unwrap().closed);
}
