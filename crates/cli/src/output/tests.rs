use super::*;
use crate::commands::Report;
#[test]
fn silent_requires_log() {
    assert!(Output::open(
        OutputPolicy::Silent,
        None,
        Arc::new(Cancellation::default())
    )
    .is_err());
}
#[test]
fn journal_writes_result_without_url_secrets() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("clean.ndjson");
    let output = Output::open(
        OutputPolicy::Silent,
        Some(&path),
        Arc::new(Cancellation::default()),
    )
    .unwrap();
    output.finish(&Report::invalid("invalid DATABASE_URL"));
    let text = std::fs::read_to_string(path).unwrap();
    assert!(text.contains("\"type\":\"result\""));
    assert!(!text.contains("secret-marker"));
    assert!(!output.failed());
}
#[test]
fn rotation_keeps_three_archives() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("clean.ndjson");
    let mut log = journal::Journal::open_with_limit(&path, 8).unwrap();
    for _ in 0..6 {
        log.write(b"1234567\n").unwrap();
    }
    assert_eq!(std::fs::read_dir(temp.path()).unwrap().count(), 4);
    assert_eq!(std::fs::read(path).unwrap(), b"1234567\n");
}
#[test]
fn write_failure_requests_cancellation() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("clean.ndjson");
    std::fs::File::create(&path)
        .unwrap()
        .set_len(10 * 1024 * 1024)
        .unwrap();
    std::fs::create_dir(temp.path().join("clean.ndjson.3")).unwrap();
    let cancel = Arc::new(Cancellation::default());
    let output = Output::open(OutputPolicy::Silent, Some(&path), cancel.clone()).unwrap();
    output.finish(&Report::invalid("fixture"));
    assert!(output.failed());
    assert!(cancel.is_requested());
}
