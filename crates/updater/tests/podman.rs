#![cfg(unix)]
use astra_updater::podman::{candidate_unit, unit_identity};
#[test]
fn replaces_only_image_and_expected_build_in_existing_unit() {
    let old=format!("[Container]\nImage=ghcr.io/kubefox-bot/astra-center@sha256:{}\nReadOnly=true\nHealthCmd=[\"/usr/local/bin/hmail-cleaner\",\"center\",\"health\",\"--data-dir\",\"/data\",\"--expect-build\",\"old\"]\n","a".repeat(64));
    assert_eq!(unit_identity(&old).unwrap().build, "old");
    let image = format!("ghcr.io/kubefox-bot/astra-center@sha256:{}", "b".repeat(64));
    let next = candidate_unit(&old, &image, "next").unwrap();
    assert!(next.contains("ReadOnly=true"));
    assert_eq!(unit_identity(&next).unwrap().image, image);
    assert_eq!(unit_identity(&next).unwrap().build, "next");
    assert!(candidate_unit(&old, "evil:latest", "next").is_err());
    assert!(unit_identity(&(old.clone() + "Image=evil\n")).is_err());
}
