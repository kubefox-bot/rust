#![cfg(unix)]
use astra_updater::config::Config;
use serde_json::json;
fn value() -> serde_json::Value {
    json!({"repository":"kubefox-bot/rust","image_repository":"ghcr.io/kubefox-bot/astra-center","state_dir":"/tmp/astra/state","staging_dir":"/tmp/astra/staging","backup_dir":"/tmp/astra/backups","unit_path":"/tmp/astra/units/hmail-center.container","socket_path":"/tmp/astra/control/control.sock","cosign":"/tmp/astra/cosign","trust_root":"/tmp/astra/trusted_root.json","credentials_file":"/tmp/astra/credentials.json","registry_authfile":"/tmp/astra/auth.json"})
}
#[test]
fn validates_host_boundaries_and_defaults() {
    let config: Config = serde_json::from_value(value()).unwrap();
    assert!(config.validate().is_ok());
    for key in ["state_dir", "unit_path", "socket_path", "cosign"] {
        let mut invalid = value();
        invalid[key] = json!("../elsewhere");
        assert!(serde_json::from_value::<Config>(invalid)
            .unwrap()
            .validate()
            .is_err());
    }
    for image in [
        "http://evil/image",
        "ghcr.io/other/image:latest",
        "ghcr.io/a/../b",
    ] {
        let mut invalid = value();
        invalid["image_repository"] = json!(image);
        assert!(serde_json::from_value::<Config>(invalid)
            .unwrap()
            .validate()
            .is_err());
    }
}
#[test]
fn credential_files_must_be_private_regular_files() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("credentials.json");
    std::fs::write(&path, b"{}").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(astra_updater::config::private_read(&path, 1024).is_err());
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    assert_eq!(
        astra_updater::config::private_read(&path, 1024).unwrap(),
        b"{}"
    );
    assert!(astra_updater::config::private_read(&path, 1).is_err());
    let link = dir.path().join("link");
    std::os::unix::fs::symlink(&path, &link).unwrap();
    assert!(astra_updater::config::private_read(&link, 1024).is_err());
}
