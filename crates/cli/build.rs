use std::{
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};
mod build_support;
fn revision() -> String {
    if let Ok(revision) = std::env::var("HMAIL_SOURCE_REVISION") {
        assert!(
            build_support::valid_source_revision(&revision),
            "invalid explicit source revision"
        );
        return revision;
    }
    let revision = Command::new("git")
        .args(["rev-parse", "--short=12", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .unwrap_or_else(|| "source".into());
    let dirty = Command::new("git")
        .args([
            "diff",
            "--quiet",
            "HEAD",
            "--",
            "../../crates",
            "../../Cargo.lock",
        ])
        .status()
        .is_ok_and(|s| !s.success());
    format!("{revision}{}", if dirty { "-dirty" } else { "" })
}
fn main() {
    println!("cargo:rerun-if-env-changed=HMAIL_BUILD_VERSION");
    println!("cargo:rerun-if-changed=build_support.rs");
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    if let Ok(reference) = Command::new("git")
        .args(["symbolic-ref", "--quiet", "HEAD"])
        .output()
    {
        if reference.status.success() {
            println!(
                "cargo:rerun-if-changed=../../.git/{}",
                String::from_utf8_lossy(&reference.stdout).trim()
            );
        }
    }
    for path in [
        "src",
        "../core/src",
        "../adapters/src",
        "../center/src",
        "../agent/src",
        "../protocol/src",
        "../runner/src",
        "../../Cargo.lock",
    ] {
        println!("cargo:rerun-if-changed={path}");
    }
    println!("cargo:rerun-if-env-changed=HMAIL_SOURCE_REVISION");
    let source_revision = revision();
    println!("cargo:rustc-env=HMAIL_SOURCE_REVISION={source_revision}");
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("build clock before epoch")
        .as_secs();
    if let Ok(version) = std::env::var("HMAIL_BUILD_VERSION") {
        assert!(
            !version.is_empty()
                && version.len() <= 128
                && version
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b".+_-".contains(&byte)),
            "invalid explicit build version"
        );
        println!("cargo:rustc-env=HMAIL_BUILD_VERSION={version}");
        return;
    }
    println!(
        "cargo:rustc-env=HMAIL_BUILD_VERSION={}+{}.{}",
        env!("CARGO_PKG_VERSION"),
        source_revision,
        timestamp
    );
}
