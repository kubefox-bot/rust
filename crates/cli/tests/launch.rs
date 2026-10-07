use std::process::{Command, Stdio};
#[test]
fn no_terminal_no_implicit_menu() {
    let output = Command::new(env!("CARGO_BIN_EXE_hmail-cleaner"))
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(!String::from_utf8_lossy(&output.stderr).contains("Choose"));
}
#[test]
fn silent_rejects_json_and_menu() {
    for arguments in [
        vec!["preview", "--silence", "--json"],
        vec!["menu", "--silence"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_hmail-cleaner"))
            .args(arguments)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
    }
}
#[test]
fn silent_preview_has_no_console_output_and_preserves_env() {
    let temp = tempfile::tempdir().unwrap();
    let env = temp.path().join("private.env");
    let log = temp.path().join("clean.ndjson");
    let content=format!("DATABASE_URL='mysql://fixture:dummy@127.0.0.1:1/hmaildb'\nHMAIL_DATA_DIR='{}'\nHMAIL_INI='{}'\nHMAIL_OLDER_THAN=30d\nHMAIL_LOG_FILE='{}'\n",temp.path().display(),temp.path().join("missing.ini").display(),log.display());
    std::fs::write(&env, &content).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_hmail-cleaner"))
        .args(["preview", "--silence", "--env-file"])
        .arg(&env)
        .env_clear()
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
    assert!(std::fs::read_to_string(log)
        .unwrap()
        .contains("\"type\":\"result\""));
    assert_eq!(std::fs::read_to_string(env).unwrap(), content);
}
