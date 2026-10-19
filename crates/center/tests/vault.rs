use hmail_cleaner_center::vault::{
    self, AccountMaterial, EnvelopeHeader, RecordIdentity, RecordKind, RecoveryKey, SecretText,
    SystemEntropy,
};
use uuid::Uuid;
fn password() -> SecretText {
    SecretText::new("fixture-password-26!".into())
}
fn header(material: &AccountMaterial) -> EnvelopeHeader {
    EnvelopeHeader::record(RecordIdentity {
        vault_id: material.vault_id(),
        kind: RecordKind::Profile,
        record_id: Uuid::parse_str("20000000-0000-4000-8000-000000000002").unwrap(),
        record_version: 7,
        key_id: material.dek_id(),
        key_generation: 1,
    })
    .unwrap()
}
#[test]
fn password_recovery_and_record_contexts_are_independent_and_authenticated() {
    let prepared = vault::initialize(&password(), &SystemEntropy).unwrap();
    let from_password = vault::open_password(&prepared.material, &password()).unwrap();
    let from_recovery = vault::open_recovery(&prepared.material, &prepared.recovery).unwrap();
    assert!(from_password.as_slice() == from_recovery.as_slice());
    assert!(from_password.as_slice() == prepared.dek.as_slice());
    let encoded_recovery = prepared.recovery.expose();
    let parsed_recovery = RecoveryKey::parse(&encoded_recovery).unwrap();
    assert!(
        vault::open_recovery(&prepared.material, &parsed_recovery)
            .unwrap()
            .as_slice()
            == prepared.dek.as_slice()
    );
    assert!(
        RecoveryKey::parse(&SecretText::new(format!("{}=", encoded_recovery.as_str()))).is_err()
    );
    let mut changed_wrap = serde_json::to_value(&prepared.material).unwrap();
    changed_wrap["recovery_wrap"]["header"]["key_generation"] = serde_json::json!(2);
    let changed_wrap: AccountMaterial = serde_json::from_value(changed_wrap).unwrap();
    assert!(vault::open_recovery(&changed_wrap, &prepared.recovery).is_err());
    assert!(vault::open_password(
        &prepared.material,
        &SecretText::new("wrong-fixture-password".into())
    )
    .is_err());
    let serialized = serde_json::to_value(&prepared.material).unwrap();
    let verifier = serialized["verifier"].as_str().unwrap();
    assert!(vault::open_password(&prepared.material, &SecretText::new(verifier.into())).is_err());
    assert!(vault::open_recovery(
        &prepared.material,
        &RecoveryKey::generate(&SystemEntropy).unwrap()
    )
    .is_err());
    let identity = header(&prepared.material);
    let secret = b"fixture-profile-secret-not-for-sqlite";
    let encrypted = vault::seal_record(&prepared.dek, &identity, secret).unwrap();
    let stored = encrypted.encode().unwrap();
    let loaded = vault::EncryptedRecord::decode(&stored).unwrap();
    assert!(
        vault::open_record(&prepared.dek, &identity, &loaded)
            .unwrap()
            .as_slice()
            == secret
    );
    let account = AccountMaterial::decode(&prepared.material.encode().unwrap()).unwrap();
    assert!(vault::open_recovery(&account, &prepared.recovery).is_ok());
    let clear = vault::open_record(&prepared.dek, &identity, &encrypted).unwrap();
    assert!(clear.as_slice() == secret);
    let another = vault::seal_record(&prepared.dek, &identity, secret).unwrap();
    assert_ne!(
        serde_json::to_value(&encrypted).unwrap()["nonce"],
        serde_json::to_value(&another).unwrap()["nonce"]
    );
    let other_identity = EnvelopeHeader::record(RecordIdentity {
        vault_id: prepared.material.vault_id(),
        kind: RecordKind::CaKey,
        record_id: identity.record_id(),
        record_version: 7,
        key_id: prepared.material.dek_id(),
        key_generation: 1,
    })
    .unwrap();
    assert!(vault::open_record(&prepared.dek, &other_identity, &encrypted).is_err());
    let other_record = EnvelopeHeader::record(RecordIdentity {
        vault_id: prepared.material.vault_id(),
        kind: RecordKind::Profile,
        record_id: Uuid::new_v4(),
        record_version: 7,
        key_id: prepared.material.dek_id(),
        key_generation: 1,
    })
    .unwrap();
    assert!(vault::open_record(&prepared.dek, &other_record, &encrypted).is_err());
    let original = serde_json::to_value(&encrypted).unwrap();
    let mut corrupt = original.clone();
    let nonce = corrupt["nonce"][0].as_u64().unwrap() ^ 1;
    corrupt["nonce"][0] = serde_json::json!(nonce);
    let corrupt = serde_json::from_value(corrupt).unwrap();
    assert!(vault::open_record(&prepared.dek, &identity, &corrupt).is_err());
    let mut changed_tag = original.clone();
    let encoded = changed_tag["ciphertext"].as_str().unwrap();
    let index = encoded.len() - 4;
    let mut encoded = encoded.to_owned();
    let replacement = if encoded.as_bytes()[index] == b'A' {
        "B"
    } else {
        "A"
    };
    encoded.replace_range(index..index + 1, replacement);
    changed_tag["ciphertext"] = serde_json::json!(encoded);
    let changed_tag = serde_json::from_value(changed_tag).unwrap();
    assert!(vault::open_record(&prepared.dek, &identity, &changed_tag).is_err());
    let mut oversized = original.clone();
    oversized["ciphertext"] = serde_json::json!("A".repeat(128 * 1024));
    assert!(serde_json::from_value::<vault::EncryptedRecord>(oversized).is_err());
    let mut unsupported = original;
    unsupported["header"]["cipher"] = serde_json::json!(2);
    assert!(serde_json::from_value::<vault::EncryptedRecord>(unsupported).is_err());
    assert!(vault::seal_record(&prepared.dek, &identity, &vec![0; 64 * 1024 + 1]).is_err());
}

#[test]
fn serialized_metadata_is_bounded_before_json_parsing() {
    assert!(matches!(
        AccountMaterial::decode(&vec![b' '; 8193]),
        Err(vault::VaultError::Oversized)
    ));
    assert!(matches!(
        vault::EncryptedRecord::decode(&vec![b' '; 128 * 1024 + 1]),
        Err(vault::VaultError::Oversized)
    ));
}
#[test]
fn format_aad_has_a_fixed_big_endian_vector() {
    let id = |last| Uuid::parse_str(&format!("00000000-0000-4000-8000-{last:012}")).unwrap();
    let identity = EnvelopeHeader::record(RecordIdentity {
        vault_id: id(1),
        kind: RecordKind::Profile,
        record_id: id(2),
        record_version: 7,
        key_id: id(3),
        key_generation: 1,
    })
    .unwrap();
    let expected = concat!(
        "686d61696c2d636c65616e65722f6161642f31",
        "00010101",
        "00000000000040008000000000000001",
        "00000000000040008000000000000002",
        "0000000000000007",
        "00000000000040008000000000000003",
        "0000000000000001",
        "00"
    );
    let actual = vault::encode_aad(&identity)
        .unwrap()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(actual, expected);
}
#[test]
fn entropy_failure_does_not_produce_a_partial_vault() {
    struct Failure;
    impl vault::Entropy for Failure {
        fn fill(&self, _: &mut [u8]) -> Result<(), vault::VaultError> {
            Err(vault::VaultError::Entropy)
        }
    }
    assert!(vault::initialize(&password(), &Failure).is_err());
}
