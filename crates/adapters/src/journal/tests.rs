use super::*;
#[test]
fn physical_rotation_and_restart_append_keep_five_total_files() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("runtime.ndjson");
    {
        let mut writer = RotatingFile::open(&path, 8, 5).unwrap();
        writer.write(b"first\n").unwrap();
    }
    let mut writer = RotatingFile::open(&path, 8, 5).unwrap();
    writer.write(b"x\n").unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"first\nx\n");
    for value in [
        b"1111111\n",
        b"2222222\n",
        b"3333333\n",
        b"4444444\n",
        b"5555555\n",
        b"6666666\n",
    ] {
        writer.write(value).unwrap();
    }
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 5);
    assert_eq!(std::fs::read(&path).unwrap(), b"6666666\n");
    assert_eq!(
        std::fs::read(root.path().join("runtime.ndjson.4")).unwrap(),
        b"2222222\n"
    );
    for entry in std::fs::read_dir(root.path()).unwrap() {
        assert!(entry.unwrap().metadata().unwrap().len() <= 8);
    }
}
#[test]
fn utf8_record_cap_rejects_before_writing_and_latches_failure() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("runtime.ndjson");
    let writer = RuntimeJournal::open(&path).unwrap();
    writer.record(&"ж".repeat(900)).unwrap();
    let before = std::fs::read(&path).unwrap();
    assert!(before.len() <= 2048);
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&before).unwrap()["event"],
        "ж".repeat(900)
    );
    assert!(writer.record(&"ж".repeat(1024)).is_err());
    assert!(!writer.clone().is_healthy());
    assert!(writer.record(&"rejected after failure").is_err());
    assert_eq!(std::fs::read(&path).unwrap(), before);
}
#[test]
fn rotation_failure_is_explicit_without_claiming_the_record_was_written() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("runtime.ndjson");
    let writer = RuntimeJournal::with_policy(&path, 100, 5).unwrap();
    writer.record(&"first").unwrap();
    std::fs::create_dir(root.path().join("runtime.ndjson.4")).unwrap();
    assert!(writer.record(&"second record forces rotation").is_err());
    assert!(!writer.is_healthy());
    assert!(!std::fs::read_to_string(path).unwrap().contains("second"));
}
#[test]
fn directory_and_missing_parent_are_explicit_startup_failures() {
    let root = tempfile::tempdir().unwrap();
    assert!(RuntimeJournal::open(root.path()).is_err());
    assert!(RuntimeJournal::open(&root.path().join("missing/runtime.ndjson")).is_err());
}
