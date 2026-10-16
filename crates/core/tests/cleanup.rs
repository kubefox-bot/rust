use chrono::NaiveDateTime;
use hmail_cleaner_core::cleanup::*;
use std::{
    collections::{BTreeMap, HashSet},
    num::NonZeroU32,
    path::PathBuf,
    sync::{Arc, Mutex},
};

#[derive(Debug, PartialEq, Eq)]
enum Event {
    Guard,
    Snapshot,
    Select(i64),
    Files(Vec<i64>),
    Delete(Vec<i64>),
    Verify(Vec<i64>),
    Prune,
}
type Events = Arc<Mutex<Vec<Event>>>;
fn date(value: &str) -> NaiveDateTime {
    NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S").unwrap()
}
fn candidate(id: i64) -> Candidate {
    Candidate {
        id,
        account_id: 1,
        folder_id: 10,
        storage: MessageStorage::Account {
            address: "info@chem.orp".into(),
        },
        filename: format!("{{AB000000-0000-0000-0000-{id:012X}}}.eml"),
        created_at: date("2026-01-01 12:00:00"),
    }
}
fn options() -> RunOptions {
    RunOptions {
        cutoff: date("2026-09-07 12:00:00"),
        file_cutoff: std::time::SystemTime::now(),
        batch_size: NonZeroU32::new(2).unwrap(),
        workers: 2,
        data_directory: PathBuf::from("/Data"),
    }
}
struct Repo {
    rows: BTreeMap<i64, Candidate>,
    events: Events,
    fail_next: bool,
    uncertain_next: bool,
    bad_count: bool,
    rejected: HashSet<i64>,
}
impl Repository for Repo {
    async fn snapshot(&mut self) -> PortResult<Snapshot> {
        self.events.lock().unwrap().push(Event::Snapshot);
        Ok(Snapshot {
            upper_id: *self.rows.keys().next_back().unwrap_or(&0),
        })
    }
    async fn next_batch(
        &mut self,
        selection: Selection,
        cursor: i64,
        limit: u32,
    ) -> PortResult<Batch> {
        self.events.lock().unwrap().push(Event::Select(cursor));
        let mut candidates = Vec::new();
        let mut rejected = Vec::new();
        let mut last_id = cursor;
        for row in self
            .rows
            .values()
            .filter(|row| row.id > cursor && row.id <= selection.upper_id)
            .take(limit as usize)
        {
            last_id = row.id;
            if self.rejected.contains(&row.id) {
                rejected.push(RowFailure {
                    id: row.id,
                    reason: "bad row".into(),
                });
            } else {
                candidates.push(row.clone());
            }
        }
        Ok(Batch {
            last_id,
            candidates,
            rejected,
        })
    }
    async fn delete_batch(&mut self, rows: &[Candidate]) -> PortResult<DeleteResult> {
        self.events
            .lock()
            .unwrap()
            .push(Event::Delete(rows.iter().map(|row| row.id).collect()));
        if self.fail_next {
            self.fail_next = false;
            return Err("SQL fixture failure".into());
        }
        for row in rows {
            self.rows.remove(&row.id);
        }
        if self.uncertain_next {
            self.uncertain_next = false;
            return Ok(DeleteResult::CommitUncertain {
                reason: "lost acknowledgement".into(),
            });
        }
        Ok(DeleteResult::Committed(DeletedRows {
            messages: if self.bad_count { 0 } else { rows.len() as u64 },
            metadata: rows.len() as u64,
            recipients: rows.len() as u64,
        }))
    }
    async fn verify_deleted(&mut self, ids: &[i64]) -> PortResult<()> {
        self.events
            .lock()
            .unwrap()
            .push(Event::Verify(ids.to_vec()));
        assert!(ids.iter().all(|id| !self.rows.contains_key(id)));
        Ok(())
    }
}
struct Files {
    events: Events,
    absent: Mutex<HashSet<i64>>,
    failures: HashSet<i64>,
    cancel: Option<Arc<Cancellation>>,
    created_at: BTreeMap<i64, std::time::SystemTime>,
    inspection_failures: HashSet<i64>,
    incomplete_inspection: bool,
}
impl FileStore for Files {
    async fn inspect(&self, files: Vec<FileTarget>, _: u8) -> PortResult<Vec<FileInspection>> {
        let mut observed = files
            .into_iter()
            .map(|file| FileInspection {
                id: file.id,
                path: file.path,
                result: if self.inspection_failures.contains(&file.id) {
                    Err(FileFailure {
                        kind: FileFailureKind::Io,
                        message: "CreationTime unavailable".into(),
                    })
                } else {
                    Ok(if self.absent.lock().unwrap().contains(&file.id) {
                        None
                    } else {
                        Some(FileMetadata {
                            created_at: self
                                .created_at
                                .get(&file.id)
                                .copied()
                                .unwrap_or(std::time::SystemTime::UNIX_EPOCH),
                            modified_at: std::time::SystemTime::UNIX_EPOCH,
                            size_bytes: 7,
                        })
                    })
                },
            })
            .collect::<Vec<_>>();
        if self.incomplete_inspection {
            observed.pop();
        }
        Ok(observed)
    }
    async fn delete(
        &self,
        files: Vec<FileInspection>,
        _workers: u8,
    ) -> PortResult<Vec<FileOutcome>> {
        self.events
            .lock()
            .unwrap()
            .push(Event::Files(files.iter().map(|file| file.id).collect()));
        if let Some(cancel) = &self.cancel {
            cancel.request();
        }
        Ok(files
            .into_iter()
            .map(|file| {
                let result = if self.failures.contains(&file.id) {
                    DeletionResult::Failed(FileFailure {
                        kind: FileFailureKind::AccessDenied,
                        message: "denied".into(),
                    })
                } else if !self.absent.lock().unwrap().insert(file.id) {
                    DeletionResult::AlreadyAbsent
                } else {
                    DeletionResult::Removed { size_bytes: 7 }
                };
                FileOutcome {
                    id: file.id,
                    path: file.path,
                    result,
                }
            })
            .collect())
    }
    async fn prune_empty(&self, _parents: Vec<PathBuf>) -> PortResult<PruneOutcome> {
        self.events.lock().unwrap().push(Event::Prune);
        Ok(PruneOutcome {
            removed: 1,
            failures: Vec::new(),
        })
    }
}
struct Guard(Events);
impl OfflineGuard for Guard {
    async fn ensure_stopped(&self) -> PortResult<()> {
        self.0.lock().unwrap().push(Event::Guard);
        Ok(())
    }
}
struct Scenario {
    repository: Repo,
    files: Files,
    guard: Guard,
    events: Events,
}
impl Scenario {
    fn new(count: i64) -> Self {
        let events = Arc::new(Mutex::new(Vec::new()));
        Self {
            repository: Repo {
                rows: (1..=count).map(|id| (id, candidate(id))).collect(),
                events: events.clone(),
                fail_next: false,
                uncertain_next: false,
                bad_count: false,
                rejected: HashSet::new(),
            },
            files: Files {
                events: events.clone(),
                absent: Mutex::new(HashSet::new()),
                failures: HashSet::new(),
                cancel: None,
                created_at: BTreeMap::new(),
                inspection_failures: HashSet::new(),
                incomplete_inspection: false,
            },
            guard: Guard(events.clone()),
            events,
        }
    }
    async fn run(&mut self, cancel: &Cancellation) -> Result<Summary, CleanupError> {
        clean(
            Dependencies {
                repository: &mut self.repository,
                files: &self.files,
                guard: &self.guard,
            },
            &options(),
            cancel,
            |_| {},
        )
        .await
    }
}
#[tokio::test]
async fn file_step_precedes_database_delete() {
    let mut s = Scenario::new(2);
    let summary = s.run(&Cancellation::default()).await.unwrap();
    assert_eq!(summary.deleted_messages, 2);
    let events = s.events.lock().unwrap();
    assert_eq!(
        &events[..8],
        &[
            Event::Guard,
            Event::Snapshot,
            Event::Select(0),
            Event::Guard,
            Event::Files(vec![1, 2]),
            Event::Delete(vec![1, 2]),
            Event::Verify(vec![1, 2]),
            Event::Prune
        ]
    );
}
#[tokio::test]
async fn absent_file_contributes_no_bytes() {
    let mut s = Scenario::new(1);
    s.files.absent.lock().unwrap().insert(1);
    let summary = s.run(&Cancellation::default()).await.unwrap();
    assert_eq!(summary.removed_bytes, 0);
    assert_eq!(summary.absent_files, 1);
}
#[tokio::test]
async fn verification_error_preserves_message() {
    let mut s = Scenario::new(1);
    s.files.failures.insert(1);
    let summary = s.run(&Cancellation::default()).await.unwrap();
    assert_eq!(summary.deleted_messages, 0);
    assert_eq!(summary.removed_bytes, 0);
    assert!(s.repository.rows.contains_key(&1));
}
#[tokio::test]
async fn confirmed_files_and_sql_rows_have_separate_counters() {
    let mut s = Scenario::new(1);
    let summary = s.run(&Cancellation::default()).await.unwrap();
    assert_eq!(summary.removed_bytes, 7);
    assert_eq!(summary.deleted_metadata, 1);
    assert_eq!(summary.deleted_recipients, 1);
}
#[tokio::test]
async fn unexpected_commit_count_is_not_verified_success() {
    let mut s = Scenario::new(1);
    s.repository.bad_count = true;
    let error = s.run(&Cancellation::default()).await.unwrap_err();
    assert_eq!(error.summary.deleted_messages, 0);
    assert_eq!(error.summary.removed_bytes, 7);
    assert!(!s.events.lock().unwrap().contains(&Event::Prune));
}
#[tokio::test]
async fn removed_and_absent_ids_are_deleted_but_failed_ids_are_preserved() {
    let mut s = Scenario::new(3);
    s.files.absent.lock().unwrap().insert(2);
    s.files.failures.insert(3);
    let summary = s.run(&Cancellation::default()).await.unwrap();
    assert_eq!(summary.removed_files, 1);
    assert_eq!(summary.absent_files, 1);
    assert_eq!(summary.deleted_messages, 2);
    assert_eq!(summary.failed, 1);
    assert_eq!(
        s.repository.rows.keys().copied().collect::<Vec<_>>(),
        vec![3]
    );
}
#[tokio::test]
async fn repeated_run_cleans_rows_after_previous_sql_failure() {
    let mut s = Scenario::new(2);
    s.repository.fail_next = true;
    let error = s.run(&Cancellation::default()).await.unwrap_err();
    assert_eq!(error.summary.removed_files, 2);
    assert_eq!(s.repository.rows.len(), 2);
    let summary = s.run(&Cancellation::default()).await.unwrap();
    assert_eq!(summary.absent_files, 2);
    assert_eq!(summary.deleted_messages, 2);
    assert!(s.repository.rows.is_empty());
}
#[tokio::test]
async fn uncertain_commit_stops_new_batches() {
    let mut s = Scenario::new(4);
    s.repository.uncertain_next = true;
    let error = s.run(&Cancellation::default()).await.unwrap_err();
    assert_eq!(error.summary.deleted_messages, 0);
    assert_eq!(
        s.repository.rows.keys().copied().collect::<Vec<_>>(),
        vec![3, 4]
    );
    assert!(!s.events.lock().unwrap().contains(&Event::Select(2)));
    assert!(!s.events.lock().unwrap().contains(&Event::Prune));
}
#[tokio::test]
async fn rejected_rows_advance_cursor() {
    let mut s = Scenario::new(4);
    s.repository.rejected.extend([1, 2]);
    let summary = s.run(&Cancellation::default()).await.unwrap();
    assert_eq!(summary.failed, 2);
    assert_eq!(summary.deleted_messages, 2);
    assert!(s.events.lock().unwrap().contains(&Event::Select(2)));
}
#[tokio::test]
async fn cancel_finishes_started_batch() {
    let mut s = Scenario::new(4);
    let cancel = Arc::new(Cancellation::default());
    s.files.cancel = Some(cancel.clone());
    let error = s.run(&cancel).await.unwrap_err();
    assert!(error.is_cancelled());
    assert_eq!(error.summary.deleted_messages, 2);
    assert!(s.events.lock().unwrap().contains(&Event::Prune));
    assert!(!s.events.lock().unwrap().contains(&Event::Select(2)));
}
#[tokio::test]
async fn preview_has_no_delete_calls() {
    let mut s = Scenario::new(3);
    let summary = preview(&mut s.repository, &s.files, &options(), |_| {})
        .await
        .unwrap();
    assert_eq!(summary.selected, 3);
    assert_eq!(summary.deleted_messages, 0);
    assert_eq!(s.repository.rows.len(), 3);
    assert!(s
        .events
        .lock()
        .unwrap()
        .iter()
        .all(|event| matches!(event, Event::Snapshot | Event::Select(_))));
}
#[tokio::test]
async fn prunes_only_after_confirmed_database_deletion() {
    let mut s = Scenario::new(2);
    s.run(&Cancellation::default()).await.unwrap();
    let events = s.events.lock().unwrap();
    let verify = events
        .iter()
        .position(|event| matches!(event, Event::Verify(_)))
        .unwrap();
    let prune = events
        .iter()
        .position(|event| matches!(event, Event::Prune))
        .unwrap();
    assert!(prune > verify);
}
#[tokio::test]
async fn failed_database_step_does_not_prune() {
    let mut s = Scenario::new(2);
    s.repository.fail_next = true;
    let error = s.run(&Cancellation::default()).await.unwrap_err();
    assert_eq!(error.stage, Stage::Database);
    assert!(!s.events.lock().unwrap().contains(&Event::Prune));
}
#[tokio::test]
async fn invalid_paths_are_reported_and_preserved() {
    let mut s = Scenario::new(1);
    s.repository.rows.get_mut(&1).unwrap().filename = "../bad.eml".into();
    let summary = s.run(&Cancellation::default()).await.unwrap();
    assert_eq!(summary.failed, 1);
    assert_eq!(summary.deleted_messages, 0);
    assert_eq!(s.repository.rows.len(), 1);
    assert!(!s
        .events
        .lock()
        .unwrap()
        .iter()
        .any(|event| matches!(event, Event::Files(_) | Event::Delete(_) | Event::Prune)));
}

