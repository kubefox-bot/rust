#![cfg(unix)]
use astra_updater::{
    delivery::{Descriptor, Image},
    discovery::eligible,
    journal::Status,
};
fn descriptor() -> Descriptor {
    Descriptor {
        format_version: 1,
        product: "astra-center".into(),
        channel: "main".into(),
        source_sha: "a".repeat(40),
        build: format!("git-{}", "a".repeat(40)),
        sequence: 3,
        image: Image {
            reference: format!("ghcr.io/kubefox-bot/astra-center@sha256:{}", "b".repeat(64)),
            digest: format!("sha256:{}", "b".repeat(64)),
            config_digest: format!("sha256:{}", "c".repeat(64)),
            platform: "linux/amd64".into(),
        },
        release_manifest_sha256: "d".repeat(64),
        updater_min: 1,
        updater_max: 1,
        schema_version: 2,
        protocol_min: 1,
        protocol_max: 1,
    }
}
#[test]
fn same_sequence_cannot_silently_change_installed_digest() {
    let descriptor = descriptor();
    let status = Status {
        installed_sequence: 3,
        installed_build: Some(descriptor.build.clone()),
        installed_digest: Some(format!("sha256:{}", "e".repeat(64))),
        ..Status::default()
    };
    assert!(eligible(&descriptor, &status).is_err());
}
#[test]
fn newer_release_is_eligible_except_quarantined_candidate() {
    let descriptor = descriptor();
    let mut status = Status::default();
    assert!(eligible(&descriptor, &status).unwrap());
    status.quarantined_digest = Some(descriptor.image.digest.clone());
    assert!(!eligible(&descriptor, &status).unwrap());
}
