use super::{
    fixtures::{date, name, Fixture},
    selection, Stopped, BOUNDARY, OLD,
};
use hmail_cleaner_adapters::filesystem::LocalFiles;
use hmail_cleaner_core::{
    cleanup::{Cancellation, Dependencies, Repository, RunOptions},
    maintenance::{self, ScanOptions},
};
use sqlx::Executor;
use std::{
    fs,
    num::NonZeroU32,
    time::{Duration, SystemTime},
};

#[tokio::test]
#[ignore = "requires disposable MySQL"]
async fn fast_cleans_public_messages_and_missing_files() {
    public_cleanup(ScanOptions::Fast).await;
}

#[tokio::test]
#[ignore = "requires disposable MySQL"]
async fn full_uses_creation_time_for_public_messages_and_orphans() {
    public_cleanup(ScanOptions::Full).await;
}

async fn public_cleanup(scan: ScanOptions) {
    let full = matches!(scan, ScanOptions::Full);
    let mut fixture = Fixture::create().await;
    for (id, created) in [
        (1, OLD),
        (2, BOUNDARY),
        (3, OLD),
        (7, BOUNDARY),
        (8, OLD),
        (9, BOUNDARY),
    ] {
        fixture.add(id, 0, 40, 2, created, &name(id)).await;
        fixture.add_dependents(id).await;
    }
    fixture
        .admin
        .execute("UPDATE hm_message_metadata SET metadata_accountid=0,metadata_folderid=40")
        .await
        .unwrap();
    fixture.add(4, 0, 0, 1, OLD, &name(4)).await;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap().join("Data");
    let bucket = root.join("#Public/00");
    fs::create_dir_all(&bucket).unwrap();
    for id in [1, 5, 7] {
        let path = bucket.join(name(id));
        fs::write(&path, b"1234567").unwrap();
        fs::OpenOptions::new()
            .write(true)
            .open(path)
            .unwrap()
            .set_times(
                fs::FileTimes::new().set_modified(SystemTime::now() + Duration::from_secs(3600)),
            )
            .unwrap();
    }
    std::thread::sleep(Duration::from_millis(20));
    for id in [2, 6, 8] {
        fs::write(bucket.join(name(id)), b"fresh").unwrap();
    }
    let file_cutoff = fs::metadata(bucket.join(name(2)))
        .unwrap()
        .created()
        .unwrap();
    let queue = root.join(name(4));
    fs::write(&queue, b"queue").unwrap();
    let files = LocalFiles::new(root.clone()).unwrap();
    let options = RunOptions {
        cutoff: date(BOUNDARY),
        file_cutoff,
        batch_size: NonZeroU32::new(1).unwrap(),
        workers: 2,
        data_directory: root,
    };
    let mut repository = fixture.repository().await;
    let preview = maintenance::preview(
        Dependencies {
            repository: &mut repository,
            files: &files,
            guard: &Stopped,
        },
        &options,
        scan,
        &Cancellation::default(),
        |_| {},
    )
    .await
    .unwrap();
    assert_eq!(preview.selected, 3);
    assert_eq!(preview.failed, 0);
    assert_eq!(preview.selected_orphan_files, u64::from(full));
    assert_eq!(preview.removed_files, 0);
    assert!(bucket.join(name(1)).exists());
    assert_eq!(
        fixture.ids("hm_messages", "messageid").await,
        vec![1, 2, 3, 4, 7, 8, 9]
    );
    let summary = maintenance::clean(
        Dependencies {
            repository: &mut repository,
            files: &files,
            guard: &Stopped,
        },
        &options,
        scan,
        &Cancellation::default(),
        |_| {},
    )
    .await
    .unwrap();
    assert_eq!(summary.failed, 0);
    assert_eq!(summary.deleted_messages, 3);
    assert_eq!(summary.deleted_metadata, 3);
    assert_eq!(summary.deleted_recipients, 3);
    assert_eq!(summary.absent_files, 1);
    assert_eq!(summary.removed_files, 2 + u64::from(full));
    assert_eq!(summary.removed_orphan_files, u64::from(full));
    assert!(!bucket.join(name(1)).exists());
    assert!(bucket.join(name(2)).exists());
    assert_eq!(bucket.join(name(5)).exists(), !full);
    assert!(bucket.join(name(6)).exists());
    assert!(!bucket.join(name(7)).exists());
    assert!(bucket.join(name(8)).exists());
    assert!(queue.exists());
    assert_eq!(
        fixture.ids("hm_messages", "messageid").await,
        vec![2, 4, 8, 9]
    );
    assert_eq!(
        fixture
            .ids("hm_message_metadata", "metadata_messageid")
            .await,
        vec![2, 8, 9]
    );
    assert_eq!(
        fixture
            .ids("hm_messagerecipients", "recipientmessageid")
            .await,
        vec![2, 8, 9]
    );
    repository.close().await.unwrap();
    fixture.finish().await;
}

#[tokio::test]
#[ignore = "requires disposable MySQL"]
async fn invalid_public_folder_is_rejected_and_duplicate_reference_is_protected() {
    let mut fixture = Fixture::create().await;
    for (id, folder) in [(1, 10), (2, 999), (3, 0), (4, 40)] {
        fixture.add(id, 0, folder, 2, OLD, &name(id)).await;
    }
    fixture.add(5, 1, 10, 2, BOUNDARY, &name(4)).await;
    let mut repository = fixture.repository().await;
    let snapshot = repository.snapshot().await.unwrap();
    let batch = repository
        .next_batch(selection(snapshot.upper_id), 0, 1000)
        .await
        .unwrap();
    assert!(batch.candidates.is_empty());
    assert_eq!(
        batch.rejected.iter().map(|row| row.id).collect::<Vec<_>>(),
        vec![1, 2, 3, 4, 5]
    );
    repository.close().await.unwrap();
    fixture.finish().await;
}

#[tokio::test]
#[ignore = "requires disposable MySQL"]
async fn changing_public_folder_owner_prevents_database_deletion() {
    let mut fixture = Fixture::create().await;
    fixture.add(1, 0, 40, 2, OLD, &name(1)).await;
    fixture.add_dependents(1).await;
    let mut repository = fixture.repository().await;
    let snapshot = repository.snapshot().await.unwrap();
    let batch = repository
        .next_batch(selection(snapshot.upper_id), 0, 1000)
        .await
        .unwrap();
    assert_eq!(batch.candidates.len(), 1);
    fixture
        .admin
        .execute("UPDATE hm_imapfolders SET folderaccountid=1 WHERE folderid=40")
        .await
        .unwrap();
    assert!(repository.delete_batch(&batch.candidates).await.is_err());
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
