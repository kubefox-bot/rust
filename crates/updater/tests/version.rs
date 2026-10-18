#![cfg(unix)]
#[test]
fn version_is_available_without_host_configuration() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_astra-updater"))
        .arg("version")
        .output()
        .unwrap();
    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["version"], 1);
    assert!(value["build"].is_string());
}
