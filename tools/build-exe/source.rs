use super::settings::Settings;
use std::{io, process::Command};
pub(super) fn revision(settings: &Settings) -> io::Result<String> {
    if let Some(value) = &settings.source_revision {
        return Ok(value.clone());
    }
    let commit = Command::new("git")
        .args(["rev-parse", "--short=12", "HEAD"])
        .current_dir(&settings.root)
        .output()?;
    if !commit.status.success() {
        return Err(io::Error::other("Cannot resolve source revision"));
    }
    let hash = String::from_utf8(commit.stdout)
        .map_err(|_| io::Error::other("Invalid source revision"))?;
    let dirty = Command::new("git")
        .args(["diff", "--quiet", "HEAD"])
        .current_dir(&settings.root)
        .status()?;
    let suffix = match dirty.code() {
        Some(0) => "",
        Some(1) => "-dirty",
        _ => return Err(io::Error::other("Cannot inspect source state")),
    };
    let value = format!("{}{suffix}", hash.trim());
    if !super::build_identity::valid_source_revision(&value) {
        return Err(io::Error::other("Invalid source revision"));
    }
    Ok(value)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_release_revision_is_preserved_without_a_second_git_lookup() {
        let expected = "0123456789ab-dirty.abcdef012345";
        let settings = Settings {
            root: std::env::temp_dir(),
            docker_context: None,
            component: super::super::settings::Component::Agent,
            build_version: Some("0.1.0+fixture".into()),
            source_revision: Some(expected.into()),
        };
        assert_eq!(revision(&settings).unwrap(), expected);
    }
}
