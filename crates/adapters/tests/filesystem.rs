use hmail_cleaner_adapters::filesystem::LocalFiles;
#[cfg(unix)]
use hmail_cleaner_core::cleanup::FileFailureKind;
use hmail_cleaner_core::cleanup::{DeletionResult, FileStore, FileTarget};
use std::{
    fs,
    path::{Path, PathBuf},
};
use tempfile::TempDir;

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

#[tokio::test]
async fn removed_file_is_verified_and_counted_by_size() {
    let (_temp, root) = setup();
    let selected = target(&root, 1);
    fs::create_dir_all(selected.path.parent().unwrap()).unwrap();
    fs::write(&selected.path, b"1234567").unwrap();
    let files = LocalFiles::new(root).unwrap();
    let outcomes = files.delete(vec![selected.clone()], 2).await.unwrap();
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
    let outcomes = files.delete(vec![first.clone()], 2).await.unwrap();
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
        files.delete(vec![target(&root, 1)], 2).await.unwrap()[0].result,
        DeletionResult::AlreadyAbsent
    );
}
#[tokio::test]
async fn missing_root_is_not_missing_message() {
    let (_temp, root) = setup();
    let files = LocalFiles::new(root.clone()).unwrap();
    fs::remove_dir(&root).unwrap();
    assert!(matches!(
        files.delete(vec![target(&root, 1)], 2).await.unwrap()[0].result,
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
    let result = files.delete(vec![first.clone()], 1).await.unwrap();
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
    let result = files.delete(vec![target(&root, 1)], 1).await.unwrap();
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
    let result = files
        .delete(
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
