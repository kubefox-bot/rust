use super::{
    fixtures::{date, name, Fixture},
    Stopped, BOUNDARY, OLD,
};
use hmail_cleaner_adapters::filesystem::LocalFiles;
use hmail_cleaner_core::{
    cleanup::*,
    maintenance::{
        recursive::{self, Action, TreeRepository},
        ScanOptions,
    },
};
use sqlx::Executor;
use std::{
    fs,
    num::NonZeroU32,
    time::{Duration, SystemTime},
};

#[tokio::test]
#[ignore = "requires disposable MySQL"]
async fn inventory_accepts_more_than_a_million_messages() {
    let mut fixture = Fixture::create().await;
    let total = 1_000_001_i64;
    queue_messages(&mut fixture, total).await;
    let temp = tempfile::tempdir().unwrap();
    let mut repository = fixture.repository().await;
    repository
        .prepare_inventory(temp.path(), &Cancellation::default())
        .await
        .unwrap();
    let matched = repository
        .match_files(&[temp.path().join(name(1)), temp.path().join(name(total))])
        .await
        .unwrap();
    assert_eq!(matched.len(), 2);
    let mut cursor = 0;
    let mut count = 0;
    loop {
        let batch = repository.unmatched_batch(cursor, 1000).await.unwrap();
        if batch.is_empty() {
            break;
        }
        assert!(batch.len() <= 1000);
        for group in batch {
            assert!(group.row.candidate.id > cursor);
            assert!(![1, total].contains(&group.row.candidate.id));
            assert_eq!(group.references, 1);
            cursor = group.row.candidate.id;
            count += 1;
        }
    }
    repository.close().await.unwrap();
    fixture.finish().await;
    assert_eq!(count, total - 2);
}

async fn queue_messages(fixture: &mut Fixture, total: i64) {
    fixture.add(1, 0, 0, 1, OLD, &name(1)).await;
    let mut upper = 1_i64;
    while upper < total {
        let added = upper.min(total - upper);
        sqlx::query("INSERT INTO hm_messages (messageid,messageaccountid,messagefolderid,messagefilename,messagetype,messagefrom,messagesize,messagecurnooftries,messagenexttrytime,messageflags,messagecreatetime,messagelocked,messageuid) SELECT messageid+?,0,0,CONCAT('{00000000-0000-0000-0000-',LPAD(HEX(messageid+?),12,'0'),'}.eml'),1,'fixture@example.invalid',100,0,'2026-01-01',0,'2026-01-01',0,messageid+? FROM hm_messages WHERE messageid<=?")
            .bind(upper).bind(upper).bind(upper).bind(added)
            .execute(&mut fixture.admin).await.unwrap();
        upper += added;
    }
}

