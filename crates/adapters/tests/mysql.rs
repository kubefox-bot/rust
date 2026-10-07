mod fixtures;

use fixtures::{date, name, Fixture};
use hmail_cleaner_adapters::mysql::{DatabaseConfig, MySqlRepository};
use hmail_cleaner_core::cleanup::{DeleteResult, Repository, Selection};
use sqlx::Executor;

const OLD: &str = "2026-01-01 10:00:00";
const BOUNDARY: &str = "2026-09-07 12:00:00";

fn selection(upper_id: i64) -> Selection {
    Selection {
        cutoff: date(BOUNDARY),
        upper_id,
    }
}

#[test]
fn config_rejects_missing_database() {
    assert!(DatabaseConfig::parse("mysql://root:dummy@127.0.0.1").is_err());
}

#[test]
fn config_debug_hides_credentials() {
    let config = DatabaseConfig::parse("mysql://root:do-not-print-this@127.0.0.1/hmaildb").unwrap();
    assert!(!format!("{config:?}").contains("do-not-print-this"));
}

#[tokio::test]
#[ignore = "requires mise run test:db disposable MySQL 5.7.21"]
async fn selects_only_old_delivered_accounts_across_folders() {
    let mut fixture = Fixture::create().await;
    for (id, account, folder, state, created) in [
        (1, 1, 10, 2, OLD),
        (2, 1, 20, 2, OLD),
        (3, 2, 30, 2, OLD),
        (4, 0, 0, 1, OLD),
        (5, 0, 40, 2, OLD),
        (6, 1, 10, 2, "2026-10-01 12:00:00"),
        (7, 1, 20, 2, BOUNDARY),
    ] {
        fixture
            .add(id, account, folder, state, created, &name(id))
            .await;
    }
    let mut repository = fixture.repository().await;
    let snapshot = repository.snapshot().await.unwrap();
    let batch = repository
        .next_batch(selection(snapshot.upper_id), 0, 1000)
        .await
        .unwrap();
    assert_eq!(
        batch
            .candidates
            .iter()
            .map(|row| row.id)
            .collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
    assert_eq!(batch.candidates[0].account_address, "info@chem.orp");
    assert_eq!(batch.candidates[2].account_address, "sales@chem.orp");
    assert!(batch.rejected.is_empty());
    repository.close().await.unwrap();
    fixture.finish().await;
}

#[tokio::test]
#[ignore = "requires disposable MySQL"]
async fn keyset_cursor_advances_over_rejected_rows() {
    let mut fixture = Fixture::create().await;
    fixture.add(1, 1, 10, 2, OLD, &name(1)).await;
    fixture.add(2, 9999, 10, 2, OLD, &name(2)).await;
    fixture.add(3, 1, 10, 2, OLD, &name(3)).await;
    fixture.add(4, 1, 30, 2, OLD, &name(4)).await;
    let mut repository = fixture.repository().await;
    let snapshot = repository.snapshot().await.unwrap();
    let first = repository
        .next_batch(selection(snapshot.upper_id), 0, 1)
        .await
        .unwrap();
    assert_eq!(first.last_id, 1);
    let rejected = repository
        .next_batch(selection(snapshot.upper_id), first.last_id, 1)
        .await
        .unwrap();
    assert_eq!(rejected.last_id, 2);
    assert_eq!(rejected.rejected[0].id, 2);
    assert!(rejected.candidates.is_empty());
    let next = repository
        .next_batch(selection(snapshot.upper_id), rejected.last_id, 1)
        .await
        .unwrap();
    assert_eq!(next.candidates[0].id, 3);
    let mismatch = repository
        .next_batch(selection(snapshot.upper_id), next.last_id, 1)
        .await
        .unwrap();
    assert_eq!(mismatch.rejected[0].id, 4);
    repository.close().await.unwrap();
    fixture.finish().await;
}

#[tokio::test]
#[ignore = "requires disposable MySQL"]
async fn legacy_and_bare_duplicate_names_are_rejected() {
    let mut fixture = Fixture::create().await;
    fixture.add(1, 1, 10, 2, OLD, &name(1)).await;
    fixture
        .add(
            2,
            1,
            10,
            2,
            "2026-10-01 10:00:00",
            &format!("D:\\Data\\chem.orp\\info\\00\\{}", name(1).to_lowercase()),
        )
        .await;
    let mut repository = fixture.repository().await;
    let snapshot = repository.snapshot().await.unwrap();
    let batch = repository
        .next_batch(selection(snapshot.upper_id), 0, 1000)
        .await
        .unwrap();
    assert!(batch.candidates.is_empty());
    assert_eq!(batch.rejected[0].id, 1);
    repository.close().await.unwrap();
    fixture.finish().await;
}

#[tokio::test]
#[ignore = "requires disposable MySQL"]
async fn deletes_only_requested_ids_and_their_dependents() {
    let mut fixture = Fixture::create().await;
    for id in [1, 2, 3] {
        fixture.add(id, 1, 10, 2, OLD, &name(id)).await;
        fixture.add_dependents(id).await;
    }
    let mut repository = fixture.repository().await;
    let snapshot = repository.snapshot().await.unwrap();
    let batch = repository
        .next_batch(selection(snapshot.upper_id), 0, 1000)
        .await
        .unwrap();
    let allowed = vec![batch.candidates[0].clone(), batch.candidates[2].clone()];
    assert_eq!(
        repository.delete_batch(&allowed).await.unwrap(),
        DeleteResult::Committed(hmail_cleaner_core::cleanup::DeletedRows {
            messages: 2,
            metadata: 2,
            recipients: 2
        })
    );
    repository.verify_deleted(&[1, 3]).await.unwrap();
    for (table, column) in [
        ("hm_messages", "messageid"),
        ("hm_message_metadata", "metadata_messageid"),
        ("hm_messagerecipients", "recipientmessageid"),
    ] {
        assert_eq!(fixture.ids(table, column).await, vec![2]);
    }
    repository.close().await.unwrap();
    fixture.finish().await;
}

#[tokio::test]
#[ignore = "requires disposable MySQL"]
async fn empty_delete_is_a_noop() {
    let mut fixture = Fixture::create().await;
    fixture.add(1, 1, 10, 2, OLD, &name(1)).await;
    let mut repository = fixture.repository().await;
    repository.snapshot().await.unwrap();
    assert_eq!(
        repository.delete_batch(&[]).await.unwrap(),
        DeleteResult::Committed(hmail_cleaner_core::cleanup::DeletedRows::default())
    );
    assert_eq!(fixture.ids("hm_messages", "messageid").await, vec![1]);
    repository.close().await.unwrap();
    fixture.finish().await;
}

#[tokio::test]
#[ignore = "requires disposable MySQL"]
async fn failed_child_delete_rolls_back_parent() {
    let mut fixture = Fixture::create().await;
    fixture.add(1, 1, 10, 2, OLD, &name(1)).await;
    fixture.add_dependents(1).await;
    fixture
        .admin
        .execute(
            "CREATE TRIGGER fail_recipient_delete BEFORE DELETE ON hm_messagerecipients
         FOR EACH ROW SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT='fixture failure'",
        )
        .await
        .unwrap();
    let mut repository = fixture.repository().await;
    let snapshot = repository.snapshot().await.unwrap();
    let batch = repository
        .next_batch(selection(snapshot.upper_id), 0, 1000)
        .await
        .unwrap();
    assert!(repository.delete_batch(&batch.candidates).await.is_err());
    for (table, column) in [
        ("hm_messages", "messageid"),
        ("hm_message_metadata", "metadata_messageid"),
        ("hm_messagerecipients", "recipientmessageid"),
    ] {
        assert_eq!(fixture.ids(table, column).await, vec![1]);
    }
    repository.close().await.unwrap();
    fixture.finish().await;
}

#[tokio::test]
#[ignore = "requires disposable MySQL"]
async fn second_connection_cannot_acquire_cleanup_lock() {
    let fixture = Fixture::create().await;
    let first = fixture.repository().await;
    assert!(MySqlRepository::connect(&fixture.config).await.is_err());
    first.close().await.unwrap();
    let next = fixture.repository().await;
    next.close().await.unwrap();
    fixture.finish().await;
}

#[tokio::test]
#[ignore = "requires disposable MySQL"]
async fn changed_row_is_not_deleted() {
    let mut fixture = Fixture::create().await;
    fixture.add(1, 1, 10, 2, OLD, &name(1)).await;
    let mut repository = fixture.repository().await;
    let snapshot = repository.snapshot().await.unwrap();
    let batch = repository
        .next_batch(selection(snapshot.upper_id), 0, 1000)
        .await
        .unwrap();
    fixture
        .admin
        .execute("UPDATE hm_messages SET messagefolderid=20 WHERE messageid=1")
        .await
        .unwrap();
    assert!(repository.delete_batch(&batch.candidates).await.is_err());
    assert_eq!(fixture.ids("hm_messages", "messageid").await, vec![1]);
    repository.close().await.unwrap();
    fixture.finish().await;
}

#[tokio::test]
#[ignore = "requires disposable MySQL"]
async fn unsupported_storage_engine_is_rejected_before_selection() {
    let mut fixture = Fixture::create().await;
    fixture
        .admin
        .execute("ALTER TABLE hm_message_metadata ENGINE=MyISAM")
        .await
        .unwrap();
    assert!(MySqlRepository::connect(&fixture.config).await.is_err());
    fixture.finish().await;
}

#[tokio::test]
#[ignore = "requires disposable MySQL"]
async fn snapshot_excludes_later_insertions_and_query_uses_primary_range() {
    use sqlx::{Column, Row};
    let mut fixture = Fixture::create().await;
    fixture.add(1, 1, 10, 2, OLD, &name(1)).await;
    let mut repository = fixture.repository().await;
    let snapshot = repository.snapshot().await.unwrap();
    fixture.add(2, 1, 10, 2, OLD, &name(2)).await;
    let batch = repository
        .next_batch(selection(snapshot.upper_id), 0, 1000)
        .await
        .unwrap();
    assert_eq!(
        batch
            .candidates
            .iter()
            .map(|row| row.id)
            .collect::<Vec<_>>(),
        vec![1]
    );
    let explain = sqlx::query("EXPLAIN SELECT messageid,messageaccountid,messagefolderid,messagefilename,messagecreatetime FROM hm_messages FORCE INDEX(PRIMARY) WHERE messageid>0 AND messageid<=2 AND messagetype=2 AND messageaccountid>0 AND messagecreatetime<'2026-09-07 12:00:00' ORDER BY messageid LIMIT 1000").fetch_one(&mut fixture.admin).await.unwrap();
    // MySQL 5.7 EXPLAIN metadata does not populate SQLx's name lookup.
    let value = |name: &str| {
        let index = explain
            .columns()
            .iter()
            .position(|column| column.name() == name)
            .unwrap();
        explain.get::<String, _>(index)
    };
    assert_eq!(value("key"), "PRIMARY");
    assert_eq!(value("type"), "range");
    assert!(!value("Extra").contains("filesort"));
    repository.close().await.unwrap();
    fixture.finish().await;
}

// Test-only transport boundary: SQL really commits, but the caller loses its acknowledgement.
struct LostAcknowledgement(MySqlRepository);
impl Repository for LostAcknowledgement {
    async fn snapshot(
        &mut self,
    ) -> hmail_cleaner_core::cleanup::PortResult<hmail_cleaner_core::cleanup::Snapshot> {
        self.0.snapshot().await
    }
    async fn next_batch(
        &mut self,
        selection: Selection,
        cursor: i64,
        limit: u32,
    ) -> hmail_cleaner_core::cleanup::PortResult<hmail_cleaner_core::cleanup::Batch> {
        self.0.next_batch(selection, cursor, limit).await
    }
    async fn delete_batch(
        &mut self,
        candidates: &[hmail_cleaner_core::cleanup::Candidate],
    ) -> hmail_cleaner_core::cleanup::PortResult<DeleteResult> {
        assert!(matches!(
            self.0.delete_batch(candidates).await?,
            DeleteResult::Committed(_)
        ));
        Ok(DeleteResult::CommitUncertain {
            reason: "fixture lost acknowledgement".into(),
        })
    }
    async fn verify_deleted(&mut self, ids: &[i64]) -> hmail_cleaner_core::cleanup::PortResult<()> {
        self.0.verify_deleted(ids).await
    }
}
struct Stopped;
impl hmail_cleaner_core::cleanup::OfflineGuard for Stopped {
    async fn ensure_stopped(&self) -> hmail_cleaner_core::cleanup::PortResult<()> {
        Ok(())
    }
}

#[tokio::test]
#[ignore = "requires disposable MySQL"]
async fn lost_commit_acknowledgement_stops_then_repeat_preserves_other_rows() {
    use hmail_cleaner_adapters::filesystem::LocalFiles;
    use hmail_cleaner_core::cleanup::{clean, Cancellation, Dependencies, RunOptions};
    let mut fixture = Fixture::create().await;
    fixture.add(1, 1, 10, 2, OLD, &name(1)).await;
    fixture.add_dependents(1).await;
    fixture
        .add(2, 1, 10, 2, "2026-10-01 10:00:00", &name(2))
        .await;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let parent = root.join("chem.orp/info/00");
    std::fs::create_dir_all(&parent).unwrap();
    std::fs::write(parent.join(name(1)), b"old").unwrap();
    std::fs::write(parent.join(name(2)), b"new").unwrap();
    let files = LocalFiles::new(root.clone()).unwrap();
    let options = RunOptions {
        cutoff: date(BOUNDARY),
        batch_size: std::num::NonZeroU32::new(1).unwrap(),
        workers: 2,
        data_directory: root,
    };
    let mut repository = LostAcknowledgement(fixture.repository().await);
    let error = clean(
        Dependencies {
            repository: &mut repository,
            files: &files,
            guard: &Stopped,
        },
        &options,
        &Cancellation::default(),
        |_| {},
    )
    .await
    .unwrap_err();
    assert_eq!(error.summary.removed_files, 1);
    assert_eq!(error.summary.deleted_messages, 0);
    assert!(!parent.join(name(1)).exists());
    assert!(parent.join(name(2)).exists());
    assert_eq!(fixture.ids("hm_messages", "messageid").await, vec![2]);
    repository.0.close().await.unwrap();
    let mut repository = fixture.repository().await;
    let summary = clean(
        Dependencies {
            repository: &mut repository,
            files: &files,
            guard: &Stopped,
        },
        &options,
        &Cancellation::default(),
        |_| {},
    )
    .await
    .unwrap();
    assert_eq!(summary.selected, 0);
    assert!(parent.join(name(2)).exists());
    repository.close().await.unwrap();
    fixture.finish().await;
}

#[tokio::test]
#[ignore = "requires disposable MySQL"]
async fn real_sql_rollback_after_file_deletion_is_reconciled_on_repeat() {
    use hmail_cleaner_adapters::filesystem::LocalFiles;
    use hmail_cleaner_core::cleanup::{clean, Cancellation, Dependencies, RunOptions};
    let mut fixture = Fixture::create().await;
    fixture.add(1, 1, 10, 2, OLD, &name(1)).await;
    fixture.add_dependents(1).await;
    fixture.admin.execute("CREATE TRIGGER fail_recipient_delete BEFORE DELETE ON hm_messagerecipients FOR EACH ROW SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT='fixture failure'").await.unwrap();
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let parent = root.join("chem.orp/info/00");
    std::fs::create_dir_all(&parent).unwrap();
    std::fs::write(parent.join(name(1)), b"old").unwrap();
    let files = LocalFiles::new(root.clone()).unwrap();
    let options = RunOptions {
        cutoff: date(BOUNDARY),
        batch_size: std::num::NonZeroU32::new(1).unwrap(),
        workers: 2,
        data_directory: root.clone(),
    };
    let mut repository = fixture.repository().await;
    assert!(clean(
        Dependencies {
            repository: &mut repository,
            files: &files,
            guard: &Stopped
        },
        &options,
        &Cancellation::default(),
        |_| {}
    )
    .await
    .is_err());
    assert!(!parent.join(name(1)).exists());
    assert!(parent.is_dir());
    assert_eq!(fixture.ids("hm_messages", "messageid").await, vec![1]);
    fixture
        .admin
        .execute("DROP TRIGGER fail_recipient_delete")
        .await
        .unwrap();
    let summary = clean(
        Dependencies {
            repository: &mut repository,
            files: &files,
            guard: &Stopped,
        },
        &options,
        &Cancellation::default(),
        |_| {},
    )
    .await
    .unwrap();
    assert_eq!(summary.absent_files, 1);
    assert_eq!(summary.deleted_messages, 1);
    assert!(root.is_dir());
    assert!(!parent.exists());
    repository.close().await.unwrap();
    fixture.finish().await;
}

#[tokio::test]
#[ignore = "requires disposable MySQL"]
async fn read_only_database_can_preview_but_fails_delete_preflight() {
    let mut fixture = Fixture::create().await;
    fixture.add(1, 1, 10, 2, OLD, &name(1)).await;
    let (user, config) = fixture.read_only_config().await;
    let mut reader = MySqlRepository::connect(&config).await.unwrap();
    let snapshot = reader.snapshot().await.unwrap();
    assert_eq!(
        reader
            .next_batch(selection(snapshot.upper_id), 0, 1000)
            .await
            .unwrap()
            .candidates
            .len(),
        1
    );
    assert!(reader.verify_delete_access().await.is_err());
    reader.close().await.unwrap();
    let mut writer = fixture.repository().await;
    writer.verify_delete_access().await.unwrap();
    assert_eq!(fixture.ids("hm_messages", "messageid").await, vec![1]);
    writer.close().await.unwrap();
    fixture.drop_reader(&user).await;
    fixture.finish().await;
}

#[tokio::test]
#[ignore = "requires disposable MySQL"]
async fn invalid_datetime_is_reported_and_next_message_is_cleaned() {
    let mut fixture = Fixture::create().await;
    for id in [1, 2] {
        fixture.add(id, 1, 10, 2, OLD, &name(id)).await;
        fixture.add_dependents(id).await;
    }
    fixture
        .admin
        .execute("SET SESSION sql_mode=''")
        .await
        .unwrap();
    fixture
        .admin
        .execute("UPDATE hm_messages SET messagecreatetime='0000-00-00 00:00:00' WHERE messageid=1")
        .await
        .unwrap();
    let mut repository = fixture.repository().await;
    let snapshot = repository.snapshot().await.unwrap();
    let bad = repository
        .next_batch(selection(snapshot.upper_id), 0, 1)
        .await
        .unwrap();
    assert_eq!(bad.last_id, 1);
    assert!(bad.candidates.is_empty());
    assert_eq!(bad.rejected[0].id, 1);
    assert!(bad.rejected[0].reason.contains("messagecreatetime"));
    let good = repository
        .next_batch(selection(snapshot.upper_id), bad.last_id, 1)
        .await
        .unwrap();
    assert_eq!(good.candidates[0].id, 2);
    assert_eq!(
        repository.delete_batch(&good.candidates).await.unwrap(),
        DeleteResult::Committed(hmail_cleaner_core::cleanup::DeletedRows {
            messages: 1,
            metadata: 1,
            recipients: 1
        })
    );
    repository.verify_deleted(&[2]).await.unwrap();
    for (table, column) in [
        ("hm_messages", "messageid"),
        ("hm_message_metadata", "metadata_messageid"),
        ("hm_messagerecipients", "recipientmessageid"),
    ] {
        assert_eq!(fixture.ids(table, column).await, vec![1]);
    }
    repository.close().await.unwrap();
    fixture.finish().await;
}
