use hmail_cleaner_adapters::filesystem::LocalFiles;
#[cfg(unix)]
use hmail_cleaner_core::cleanup::FileFailureKind;
use hmail_cleaner_core::cleanup::{DeletionResult, FileStore, FileTarget};
use hmail_cleaner_core::maintenance::{FileScan, FullFileStore};
use std::{
    fs,
    path::{Path, PathBuf},
};
use tempfile::TempDir;

fn stored(root: &Path, id: i64) -> FileTarget {
    FileTarget {
        id,
        path: root.join(format!(
            "domain/account/AB/{{AB000000-0000-0000-0000-{id:012X}}}.eml"
        )),
    }
}

#[tokio::test]
async fn scan_yields_bounded_batches() {
    let (_temp, root) = setup();
    for id in 1..=5 {
        write(&stored(&root, id));
    }
    let files = LocalFiles::new(root).unwrap();
    let mut scan = files.open_scan().await.unwrap();
    let mut total = 0;
    loop {
        let batch = scan.next_batch(2).await.unwrap();
        assert!(batch.files.len() <= 2);
        total += batch.examined_files;
        if batch.finished {
            break;
        }
    }
    scan.close().await.unwrap();
    assert_eq!(total, 5);
}

#[tokio::test]
async fn scan_counts_fresh_and_referenced_files() {
    let (_temp, root) = setup();
    write(&stored(&root, 1));
    write(&stored(&root, 2));
    let files = LocalFiles::new(root).unwrap();
    assert_eq!(files.validate_tree().await.unwrap().examined_files, 2);
    let mut scan = files.open_scan().await.unwrap();
    assert_eq!(scan.next_batch(1000).await.unwrap().files.len(), 2);
    scan.close().await.unwrap();
}

#[tokio::test]
async fn public_and_root_queue_are_scanned() {
    let (_temp, root) = setup();
    write(&stored(&root, 1));
    let queue = root.join("{AB000000-0000-0000-0000-000000000002}.eml");
    fs::write(&queue, b"queue").unwrap();
    let public = root.join("#Public/AB/{AB000000-0000-0000-0000-000000000003}.eml");
    fs::create_dir_all(public.parent().unwrap()).unwrap();
    fs::write(&public, b"public").unwrap();
    let files = LocalFiles::new(root.clone()).unwrap();
    assert_eq!(files.validate_tree().await.unwrap().examined_files, 3);
    let mut scan = files.open_scan().await.unwrap();
    let batch = scan.next_batch(1000).await.unwrap();
    assert_eq!(batch.files.len(), 3);
    assert!(queue.exists() && public.exists());
    scan.close().await.unwrap();
    let public_files = batch
        .files
        .into_iter()
        .filter(|file| file.path == public)
        .collect();
    let outcomes = files.delete_orphans(public_files, 2).await.unwrap();
    assert_eq!(
        outcomes[0].result,
        DeletionResult::Removed { size_bytes: 6 }
    );
    assert!(!public.exists());
    assert!(queue.exists());
    assert!(stored(&root, 1).path.exists());
    let pruning = files
        .prune_empty(vec![public.parent().unwrap().to_path_buf()])
        .await
        .unwrap();
    assert_eq!(pruning.removed, 2);
    assert!(root.is_dir());
}

#[tokio::test]
async fn public_scan_accepts_extra_depth_and_arbitrary_bucket() {
    let (_temp, root) = setup();
    for relative in [
        "#Public/CD/{AB000000-0000-0000-0000-000000000001}.eml",
        "#Public/account/AB/{AB000000-0000-0000-0000-000000000002}.eml",
    ] {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, b"keep").unwrap();
    }
    let files = LocalFiles::new(root).unwrap();
    let mut scan = files.open_scan().await.unwrap();
    assert_eq!(scan.next_batch(1000).await.unwrap().files.len(), 2);
    scan.close().await.unwrap();
}

#[cfg(unix)]
#[tokio::test]
async fn public_symlink_is_not_followed() {
    use std::os::unix::fs::symlink;
    let (temp, root) = setup();
    let outside = temp.path().join("outside");
    let bucket = outside.join("AB");
    fs::create_dir_all(&bucket).unwrap();
    let path = bucket.join("{AB000000-0000-0000-0000-000000000001}.eml");
    fs::write(&path, b"keep").unwrap();
    symlink(outside, root.join("#Public")).unwrap();
    let files = LocalFiles::new(root).unwrap();
    let mut scan = files.open_scan().await.unwrap();
    assert!(scan.next_batch(1000).await.unwrap().files.is_empty());
    scan.close().await.unwrap();
    assert!(path.exists());
}

