use super::*;
#[test]
fn encrypted_profiles_are_random_bound_expiring_and_tamper_resistant() {
    let tokens = ProfileTokens::new("operator".into()).unwrap();
    let token = tokens.token_at(42, 100).unwrap();
    let other = tokens.token_at(42, 100).unwrap();
    assert_ne!(token, other);
    assert!(!token.contains("operator"));
    let plaintext = tokens.cipher.open(&token).unwrap();
    let claims: serde_json::Value = serde_json::from_slice(&plaintext).unwrap();
    assert_eq!(claims["login"], "operator");
    assert!(claims.get("token").is_none());
    assert!(tokens.accepts_at(&token, 42, 100));
    assert!(tokens.accepts_at(&token, 42, 399));
    assert!(!tokens.accepts_at(&token, 42, 400));
    assert!(!tokens.accepts_at(&token, 42, 99));
    assert!(!tokens.accepts_at(&token, 43, 100));
    assert!(!ProfileTokens::new("operator".into())
        .unwrap()
        .accepts_at(&token, 42, 100));
    let mut tampered = token.into_bytes();
    tampered[10] = if tampered[10] == b'A' { b'B' } else { b'A' };
    assert!(!tokens.accepts_at(std::str::from_utf8(&tampered).unwrap(), 42, 100));
    for malformed in ["v1.", "legacy.jwt.token", &"x".repeat(3000)] {
        assert!(!tokens.accepts_at(malformed, 42, 100));
    }
}
