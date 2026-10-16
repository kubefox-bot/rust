use hmail_cleaner_adapters::filesystem::LocalFiles;
use hmail_cleaner_core::{
    cleanup::*,
    maintenance::{
        recursive::{self, Action, TreeRepository},
        *,
    },
};
use std::{
    fs,
    num::NonZeroU32,
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};
struct Repo {
    rows: Vec<StoredMessage>,
    fail_sql: bool,
}
impl Repository for Repo {
    async fn snapshot(&mut self) -> PortResult<Snapshot> {
        unreachable!()
    }
    async fn next_batch(&mut self, _: Selection, _: i64, _: u32) -> PortResult<Batch> {
        unreachable!()
    }
    async fn delete_batch(&mut self, _: &[Candidate]) -> PortResult<DeleteResult> {
        unreachable!()
    }
    async fn verify_deleted(&mut self, ids: &[i64]) -> PortResult<()> {
        assert!(!self.rows.iter().any(|r| ids.contains(&r.candidate.id)));
        Ok(())
    }
}
impl FullRepository for Repo {
    async fn orphan_upper_id(&mut self, _: OrphanTable) -> PortResult<i64> {
        Ok(0)
    }
    async fn orphan_batch(
        &mut self,
        _: OrphanSelection,
        cursor: i64,
        _: u32,
    ) -> PortResult<OrphanBatch> {
        Ok(OrphanBatch {
            last_id: cursor,
            rows: vec![],
        })
    }
    async fn delete_orphans(
        &mut self,
        _: OrphanTable,
        _: &[OrphanRow],
    ) -> PortResult<DeleteResult> {
        unreachable!()
    }
    async fn verify_orphans_deleted(&mut self, _: OrphanTable, _: &[i64]) -> PortResult<()> {
        unreachable!()
    }
    async fn file_references(&mut self, _: usize) -> PortResult<FileReferences> {
        unreachable!()
    }
}
impl TreeRepository for Repo {
    async fn stored_messages(&mut self, _: &Path, _: usize) -> PortResult<Vec<StoredMessage>> {
        Ok(self.rows.clone())
    }
    async fn delete_stored(&mut self, rows: &[StoredMessage]) -> PortResult<DeleteResult> {
        assert!(
            rows.iter().all(|row| !row.path.exists()),
            "SQL must follow confirmed absence"
        );
        if self.fail_sql {
            self.fail_sql = false;
            return Err("fixture SQL failure".into());
        }
        let ids = rows.iter().map(|r| r.candidate.id).collect::<Vec<_>>();
        self.rows.retain(|row| !ids.contains(&row.candidate.id));
        Ok(DeleteResult::Committed(DeletedRows {
            messages: ids.len() as u64,
            metadata: ids.len() as u64,
            recipients: 0,
        }))
    }
}
struct Guard;
impl OfflineGuard for Guard {
    async fn ensure_stopped(&self) -> PortResult<()> {
        Ok(())
    }
}
fn row(id: i64, path: PathBuf, created: chrono::NaiveDateTime) -> StoredMessage {
    StoredMessage {
        candidate: Candidate {
            id,
            account_id: 0,
            folder_id: 0,
            filename: path.to_string_lossy().into(),
            created_at: created,
            storage: MessageStorage::Queue,
        },
        message_type: 1,
        path,
    }
}
fn options(root: &Path) -> RunOptions {
    RunOptions {
        cutoff: chrono::NaiveDate::from_ymd_opt(2025, 1, 1)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap(),
        file_cutoff: SystemTime::now() + Duration::from_secs(60),
        batch_size: NonZeroU32::new(2).unwrap(),
        workers: 2,
        data_directory: root.to_owned(),
    }
}
async fn run(
    repo: &mut Repo,
    root: &Path,
    options: &RunOptions,
    scan: ScanOptions,
    action: Action,
) -> Result<Summary, CleanupError> {
    recursive::run(
        Dependencies {
            repository: repo,
            files: &LocalFiles::new(root.to_owned()).unwrap(),
            guard: &Guard,
        },
        options,
        scan,
        &Cancellation::default(),
        |_| {},
        action,
    )
    .await
}
#[tokio::test]
async fn fast_cleans_queue_arbitrary_depth_orphans_and_initially_empty_directories() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let queue = root.join("{00000000-0000-0000-0000-000000000001}.eml");
    let nested = root.join("unusual/deep/extra/letter.EML");
    fs::create_dir_all(nested.parent().unwrap()).unwrap();
    fs::create_dir_all(root.join("already/empty")).unwrap();
    fs::write(&queue, b"queue").unwrap();
    fs::write(&nested, b"orphan").unwrap();
    fs::write(root.join("keep.txt"), b"keep").unwrap();
    let options = options(&root);
    let fresh_database_date = options.cutoff + chrono::Duration::days(100);
    let mut repo = Repo {
        rows: vec![row(1, queue.clone(), fresh_database_date)],
        fail_sql: false,
    };
    let summary = run(&mut repo, &root, &options, ScanOptions::Fast, Action::Clean)
        .await
        .unwrap();
    assert_eq!(summary.removed_files, 2);
    assert_eq!(summary.remaining_old_files, None);
    assert_eq!(summary.deleted_messages, 1);
    assert_eq!(summary.removed_orphan_files, 1);
    assert_eq!(summary.removed_bytes, 11);
    assert_eq!(summary.failed, 0);
    assert!(repo.rows.is_empty());
    assert!(!queue.exists() && !nested.exists() && !root.join("already").exists());
    assert!(root.join("keep.txt").exists());
}
#[tokio::test]
async fn preview_and_exact_creation_boundary_preserve_files_and_database() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let file = root.join("old.eml");
    fs::write(&file, b"email").unwrap();
    let mut options = options(&root);
    let mut repo = Repo {
        rows: vec![row(1, file.clone(), options.cutoff)],
        fail_sql: false,
    };
    let summary = run(
        &mut repo,
        &root,
        &options,
        ScanOptions::Fast,
        Action::Preview,
    )
    .await
    .unwrap();
    assert_eq!(summary.selected, 1);
    assert_eq!(summary.selected_bytes, 5);
    assert_eq!(summary.removed_files, 0);
    assert!(file.exists());
    assert_eq!(repo.rows.len(), 1);
    options.file_cutoff = fs::metadata(&file).unwrap().created().unwrap();
    let summary = run(&mut repo, &root, &options, ScanOptions::Full, Action::Clean)
        .await
        .unwrap();
    assert_eq!(summary.removed_files, 0);
    assert_eq!(summary.failed, 0);
    assert!(file.exists());
}
#[tokio::test]
async fn rerun_reconciles_missing_file_after_database_failure_and_keeps_fresh_missing_row() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let file = root.join("old.eml");
    fs::write(&file, b"email").unwrap();
    let options = options(&root);
    let old = options.cutoff - chrono::Duration::days(1);
    let mut repo = Repo {
        rows: vec![
            row(1, file.clone(), old),
            row(2, root.join("missing-fresh.eml"), options.cutoff),
        ],
        fail_sql: true,
    };
    assert!(
        run(&mut repo, &root, &options, ScanOptions::Fast, Action::Clean)
            .await
            .is_err()
    );
    assert!(!file.exists());
    assert_eq!(repo.rows.len(), 2);
    let summary = run(&mut repo, &root, &options, ScanOptions::Full, Action::Clean)
        .await
        .unwrap();
    assert_eq!(summary.absent_files, 1);
    assert_eq!(summary.deleted_messages, 1);
    assert_eq!(summary.remaining_old_files, Some(0));
    assert_eq!(repo.rows[0].candidate.id, 2);
}
#[tokio::test]
async fn ambiguous_database_path_is_reported_and_preserved() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let file = root.join("old.eml");
    fs::write(&file, b"email").unwrap();
    let options = options(&root);
    let mut repo = Repo {
        rows: vec![
            row(1, file.clone(), options.cutoff),
            row(2, file.clone(), options.cutoff),
        ],
        fail_sql: false,
    };
    let summary = run(&mut repo, &root, &options, ScanOptions::Full, Action::Clean)
        .await
        .unwrap();
    assert!(summary.failed > 0);
    assert_eq!(summary.remaining_old_files, Some(1));
    assert!(file.exists());
    assert_eq!(repo.rows.len(), 2);
}

