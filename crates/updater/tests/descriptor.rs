#![cfg(unix)]
use astra_updater::delivery::Descriptor;
use serde_json::json;
fn sample() -> serde_json::Value {
    json!({"format_version":1,"product":"astra-center","channel":"main","source_sha":"a".repeat(40),"build":format!("git-{}","a".repeat(40)),"sequence":1,"image":{"reference":format!("ghcr.io/kubefox-bot/astra-center@sha256:{}","b".repeat(64)),"digest":format!("sha256:{}","b".repeat(64)),"config_digest":format!("sha256:{}","c".repeat(64)),"platform":"linux/amd64"},"release_manifest_sha256":"d".repeat(64),"updater_min":1,"updater_max":1,"schema_version":2,"protocol_min":1,"protocol_max":1})
}
#[test]
fn validates_full_release_identity_and_compatibility() {
    assert!(Descriptor::parse(&serde_json::to_vec(&sample()).unwrap()).is_ok());
    for (key, value) in [
        ("product", json!("other")),
        ("channel", json!("dev")),
        ("source_sha", json!("a")),
        ("build", json!("latest")),
        ("schema_version", json!(3)),
        ("schema_version", json!(1)),
        ("updater_min", json!(2)),
        ("sequence", json!(0)),
        ("protocol_min", json!(2)),
    ] {
        let mut input = sample();
        input[key] = value;
        assert!(
            Descriptor::parse(&serde_json::to_vec(&input).unwrap()).is_err(),
            "{key}"
        );
    }
    let mut input = sample();
    input["image"]["reference"] = json!("ghcr.io/evil/other@sha256:aaaa");
    assert!(Descriptor::parse(&serde_json::to_vec(&input).unwrap()).is_err());
}
