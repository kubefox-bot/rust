/// Source-included build metadata API shared by build.rs and the standalone EXE packager.
pub fn valid_source_revision(value: &str) -> bool {
    let (commit, dirty) = value
        .split_once("-dirty")
        .map_or((value, None), |(commit, suffix)| (commit, Some(suffix)));
    let lowercase_hex = |text: &str| {
        text.bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    };
    let valid_suffix = dirty.is_none_or(|suffix| {
        suffix.is_empty()
            || suffix
                .strip_prefix('.')
                .is_some_and(|fingerprint| fingerprint.len() == 12 && lowercase_hex(fingerprint))
    });
    (12..=40).contains(&commit.len()) && lowercase_hex(commit) && valid_suffix
}