#[cfg(unix)]
#[tokio::test]
async fn denied_unlink_keeps_database_row() {
    use std::{
        os::unix::fs::PermissionsExt,
        sync::atomic::{AtomicBool, Ordering},
    };
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let parent = root.join("mail");
    fs::create_dir(&parent).unwrap();
    let file = parent.join("old.eml");
    fs::write(&file, b"email").unwrap();
    let options = options(&root);
    let mut repo = Repo {
        rows: vec![row(1, file.clone(), options.cutoff)],
        fail_sql: false,
    };
    let denied = AtomicBool::new(false);
    let result = recursive::run(
        Dependencies {
            repository: &mut repo,
            files: &LocalFiles::new(root.clone()).unwrap(),
            guard: &Guard,
        },
        &options,
        ScanOptions::Fast,
        &Cancellation::default(),
        |event| {
            if event.stage == Stage::Files && !denied.swap(true, Ordering::SeqCst) {
                fs::set_permissions(&parent, fs::Permissions::from_mode(0o500)).unwrap();
            }
        },
        Action::Clean,
    )
    .await;
    fs::set_permissions(&parent, fs::Permissions::from_mode(0o700)).unwrap();
    let summary = result.unwrap();
    assert_eq!(summary.failed, 1);
    assert_eq!(summary.deleted_messages, 0);
    assert!(file.exists());
    assert_eq!(repo.rows.len(), 1);
}
#[tokio::test]
async fn cancellation_completes_started_file_and_sql_batch_then_closes_scan() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let files = [root.join("a.eml"), root.join("b.eml")];
    for file in &files {
        fs::write(file, b"email").unwrap();
    }
    let mut options = options(&root);
    options.batch_size = NonZeroU32::new(1).unwrap();
    let mut repo = Repo {
        rows: vec![
            row(1, files[0].clone(), options.cutoff),
            row(2, files[1].clone(), options.cutoff),
        ],
        fail_sql: false,
    };
    let cancel = Cancellation::default();
    let error = recursive::run(
        Dependencies {
            repository: &mut repo,
            files: &LocalFiles::new(root.clone()).unwrap(),
            guard: &Guard,
        },
        &options,
        ScanOptions::Fast,
        &cancel,
        |event| {
            if event.stage == Stage::Files {
                cancel.request();
            }
        },
        Action::Clean,
    )
    .await
    .unwrap_err();
    assert_eq!(error.stage, Stage::Cancelled);
    assert_eq!(error.summary.removed_files, 1);
    assert_eq!(error.summary.deleted_messages, 1);
    assert_eq!(repo.rows.len(), 1);
    assert_eq!(files.iter().filter(|f| f.exists()).count(), 1);
    fs::remove_dir_all(root).unwrap();
}
#[tokio::test]
async fn reappeared_missing_file_blocks_database_cleanup() {
    use std::sync::atomic::{AtomicBool, Ordering};
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let file = root.join("reappeared.eml");
    let options = options(&root);
    let mut repo = Repo {
        rows: vec![row(
            1,
            file.clone(),
            options.cutoff - chrono::Duration::days(1),
        )],
        fail_sql: false,
    };
    let appeared = AtomicBool::new(false);
    let summary = recursive::run(
        Dependencies {
            repository: &mut repo,
            files: &LocalFiles::new(root.clone()).unwrap(),
            guard: &Guard,
        },
        &options,
        ScanOptions::Fast,
        &Cancellation::default(),
        |event| {
            if event.stage == Stage::Files && !appeared.swap(true, Ordering::SeqCst) {
                fs::write(&file, b"new mail").unwrap();
            }
        },
        Action::Clean,
    )
    .await
    .unwrap();
    assert_eq!(summary.failed, 1);
    assert_eq!(summary.deleted_messages, 0);
    assert_eq!(fs::read(&file).unwrap(), b"new mail");
    assert_eq!(repo.rows.len(), 1);
}

