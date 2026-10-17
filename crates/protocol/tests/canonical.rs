use hmail_cleaner_protocol::{canonical_job, validate_job, JobCommand};
use serde_json::{json, Value};

#[test]
fn canonical_command_is_stable_and_includes_secret_profile_values() {
    let input: Value = serde_json::from_str(include_str!("fixtures/job-v1.json")).unwrap();
    let first = validate_job(serde_json::from_value::<JobCommand>(input.clone()).unwrap()).unwrap();
    let shuffled: Value = serde_json::from_str(&serde_json::to_string(&input).unwrap()).unwrap();
    let second = validate_job(serde_json::from_value(shuffled).unwrap()).unwrap();
    let bytes = canonical_job(&first);
    assert!(bytes.starts_with(b"hmail-cleaner/job/1"));
    assert_eq!(bytes, canonical_job(&second));
    let mut changed = input;
    changed["mail_profile"]["mysql"]["password"] = json!("different-fixture-secret");
    let third = validate_job(serde_json::from_value(changed).unwrap()).unwrap();
    assert_ne!(bytes, canonical_job(&third));
}
