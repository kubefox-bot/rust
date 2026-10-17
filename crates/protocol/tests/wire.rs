use hmail_cleaner_protocol::{validate_job, Counter, JobCommand, ProtocolError, SummaryDto};
use serde_json::{json, Value};

fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/job-v1.json")).unwrap()
}

#[test]
fn windows_paths_survive_linux_validation() {
    let command: JobCommand = serde_json::from_value(fixture()).unwrap();
    let job = validate_job(command).unwrap();
    assert_eq!(job.command().mail_profile.data_dir, r"D:\Mail\Data");
    assert_eq!(job.command().resolved_cutoff.utc_offset_seconds, 10800);
}

#[test]
fn incompatible_protocol_and_invalid_parameters_are_rejected() {
    for (path, value) in [
        ("protocol_version", json!(2)),
        ("profile_version", json!("0")),
    ] {
        let mut input = fixture();
        input[path] = value;
        let command = serde_json::from_value(input).unwrap();
        assert!(validate_job(command).is_err());
    }
    let mut input = fixture();
    input["protocol_version"] = json!(2);
    assert!(matches!(
        validate_job(serde_json::from_value(input).unwrap()),
        Err(ProtocolError::UnsupportedVersion)
    ));
    for (batch, workers) in [(0, 2), (10001, 2), (1000, 0), (1000, 9)] {
        let mut input = fixture();
        input["limits"] = json!({"batch_size":batch,"workers":workers});
        assert!(validate_job(serde_json::from_value(input).unwrap()).is_err());
    }
}

#[test]
fn unknown_fields_and_noncanonical_ids_are_rejected_at_wire_boundary() {
    let mut input = fixture();
    input["unexpected"] = json!(true);
    assert!(serde_json::from_value::<JobCommand>(input).is_err());
    for id in [
        "00000000-0000-0000-0000-000000000000",
        "10000000000040008000000000000001",
        "BAD-ID",
    ] {
        let mut input = fixture();
        input["job_id"] = json!(id);
        assert!(serde_json::from_value::<JobCommand>(input).is_err());
    }
}

#[test]
fn cutoff_offset_and_preview_service_policy_must_be_consistent() {
    let mut input = fixture();
    input["resolved_cutoff"]["utc_offset_seconds"] = json!(0);
    assert!(validate_job(serde_json::from_value(input).unwrap()).is_err());
    let mut input = fixture();
    input["service_policy"] = json!("manage");
    assert!(validate_job(serde_json::from_value(input).unwrap()).is_err());
}

#[test]
fn wire_counters_are_lossless_and_reject_overflow() {
    let summary = SummaryDto {
        removed_bytes: Counter::from(9_007_199_254_740_993),
        ..SummaryDto::default()
    };
    let value = serde_json::to_value(summary).unwrap();
    assert_eq!(value["removed_bytes"], "9007199254740993");
    for input in [
        json!(1),
        json!("18446744073709551616"),
        json!("-1"),
        json!("01"),
        json!("+1"),
    ] {
        assert!(serde_json::from_value::<Counter>(input).is_err());
    }
    assert_eq!(
        serde_json::from_value::<Counter>(json!("18446744073709551615"))
            .unwrap()
            .get(),
        u64::MAX
    );
}

#[test]
fn generated_counter_schema_identifies_the_lossless_runtime_validator() {
    let schema = serde_json::to_value(schemars::schema_for!(Counter)).unwrap();
    assert_eq!(schema["type"], "string");
    assert_eq!(schema["format"], "uint64-decimal");
}

#[test]
fn v1_snapshot_fixture_preserves_large_counts_and_excludes_profile_secrets() {
    let snapshot: hmail_cleaner_protocol::JobSnapshot =
        serde_json::from_str(include_str!("fixtures/snapshot-v1.json")).unwrap();
    assert_eq!(snapshot.summary.selected_bytes.get(), 9_007_199_254_740_993);
    assert_eq!(snapshot.phase, hmail_cleaner_protocol::JobPhase::Completed);
    assert_eq!(snapshot.summary.removed_files.get(), 0);
    let encoded = serde_json::to_value(snapshot).unwrap();
    assert_eq!(encoded["summary"]["selected_bytes"], "9007199254740993");
    assert!(encoded["job"].get("mail_profile").is_none());
}

#[test]
fn public_error_rejects_raw_provider_messages() {
    let error =
        hmail_cleaner_protocol::PublicError::new(hmail_cleaner_protocol::ErrorCode::Unavailable);
    let mut encoded = serde_json::to_value(error).unwrap();
    encoded["message"] = json!("fixture mysql://name:password@private-host/database");
    assert!(serde_json::from_value::<hmail_cleaner_protocol::PublicError>(encoded).is_err());
}