#[tokio::test]
async fn fast_preview_is_read_only_with_running_mail_but_other_operations_require_stop() {
    struct Running;
    impl OfflineGuard for Running {
        async fn ensure_stopped(&self) -> PortResult<()> {
            Err("fixture mail is running".into())
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let file = root.join("old.eml");
    fs::write(&file, b"email").unwrap();
    let options = options(&root);
    let files = LocalFiles::new(root.clone()).unwrap();
    let mut repo = Repo {
        rows: vec![],
        fail_sql: false,
    };
    let summary = recursive::run(
        Dependencies {
            repository: &mut repo,
            files: &files,
            guard: &Running,
        },
        &options,
        ScanOptions::Fast,
        &Cancellation::default(),
        |_| {},
        Action::Preview,
    )
    .await
    .unwrap();
    assert_eq!(summary.selected_files, 1);
    assert_eq!(summary.removed_files, 0);
    assert!(file.exists());
    for (scan, action) in [
        (ScanOptions::Full, Action::Preview),
        (ScanOptions::Fast, Action::Clean),
        (ScanOptions::Full, Action::Clean),
    ] {
        let error = recursive::run(
            Dependencies {
                repository: &mut repo,
                files: &files,
                guard: &Running,
            },
            &options,
            scan,
            &Cancellation::default(),
            |_| {},
            action,
        )
        .await
        .unwrap_err();
        assert_eq!(error.stage, Stage::Preflight);
        assert!(file.exists());
    }
}