#[tokio::test]
async fn arbitrary_bucket_is_scanned() {
    let (_temp, root) = setup();
    let wrong = root.join("domain/account/CD/{AB000000-0000-0000-0000-000000000001}.eml");
    fs::create_dir_all(wrong.parent().unwrap()).unwrap();
    fs::write(&wrong, b"wrong bucket").unwrap();
    let files = LocalFiles::new(root).unwrap();
    let mut scan = files.open_scan().await.unwrap();
    assert_eq!(scan.next_batch(1000).await.unwrap().files.len(), 1);
    scan.close().await.unwrap();
    assert!(wrong.exists());
}

#[tokio::test]
async fn changed_observation_is_not_deleted() {
    let (_temp, root) = setup();
    let selected = stored(&root, 1);
    write(&selected);
    let files = LocalFiles::new(root).unwrap();
    let mut scan = files.open_scan().await.unwrap();
    let batch = scan.next_batch(1000).await.unwrap();
    scan.close().await.unwrap();
    fs::write(&selected.path, b"a changed larger message").unwrap();
    let outcomes = files.delete_orphans(batch.files, 2).await.unwrap();
    assert!(matches!(outcomes[0].result, DeletionResult::Failed(_)));
    assert!(selected.path.exists());
}

#[cfg(unix)]
#[tokio::test]
async fn inaccessible_tree_fails_precheck() {
    use std::os::unix::fs::PermissionsExt;
    let (_temp, root) = setup();
    let selected = stored(&root, 1);
    write(&selected);
    let parent = selected.path.parent().unwrap();
    fs::set_permissions(parent, fs::Permissions::from_mode(0o000)).unwrap();
    let result = LocalFiles::new(root.clone()).unwrap().validate_tree().await;
    fs::set_permissions(parent, fs::Permissions::from_mode(0o700)).unwrap();
    assert!(result.is_err());
    assert!(selected.path.exists());
}

#[tokio::test]
async fn scan_resources_are_closed_on_cancellation() {
    let (_temp, root) = setup();
    write(&stored(&root, 1));
    let files = LocalFiles::new(root.clone()).unwrap();
    let mut scan = files.open_scan().await.unwrap();
    scan.next_batch(1).await.unwrap();
    scan.close().await.unwrap();
    fs::remove_dir_all(&root).unwrap();
    assert!(!root.exists());
}

fn setup() -> (TempDir, PathBuf) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap().join("Data");
    fs::create_dir(&root).unwrap();
    (temp, root)
}
fn target(root: &Path, id: i64) -> FileTarget {
    FileTarget {
        id,
        path: root.join(format!("domain/account/AB/{id}.eml")),
    }
}
fn write(target: &FileTarget) {
    fs::create_dir_all(target.path.parent().unwrap()).unwrap();
    fs::write(&target.path, b"email").unwrap();
}

async fn delete(
    files: &LocalFiles,
    targets: Vec<FileTarget>,
    workers: u8,
) -> hmail_cleaner_core::cleanup::PortResult<Vec<hmail_cleaner_core::cleanup::FileOutcome>> {
    let observed = files.inspect(targets, workers).await?;
    files.delete(observed, workers).await
}

#[tokio::test]
async fn removed_file_is_verified_and_counted_by_size() {
    let (_temp, root) = setup();
    let selected = target(&root, 1);
    fs::create_dir_all(selected.path.parent().unwrap()).unwrap();
    fs::write(&selected.path, b"1234567").unwrap();
    let files = LocalFiles::new(root).unwrap();
    let outcomes = delete(&files, vec![selected.clone()], 2).await.unwrap();
    assert_eq!(
        outcomes[0].result,
        DeletionResult::Removed { size_bytes: 7 }
    );
    assert!(!selected.path.try_exists().unwrap());
}

