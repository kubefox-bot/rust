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
        .contains("Не задан DATABASE_URL"));
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

#[test]
fn sibling_env_is_loaded_from_another_working_directory_and_override_wins() {
    let install = tempfile::tempdir().unwrap();
    let working = tempfile::tempdir().unwrap();
    let executable = install.path().join(if cfg!(windows) {
        "hmail-cleaner.exe"
    } else {
        "hmail-cleaner"
    });
    fs::copy(env!("CARGO_BIN_EXE_hmail-cleaner"), &executable).unwrap();
    let root = install.path().canonicalize().unwrap();
    fs::write(install.path().join("cleaner.env"),format!("DATABASE_URL=mysql://fixture:dummy@127.0.0.1:1/hmaildb\nHMAIL_DATA_DIR='{}'\nHMAIL_INI='{}'\nHMAIL_OLDER_THAN=2y\n",root.display(),root.join("missing.ini").display())).unwrap();
    let launch = |extra: &[&str]| {
        let output = Command::new(&executable)
            .args(["preview", "--json"])
            .args(extra)
            .env_clear()
            .current_dir(working.path())
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(!output.stdout.contains(&27));
        assert!(!String::from_utf8_lossy(&output.stdout).contains("\"type\":\"action\""));
        String::from_utf8(output.stdout)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
            .next_back()
            .unwrap()
    };
    let automatic = launch(&[]);
    assert!(!automatic["error"]
        .as_str()
        .unwrap()
        .contains("Не задан DATABASE_URL"));
    let alternate = install.path().join("alternate.env");
    fs::write(&alternate, "HMAIL_OLDER_THAN=2y\n").unwrap();
    let explicit = launch(&["--env-file", alternate.to_str().unwrap()]);
    assert!(explicit["error"]
        .as_str()
        .unwrap()
        .contains("Не задан DATABASE_URL"));
}