#[tokio::test]
async fn creation_time_overrides_database_date_in_both_directions() {
    let mut s = Scenario::new(2);
    s.repository.rows.get_mut(&1).unwrap().created_at = date("2026-10-01 12:00:00");
    s.files.created_at.insert(
        2,
        options().file_cutoff + std::time::Duration::from_secs(3600),
    );
    let planned = preview(&mut s.repository, &s.files, &options(), |_| {})
        .await
        .unwrap();
    assert_eq!(planned.selected, 1);
    assert_eq!(planned.examined_messages, 2);
    assert_eq!(planned.removed_files, 0);
    let summary = s.run(&Cancellation::default()).await.unwrap();
    assert_eq!(summary.selected, 1);
    assert_eq!(summary.deleted_messages, 1);
    assert_eq!(
        s.repository.rows.keys().copied().collect::<Vec<_>>(),
        vec![2]
    );
}

#[tokio::test]
async fn missing_files_use_database_age_without_removing_fresh_records() {
    let mut s = Scenario::new(2);
    s.files.absent.lock().unwrap().extend([1, 2]);
    s.repository.rows.get_mut(&2).unwrap().created_at = options().cutoff;
    let summary = s.run(&Cancellation::default()).await.unwrap();
    assert_eq!(summary.selected, 1);
    assert_eq!(summary.absent_files, 1);
    assert_eq!(summary.deleted_messages, 1);
    assert!(s.repository.rows.contains_key(&2));
}

#[tokio::test]
async fn failed_creation_time_inspection_preserves_the_message() {
    let mut s = Scenario::new(1);
    s.files.inspection_failures.insert(1);
    let summary = s.run(&Cancellation::default()).await.unwrap();
    assert_eq!(summary.failed, 1);
    assert_eq!(summary.deleted_messages, 0);
    assert!(s.repository.rows.contains_key(&1));
    assert!(!s
        .events
        .lock()
        .unwrap()
        .iter()
        .any(|event| matches!(event, Event::Files(_) | Event::Delete(_))));
}

#[tokio::test]
async fn incomplete_inspection_blocks_file_and_database_deletion() {
    let mut s = Scenario::new(2);
    s.files.incomplete_inspection = true;
    assert!(s.run(&Cancellation::default()).await.is_err());
    assert_eq!(s.repository.rows.len(), 2);
    assert!(!s
        .events
        .lock()
        .unwrap()
        .iter()
        .any(|event| matches!(event, Event::Files(_) | Event::Delete(_))));
}
