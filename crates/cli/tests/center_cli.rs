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

#[cfg(all(unix, feature = "local-cli"))]
#[tokio::test]
async fn center_run_handles_sigterm_and_sigint_with_a_successful_exit() {
    use hmail_cleaner_center::{
        account::{initialize, InitInput, SecretPrompt},
        storage::{CenterStore, DataDirectory},
        vault::SecretText,
    };
    use std::{
        process::{Child, Stdio},
        time::Duration,
    };
    struct Prompt;
    impl SecretPrompt for Prompt {
        fn password(&mut self) -> Result<SecretText, String> {
            Ok(SecretText::new("fixture-shutdown-password".into()))
        }
        fn confirm_recovery(&mut self, _: &SecretText) -> Result<bool, String> {
            Ok(true)
        }
    }
    struct OwnedChild(Child);
    impl Drop for OwnedChild {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let root = tempfile::tempdir().unwrap();
    let static_dir = tempfile::tempdir().unwrap();
    for page in [
        "index.html",
        "login/index.html",
        "history/index.html",
        "settings/index.html",
    ] {
        let path = static_dir.path().join(page);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            path,
            "<!doctype html><html lang=\"ru\"><title>Fixture</title></html>",
        )
        .unwrap();
    }
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    {
        let directory = DataDirectory::acquire(root.path()).unwrap();
        std::fs::write(
            root.path().join("center.json"),
            serde_json::to_vec(&serde_json::json!({
                "origin":format!("https://localhost:{}", address.port()),
                "listen":address.to_string(), "static_dir":static_dir.path(),
            }))
            .unwrap(),
        )
        .unwrap();
        let store = CenterStore::open(&directory).await.unwrap();
        initialize(
            &store,
            &InitInput {
                login: "admin".into(),
            },
            &mut Prompt,
        )
        .await
        .unwrap();
    }
    for signal in ["-TERM", "-INT"] {
        let child = Command::new(env!("CARGO_BIN_EXE_hmail-cleaner"))
            .args(["center", "run", "--data-dir"])
            .arg(root.path())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        let mut child = OwnedChild(child);
        tokio::time::timeout(Duration::from_secs(15), async {
            loop {
                let output = Command::new(env!("CARGO_BIN_EXE_hmail-cleaner"))
                    .args(["center", "health", "--data-dir"])
                    .arg(root.path())
                    .output()
                    .unwrap();
                if output.status.success() {
                    break;
                }
                assert!(
                    child.0.try_wait().unwrap().is_none(),
                    "center exited before becoming healthy"
                );
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        })
        .await
        .expect("center health deadline");
        assert!(Command::new("/bin/kill")
            .arg(signal)
            .arg(child.0.id().to_string())
            .status()
            .unwrap()
            .success());
        let status = tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                if let Some(status) = child.0.try_wait().unwrap() {
                    break status;
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        })
        .await
        .expect("center shutdown deadline");
        assert!(
            status.success(),
            "{signal} must reach graceful shutdown: {status}"
        );
    }
}

#[test]
fn operational_commands_refuse_missing_data_without_creating_a_database() {
    let root = tempfile::tempdir().unwrap();
    let missing = root.path().join("not-initialized");
    for operation in ["health", "backup"] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_hmail-cleaner"));
        command
            .args(["center", operation, "--data-dir"])
            .arg(&missing);
        if operation == "backup" {
            command
                .arg("--destination")
                .arg(root.path().join("snapshot"));
        }
        assert!(!command.output().unwrap().status.success());
        assert!(!missing.exists());
    }
    let rejected = Command::new(env!("CARGO_BIN_EXE_hmail-cleaner"))
        .args(["center", "health", "--data-dir"])
        .arg(&missing)
        .arg("--insecure")
        .output()
        .unwrap();
    assert!(!rejected.status.success());
    assert!(!missing.exists());
}
