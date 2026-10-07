use hmail_cleaner_adapters::offline::{verify_data_folder, WindowsOffline};
use hmail_cleaner_core::cleanup::OfflineGuard;
use std::fs;

#[test]
fn ini_root_mismatch_blocks_changes() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap().join("Data");
    fs::create_dir(&root).unwrap();
    let ini = temp.path().join("hMailServer.ini");
    fs::write(&ini, "[Directories]\nDataFolder=wrong-path\n").unwrap();
    assert!(verify_data_folder(&root, &ini).is_err());
    fs::write(
        &ini,
        format!("[Directories]\nDataFolder={}\n", root.display()),
    )
    .unwrap();
    verify_data_folder(&root, &ini).unwrap();
    fs::remove_dir(&root).unwrap();
    assert!(verify_data_folder(&root, &ini).is_err());
}
#[cfg(not(windows))]
#[tokio::test]
async fn production_clean_is_blocked_off_windows() {
    assert!(WindowsOffline::new("hMailServer".into())
        .ensure_stopped()
        .await
        .is_err());
}
