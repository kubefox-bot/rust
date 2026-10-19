use std::collections::HashSet;

/// Allocate a readable URL identifier once; subsequent name edits do not regenerate it.
pub(crate) fn allocate(name: &str, reserved: &HashSet<String>) -> String {
    let words: Vec<String> = name
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(str::to_owned)
        .collect();
    let readable = words.join("-");
    let base = if readable.is_empty() {
        "server".to_owned()
    } else if uuid::Uuid::parse_str(&readable).is_ok() {
        format!("server-{readable}")
    } else {
        readable
    };
    if !reserved.contains(&base) {
        return base;
    }
    (2..=reserved.len() + 2)
        .map(|suffix| format!("{base}-{suffix}"))
        .find(|candidate| !reserved.contains(candidate))
        .expect("one more candidate than occupied aliases")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn readable_slugs_handle_duplicates_unicode_and_legacy_uuid_ambiguity() {
        let reserved = HashSet::from(["mail-one".into(), "mail-one-2".into()]);
        assert_eq!(allocate(" Mail One ", &reserved), "mail-one-3");
        assert_eq!(allocate("Почта № 2", &HashSet::new()), "почта-2");
        assert_eq!(allocate("---", &HashSet::new()), "server");
        assert_eq!(
            allocate("077e6c0f-4ff7-4b02-bb7f-63b169e64c43", &HashSet::new()),
            "server-077e6c0f-4ff7-4b02-bb7f-63b169e64c43"
        );
    }
}