#[tokio::test]
async fn removes_only_exact_targets() {
    let (_temp, root) = setup();
    let first = target(&root, 1);
    let second = target(&root, 2);
    write(&first);
    write(&second);
    let files = LocalFiles::new(root).unwrap();
    let outcomes = delete(&files, vec![first.clone()], 2).await.unwrap();
    assert_eq!(
        outcomes[0].result,
        DeletionResult::Removed { size_bytes: 5 }
    );
    assert!(!first.path.exists());
    assert!(second.path.exists());
}
#[tokio::test]
async fn missing_target_is_already_absent() {
    let (_temp, root) = setup();
    let files = LocalFiles::new(root.clone()).unwrap();
    assert_eq!(
        delete(&files, vec![target(&root, 1)], 2).await.unwrap()[0].result,
        DeletionResult::AlreadyAbsent
    );
}
#[tokio::test]
async fn missing_root_is_not_missing_message() {
    let (_temp, root) = setup();
    let files = LocalFiles::new(root.clone()).unwrap();
    fs::remove_dir(&root).unwrap();
    assert!(matches!(
        delete(&files, vec![target(&root, 1)], 2).await.unwrap()[0].result,
        DeletionResult::Failed(_)
    ));
    assert!(LocalFiles::new(root).is_err());
}
#[tokio::test]
async fn permission_error_is_failure() {
    let (_temp, root) = setup();
    let first = target(&root, 1);
    write(&first);
    let files = LocalFiles::new(root).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(
            first.path.parent().unwrap(),
            fs::Permissions::from_mode(0o500),
        )
        .unwrap();
    }
    #[cfg(windows)]
    let locked = {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_SHARE_READ: u32 = 1;
        const FILE_SHARE_WRITE: u32 = 2;
        fs::OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
            .open(&first.path)
            .unwrap()
    };
    let result = delete(&files, vec![first.clone()], 1).await.unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(
            first.path.parent().unwrap(),
            fs::Permissions::from_mode(0o700),
        )
        .unwrap();
    }
    #[cfg(windows)]
    drop(locked);
    #[cfg(unix)]
    assert!(
        matches!(&result[0].result,DeletionResult::Failed(failure) if failure.kind==FileFailureKind::AccessDenied)
    );
    #[cfg(windows)]
    assert!(matches!(&result[0].result, DeletionResult::Failed(_)));
    assert!(first.path.exists());
}
#[cfg(unix)]
#[tokio::test]
async fn symlink_chain_is_rejected() {
    use std::os::unix::fs::symlink;
    let (temp, root) = setup();
    let outside = temp.path().canonicalize().unwrap().join("outside");
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("1.eml"), b"keep").unwrap();
    fs::create_dir_all(root.join("domain/account")).unwrap();
    symlink(&outside, root.join("domain/account/AB")).unwrap();
    let files = LocalFiles::new(root.clone()).unwrap();
    let result = delete(&files, vec![target(&root, 1)], 1).await.unwrap();
    assert!(
        matches!(&result[0].result,DeletionResult::Failed(failure) if failure.kind==FileFailureKind::InvalidPath)
    );
    assert!(outside.join("1.eml").exists());
}
#[tokio::test]
async fn prunes_empty_parents_but_preserves_root() {
    let (_temp, root) = setup();
    let parent = root.join("domain/account/AB");
    fs::create_dir_all(&parent).unwrap();
    let files = LocalFiles::new(root.clone()).unwrap();
    let result = files
        .prune_empty(vec![parent.clone(), parent])
        .await
        .unwrap();
    assert_eq!(result.removed, 3);
    assert!(root.is_dir());
}
#[tokio::test]
async fn preserves_nonempty_and_hidden_entries() {
    let (_temp, root) = setup();
    let parent = root.join("domain/account/AB");
    fs::create_dir_all(&parent).unwrap();
    fs::write(parent.join(".keep"), b"keep").unwrap();
    let files = LocalFiles::new(root.clone()).unwrap();
    assert_eq!(
        files
            .prune_empty(vec![parent.clone()])
            .await
            .unwrap()
            .removed,
        0
    );
    assert!(parent.join(".keep").exists());
}
#[tokio::test]
async fn prune_has_no_global_tree_walk() {
    let (_temp, root) = setup();
    let touched = root.join("domain/account/AB");
    let unrelated = root.join("other/empty");
    fs::create_dir_all(&touched).unwrap();
    fs::create_dir_all(&unrelated).unwrap();
    let files = LocalFiles::new(root.clone()).unwrap();
    files.prune_empty(vec![touched]).await.unwrap();
    assert!(unrelated.is_dir());
    assert!(root.is_dir());
}
#[tokio::test]
async fn outside_root_is_rejected_for_delete_and_prune() {
    let (temp, root) = setup();
    let outside = temp.path().canonicalize().unwrap().join("outside");
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("1.eml"), b"keep").unwrap();
    let files = LocalFiles::new(root).unwrap();
    let result = delete(
        &files,
        vec![FileTarget {
            id: 1,
            path: outside.join("1.eml"),
        }],
        1,
    )
    .await
    .unwrap();
    assert!(matches!(result[0].result, DeletionResult::Failed(_)));
    assert!(!files
        .prune_empty(vec![outside.clone()])
        .await
        .unwrap()
        .failures
        .is_empty());
    assert!(outside.join("1.eml").exists());
}

#[tokio::test]
async fn changed_file_and_reappeared_absent_file_are_preserved() {
    let (_temp, root) = setup();
    let first = target(&root, 1);
    let absent = target(&root, 2);
    write(&first);
    let files = LocalFiles::new(root).unwrap();
    let observed = files
        .inspect(vec![first.clone(), absent.clone()], 2)
        .await
        .unwrap();
    fs::write(&first.path, b"a changed message after inspection").unwrap();
    write(&absent);
    let outcomes = files.delete(observed, 2).await.unwrap();
    assert!(outcomes
        .iter()
        .all(|file| matches!(file.result, DeletionResult::Failed(_))));
    assert!(first.path.exists());
    assert!(absent.path.exists());
}
