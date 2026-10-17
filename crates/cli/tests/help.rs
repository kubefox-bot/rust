#![cfg(feature = "local-cli")]
use std::process::Command;
#[test]
fn help_is_available_without_a_database() {
    let result = Command::new(env!("CARGO_BIN_EXE_hmail-cleaner"))
        .arg("--help")
        .env_remove("DATABASE_URL")
        .output()
        .unwrap();
    assert!(result.status.success());
    let output = String::from_utf8(result.stdout).unwrap();
    assert!(output.contains("preview"));
    assert!(output.contains("clean"));
}
