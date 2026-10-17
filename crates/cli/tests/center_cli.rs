#![cfg(feature = "center")]
use std::process::Command;
#[test]
fn center_help_has_no_secret_arguments() {
    let output = Command::new(env!("CARGO_BIN_EXE_hmail-cleaner"))
        .args(["center", "init", "--help"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("--data-dir"));
    assert!(!text.contains("--password"));
    assert!(!text.contains("--recovery"));
}
#[test]
fn center_rejects_password_argument_before_opening_data_directory() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("not-created");
    let output = Command::new(env!("CARGO_BIN_EXE_hmail-cleaner"))
        .args(["center", "init", "--data-dir"])
        .arg(&path)
        .args(["--password", "invalid-argument"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(!path.exists());
}
#[test]
fn recovery_refuses_running_center_before_prompting_for_secrets() {
    let root = tempfile::tempdir().unwrap();
    let _lease = hmail_cleaner_center::storage::DataDirectory::acquire(root.path()).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_hmail-cleaner"))
        .args(["center", "recover", "--data-dir"])
        .arg(root.path())
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let error = String::from_utf8(output.stderr).unwrap();
    assert!(error.contains("already locked"));
    assert!(!root.path().join("center.sqlite").exists());
}
#[test]
fn recovery_help_never_accepts_secrets_in_argv() {
    let output = Command::new(env!("CARGO_BIN_EXE_hmail-cleaner"))
        .args(["center", "recover", "--help"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("--data-dir"));
    assert!(!text.contains("--password"));
    assert!(!text.contains("--recovery-key"));
}
