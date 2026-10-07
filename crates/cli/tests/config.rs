use std::{fs, process::Command};
fn command() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_hmail-cleaner"));
    cmd.env_remove("DATABASE_URL");
    cmd.args([
        "preview",
        "--older-than",
        "30d",
        "--data-dir",
        "unused",
        "--hmail-ini",
        "unused",
        "--json",
    ]);
    cmd
}
#[test]
fn legacy_environment_is_not_loaded_by_the_binary() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join(".env"),
        "DATABASE_URL=invalid://fixture:secret-marker@localhost/hmaildb\n",
    )
    .unwrap();
    let output = command().current_dir(temp.path()).output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["type"], "result");
    assert_eq!(result["status"], "invalid");
    assert!(result["error"]
        .as_str()
        .unwrap()
        .contains("DATABASE_URL is required"));
}
#[test]
fn invalid_explicit_url_never_appears_in_diagnostics() {
    let temp = tempfile::tempdir().unwrap();
    let file = temp.path().join("explicit.env");
    fs::write(
        &file,
        "DATABASE_URL=mysql://fixture:secret-marker@localhost\n",
    )
    .unwrap();
    let output = command().arg("--env-file").arg(file).output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    for bytes in [&output.stdout, &output.stderr] {
        assert!(!String::from_utf8_lossy(bytes).contains("secret-marker"));
    }
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["status"], "invalid");
}
