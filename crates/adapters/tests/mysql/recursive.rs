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
    let rows = repository.stored_messages(&root, 100).await.unwrap();
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
async fn inventory_limit_and_unknown_message_state_fail_before_file_deletion() {
    let mut fixture = Fixture::create().await;
    fixture.add(1, 0, 0, 1, OLD, &name(1)).await;
    fixture.add(2, 0, 0, 1, OLD, &name(2)).await;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let file = root.join(name(1));
    fs::write(&file, b"keep").unwrap();
    let mut repository = fixture.repository().await;
    assert!(repository.stored_messages(&root, 1).await.is_err());
    fixture
        .admin
        .execute("UPDATE hm_messages SET messagetype=127 WHERE messageid=1")
        .await
        .unwrap();
    assert!(repository.stored_messages(&root, 100).await.is_err());
    assert!(file.exists());
    assert_eq!(fixture.ids("hm_messages", "messageid").await, vec![1, 2]);
    repository.close().await.unwrap();
    fixture.finish().await;
}