#[tokio::test]
#[ignore = "requires disposable MySQL"]
async fn recursive_preview_needs_only_select_access() {
    let mut fixture = Fixture::create().await;
    fixture.add(1, 0, 0, 1, OLD, &name(1)).await;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let file = root.join(name(1));
    fs::write(&file, b"keep").unwrap();
    let (user, config) = fixture.read_only_config().await;
    let mut repository = hmail_cleaner_adapters::mysql::MySqlRepository::connect(&config)
        .await
        .unwrap();
    let options = RunOptions {
        data_directory: root.clone(),
        cutoff: date(BOUNDARY),
        file_cutoff: SystemTime::now() + Duration::from_secs(60),
        batch_size: NonZeroU32::new(1).unwrap(),
        workers: 1,
    };
    let summary = recursive::run(
        Dependencies {
            repository: &mut repository,
            files: &LocalFiles::new(root).unwrap(),
            guard: &Stopped,
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
    assert_eq!(summary.deleted_messages, 0);
    assert_eq!(fs::read(file).unwrap(), b"keep");
    repository.close().await.unwrap();
    fixture.drop_reader(&user).await;
    fixture.finish().await;
}

#[tokio::test]
#[ignore = "requires disposable MySQL"]
async fn duplicate_path_across_mysql_pages_keeps_file_and_both_rows() {
    let mut fixture = Fixture::create().await;
    queue_messages(&mut fixture, 1001).await;
    sqlx::query("UPDATE hm_messages SET messagefilename=? WHERE messageid=1001")
        .bind(name(1).to_lowercase())
        .execute(&mut fixture.admin)
        .await
        .unwrap();
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let file = root.join(name(1));
    fs::write(&file, b"keep ambiguous").unwrap();
    let mut repository = fixture.repository().await;
    let options = RunOptions {
        data_directory: root.clone(),
        cutoff: date("2025-01-01 00:00:00"),
        file_cutoff: SystemTime::now() + Duration::from_secs(60),
        batch_size: NonZeroU32::new(7).unwrap(),
        workers: 1,
    };
    let summary = recursive::run(
        Dependencies {
            repository: &mut repository,
            files: &LocalFiles::new(root).unwrap(),
            guard: &Stopped,
        },
        &options,
        ScanOptions::Fast,
        &Cancellation::default(),
        |_| {},
        Action::Clean,
    )
    .await
    .unwrap();
    assert_eq!(summary.failed, 1);
    assert_eq!(summary.examined_messages, 1001);
    assert_eq!(summary.deleted_messages, 0);
    assert_eq!(fixture.ids("hm_messages", "messageid").await.len(), 1001);
    assert_eq!(fs::read(file).unwrap(), b"keep ambiguous");
    repository.close().await.unwrap();
    fixture.finish().await;
}

#[tokio::test]
#[ignore = "requires disposable MySQL"]
async fn invalid_path_on_later_mysql_page_blocks_all_file_deletion() {
    let mut fixture = Fixture::create().await;
    queue_messages(&mut fixture, 1001).await;
    sqlx::query("UPDATE hm_messages SET messagefilename='../outside.eml' WHERE messageid=1001")
        .execute(&mut fixture.admin)
        .await
        .unwrap();
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let file = root.join(name(1));
    fs::write(&file, b"keep").unwrap();
    let mut repository = fixture.repository().await;
    let options = RunOptions {
        data_directory: root.clone(),
        cutoff: date(BOUNDARY),
        file_cutoff: SystemTime::now() + Duration::from_secs(60),
        batch_size: NonZeroU32::new(1).unwrap(),
        workers: 1,
    };
    let error = recursive::run(
        Dependencies {
            repository: &mut repository,
            files: &LocalFiles::new(root).unwrap(),
            guard: &Stopped,
        },
        &options,
        ScanOptions::Fast,
        &Cancellation::default(),
        |_| {},
        Action::Clean,
    )
    .await
    .unwrap_err();
    assert_eq!(error.stage, Stage::References);
    assert_eq!(error.summary.removed_files, 0);
    assert_eq!(fixture.ids("hm_messages", "messageid").await.len(), 1001);
    assert!(repository.unmatched_batch(0, 1).await.is_err());
    assert_eq!(fs::read(file).unwrap(), b"keep");
    // Dropped read-only transactions and partial scratch indexes permit a retry.
    sqlx::query("UPDATE hm_messages SET messagefilename=? WHERE messageid=1001")
        .bind(name(1001))
        .execute(&mut fixture.admin)
        .await
        .unwrap();
    repository
        .prepare_inventory(&options.data_directory, &Cancellation::default())
        .await
        .unwrap();
    assert_eq!(
        repository.unmatched_batch(0, 1).await.unwrap()[0]
            .row
            .candidate
            .id,
        1
    );
    repository.close().await.unwrap();
    fixture.finish().await;
}

#[tokio::test]
#[ignore = "requires disposable MySQL"]
async fn cancelled_inventory_leaves_the_connection_and_read_only_rows_usable() {
    let mut fixture = Fixture::create().await;
    fixture.add(1, 0, 0, 1, OLD, &name(1)).await;
    let mut repository = fixture.repository().await;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let cancel = Cancellation::default();
    cancel.request();
    assert!(repository.prepare_inventory(&root, &cancel).await.is_err());
    repository
        .prepare_inventory(&root, &Cancellation::default())
        .await
        .unwrap();
    let groups = repository.unmatched_batch(0, 100).await.unwrap();
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].row.candidate.id, 1);
    repository.close().await.unwrap();
    fixture.finish().await;
}
#[tokio::test]
#[ignore = "requires disposable MySQL"]
async fn recursive_fast_and_full_clean_queue_nested_public_and_missing_rows() {
    for scan in [ScanOptions::Fast, ScanOptions::Full] {
        let mut fixture = Fixture::create().await;
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap().join("Data");
        let nested = root.join("unexpected/extra/depth").join(name(2));
        fs::create_dir_all(nested.parent().unwrap()).unwrap();
        let public = root.join("#Public/00").join(name(3));
        fs::create_dir_all(public.parent().unwrap()).unwrap();
        for path in [
            root.join(name(1)),
            nested.clone(),
            public,
            root.join("orphan.eml"),
        ] {
            fs::write(path, b"email").unwrap();
        }
        fixture.add(1, 0, 0, 1, BOUNDARY, &name(1)).await;
        fixture
            .add(2, 1, 10, 2, BOUNDARY, nested.to_str().unwrap())
            .await;
        fixture.add(3, 0, 40, 2, BOUNDARY, &name(3)).await;
        fixture.add(4, 0, 0, 1, OLD, &name(4)).await;
        fixture.add(5, 0, 0, 1, BOUNDARY, &name(5)).await;
        for id in 1..=5 {
            fixture.add_dependents(id).await;
        }
        let options = RunOptions {
            cutoff: date(BOUNDARY),
            file_cutoff: SystemTime::now() + Duration::from_secs(60),
            batch_size: NonZeroU32::new(2).unwrap(),
            workers: 2,
            data_directory: root.clone(),
        };
        let mut repository = fixture.repository().await;
        let summary = recursive::run(
            Dependencies {
                repository: &mut repository,
                files: &LocalFiles::new(root.clone()).unwrap(),
                guard: &Stopped,
            },
            &options,
            scan,
            &Cancellation::default(),
            |_| {},
            Action::Clean,
        )
        .await
        .unwrap();
        assert_eq!(summary.removed_files, 4);
        assert_eq!(summary.deleted_messages, 4);
        assert_eq!(summary.deleted_metadata, 4);
        assert_eq!(summary.deleted_recipients, 4);
        assert_eq!(summary.absent_files, 1);
        assert_eq!(summary.failed, 0);
        assert_eq!(fixture.ids("hm_messages", "messageid").await, vec![5]);
        assert_eq!(
            fixture
                .ids("hm_message_metadata", "metadata_messageid")
                .await,
            vec![5]
        );
        assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
        repository.close().await.unwrap();
        fixture.finish().await;
    }
}
#[tokio::test]
#[ignore = "requires disposable MySQL"]
async fn changed_queue_state_blocks_sql_deletion() {
    let mut fixture = Fixture::create().await;
    fixture.add(1, 0, 0, 1, OLD, &name(1)).await;
    fixture.add_dependents(1).await;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let mut repository = fixture.repository().await;
    repository
        .prepare_inventory(&root, &Cancellation::default())
        .await
        .unwrap();
    let rows = repository
        .unmatched_batch(0, 100)
        .await
        .unwrap()
        .into_iter()
        .map(|group| group.row)
        .collect::<Vec<_>>();
    fixture
        .admin
        .execute("UPDATE hm_messages SET messagetype=0 WHERE messageid=1")
        .await
        .unwrap();
    assert!(repository.delete_stored(&rows).await.is_err());
    assert_eq!(fixture.ids("hm_messages", "messageid").await, vec![1]);
    assert_eq!(
        fixture
            .ids("hm_message_metadata", "metadata_messageid")
            .await,
        vec![1]
    );
    repository.close().await.unwrap();
    fixture.finish().await;
}

