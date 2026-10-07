use super::*;
use crate::args::Args;
use chrono::NaiveDateTime;
use clap::Parser;
use std::fs;
fn args(extra: &[String]) -> Args {
    #[cfg(windows)]
    let (root, ini) = (r"C:\Data", r"C:\Bin\hMailServer.ini");
    #[cfg(not(windows))]
    let (root, ini) = ("/Data", "/Bin/hMailServer.ini");
    Args::try_parse_from(
        [
            "hmail-cleaner",
            "preview",
            "--data-dir",
            root,
            "--hmail-ini",
            ini,
            "--older-than",
            "30d",
        ]
        .into_iter()
        .map(str::to_owned)
        .chain(extra.iter().cloned()),
    )
    .unwrap()
}
fn now() -> NaiveDateTime {
    NaiveDateTime::parse_from_str("2026-10-07 12:00:00", "%Y-%m-%d %H:%M:%S").unwrap()
}
#[test]
fn legacy_env_is_not_auto_loaded() {
    assert!(load_with_database(&args(&[]), None, now()).is_err());
}
#[test]
fn explicit_env_file_is_supported() {
    let temp = tempfile::tempdir().unwrap();
    let file = temp.path().join("explicit.env");
    fs::write(
        &file,
        "DATABASE_URL=mysql://fixture:dummy@localhost/hmaildb\n",
    )
    .unwrap();
    let config = load_with_database(
        &args(&["--env-file".into(), file.to_string_lossy().into_owned()]),
        None,
        now(),
    )
    .unwrap();
    assert_eq!(
        config.options.cutoff,
        NaiveDateTime::parse_from_str("2026-09-07 12:00:00", "%Y-%m-%d %H:%M:%S").unwrap()
    );
}
#[test]
fn diagnostics_do_not_contain_secret_url() {
    let error = load_with_database(
        &args(&[]),
        Some("mysql://fixture:secret-marker@localhost"),
        now(),
    )
    .unwrap_err();
    assert!(!format!("{error:?} {error}").contains("secret-marker"));
    let config = load_with_database(
        &args(&[]),
        Some("mysql://fixture:secret-marker@localhost/hmaildb"),
        now(),
    )
    .unwrap();
    assert!(!format!("{config:?}").contains("secret-marker"));
}
#[test]
fn full_cutoff_rejects_dst_gap_and_overlap() {
    let zone = chrono_tz::America::New_York;
    for raw in ["2026-03-08 02:30:00", "2026-11-01 01:30:00"] {
        let local = NaiveDateTime::parse_from_str(raw, "%Y-%m-%d %H:%M:%S").unwrap();
        assert!(matches!(
            file_cutoff(&zone, local),
            Err(ConfigError::LocalTime)
        ));
    }
    assert!(file_cutoff(&zone, now()).is_ok());
}
