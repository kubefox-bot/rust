const MAX_SCALARS: usize = 64;
const MAX_UTF8_BYTES: usize = 256;

pub(crate) fn valid(login: &str) -> bool {
    !login.is_empty()
        && login.len() <= MAX_UTF8_BYTES
        && login.chars().count() <= MAX_SCALARS
        && login.trim() == login
        && !login.chars().any(char::is_control)
}

#[cfg(test)]
mod tests {
    use super::valid;

    #[test]
    fn scalar_and_byte_boundaries_preserve_normalization_and_whitespace_policy() {
        for login in [
            "a".into(),
            "я".repeat(40),
            "𐐀".repeat(64),
            "a\u{301}".repeat(32),
            "a b".into(),
        ] {
            assert!(valid(&login), "{login:?}");
        }
        for login in [
            "".into(),
            "a".repeat(65),
            "я".repeat(65),
            "𐐀".repeat(65),
            " admin".into(),
            "admin ".into(),
            "a\nb".into(),
            "a\0b".into(),
            "\u{2003}admin".into(),
        ] {
            assert!(!valid(&login), "{login:?}");
        }
    }
}