#[tokio::test]
#[ignore = "requires disposable MySQL"]
async fn duplicate_guid_in_different_mailboxes_is_matched_by_full_path() {
    let mut fixture = Fixture::create().await;
    fixture.add(1, 1, 10, 2, BOUNDARY, &name(1)).await;
    fixture.add(2, 2, 30, 2, BOUNDARY, &name(1)).await;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap().join("Data");
    let old = root.join("chem.orp/info/00").join(name(1));
    let fresh = root.join("chem.orp/sales/00").join(name(1));
    fs::create_dir_all(old.parent().unwrap()).unwrap();
    fs::create_dir_all(fresh.parent().unwrap()).unwrap();
    fs::write(&old, b"old").unwrap();
    std::thread::sleep(Duration::from_millis(30));
    fs::write(&fresh, b"fresh").unwrap();
    let options = RunOptions {
        cutoff: date(BOUNDARY),
        file_cutoff: fs::metadata(&fresh).unwrap().created().unwrap(),
        batch_size: NonZeroU32::new(2).unwrap(),
        workers: 2,
        data_directory: root.clone(),
    };
    let mut repository = fixture.repository().await;
    let summary = recursive::run(
        Dependencies {
            repository: &mut repository,
            files: &LocalFiles::new(root.clone()).unwrap(),
            guard: &Stopped,
        },
        &options,
        ScanOptions::Fast,
        &Cancellation::default(),
        |_| {},
        Action::Clean,
    )
    .await
    .unwrap();
    assert_eq!(summary.removed_files, 1);
    assert_eq!(summary.deleted_messages, 1);
    assert_eq!(summary.failed, 0);
    assert!(fresh.exists() && !old.exists());
    assert_eq!(fixture.ids("hm_messages", "messageid").await, vec![2]);
    repository.close().await.unwrap();
    fixture.finish().await;
}
#[tokio::test]
#[ignore = "requires disposable MySQL"]
async fn unknown_message_state_fails_before_file_deletion() {
    let mut fixture = Fixture::create().await;
    fixture.add(1, 0, 0, 1, OLD, &name(1)).await;
    fixture.add(2, 0, 0, 1, OLD, &name(2)).await;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let file = root.join(name(1));
    fs::write(&file, b"keep").unwrap();
    let mut repository = fixture.repository().await;
    fixture
        .admin
        .execute("UPDATE hm_messages SET messagetype=127 WHERE messageid=1")
        .await
        .unwrap();
    assert!(repository
        .prepare_inventory(&root, &Cancellation::default())
        .await
        .is_err());
    assert!(file.exists());
    assert_eq!(fixture.ids("hm_messages", "messageid").await, vec![1, 2]);
    repository.close().await.unwrap();
    fixture.finish().await;
}
