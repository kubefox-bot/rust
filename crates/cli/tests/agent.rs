#![cfg(feature = "agent")]
use std::process::Command;

#[test]
fn agent_enrollment_keeps_the_key_local_and_preserves_it_on_retry() {
    let temp = tempfile::tempdir().unwrap();
    let data = temp.path().join("hMailCleanerAgent");
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_hmail-cleaner"))
            .args([
                "agent",
                "enroll",
                "--agent-id",
                "20000000-0000-4000-8000-000000000002",
                "--data-dir",
            ])
            .arg(&data)
            .output()
            .unwrap()
    };
    let first = run();
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let output = String::from_utf8(first.stdout).unwrap();
    assert!(output.contains("CERTIFICATE REQUEST"));
    assert!(!output.contains("PRIVATE KEY"));
    let key = std::fs::read(data.join("identity-key.pem")).unwrap();
    assert!(run().status.success());
    assert!(std::fs::read(data.join("identity-key.pem")).unwrap() == key);
}
#[test]
fn agent_configuration_failure_does_not_load_legacy_mail_credentials() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(
        temp.path().join("cleaner.env"),
        b"DATABASE_URL=mysql://fixture:never-print-fixture@127.0.0.1/mysql\n",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_hmail-cleaner"))
        .current_dir(temp.path())
        .args(["agent", "run", "--config", "missing.json"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(!String::from_utf8_lossy(&output.stderr).contains("never-print-fixture"));
    assert!(!temp.path().join("jobs.sqlite").exists());
}
#[test]
fn agent_version_reports_the_compiled_build_identifier() {
    let output = Command::new(env!("CARGO_BIN_EXE_hmail-cleaner"))
        .args(["agent", "--version"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains(env!("HMAIL_BUILD_VERSION")));
}
