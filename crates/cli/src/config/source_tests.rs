use super::*;
use clap::Parser;
use std::collections::BTreeMap;
fn map(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}
fn base() -> BTreeMap<String, String> {
    #[cfg(windows)]
    let (data, ini) = (r"C:\Data", r"C:\Bin\hMailServer.ini");
    #[cfg(not(windows))]
    let (data, ini) = ("/Data", "/Bin/hMailServer.ini");
    map(&[
        ("DATABASE_URL", "mysql://fixture:dummy@localhost/hmaildb"),
        ("HMAIL_DATA_DIR", data),
        ("HMAIL_INI", ini),
        ("HMAIL_OLDER_THAN", "30d"),
    ])
}
fn cli(extra: &[&str]) -> Args {
    Args::try_parse_from(
        ["hmail-cleaner", "preview"]
            .into_iter()
            .chain(extra.iter().copied()),
    )
    .unwrap()
}
#[test]
fn env_supplies_paths_age_and_limits() {
    let mut file = base();
    file.extend(map(&[("HMAIL_WORKERS", "3"), ("HMAIL_BATCH_SIZE", "500")]));
    let s = load_with_sources(&cli(&[]), &BTreeMap::new(), &file).unwrap();
    assert_eq!((s.workers, s.batch_size.get()), (3, 500));
}
#[test]
fn cli_overrides_system_and_file() {
    let mut file = base();
    file.remove("HMAIL_OLDER_THAN");
    file.extend(map(&[
        ("HMAIL_BEFORE", "2020-01-01"),
        ("HMAIL_WORKERS", "2"),
        ("HMAIL_BATCH_SIZE", "1000"),
    ]));
    let system = map(&[("HMAIL_WORKERS", "3")]);
    let s = load_with_sources(
        &cli(&[
            "--workers",
            "4",
            "--batch-size",
            "500",
            "--older-than",
            "30d",
        ]),
        &system,
        &file,
    )
    .unwrap();
    assert_eq!((s.workers, s.batch_size.get()), (4, 500));
    let now = chrono::Local::now();
    let config = s
        .for_run(
            RunIntent {
                mode: crate::args::Mode::Preview,
                scan: s.scan,
                service_policy: ServicePolicy::RequireStopped,
            },
            now,
        )
        .unwrap();
    assert_eq!(
        config.options.cutoff,
        now.naive_local() - chrono::Duration::days(30)
    );
}
#[test]
fn system_overrides_file() {
    let mut file = base();
    file.extend(map(&[("HMAIL_WORKERS", "2")]));
    let s = load_with_sources(&cli(&[]), &map(&[("HMAIL_WORKERS", "3")]), &file).unwrap();
    assert_eq!(s.workers, 3);
}
#[test]
fn same_source_retention_pair_rejected() {
    let mut file = base();
    file.insert("HMAIL_BEFORE".into(), "2020-01-01".into());
    assert!(load_with_sources(&cli(&[]), &BTreeMap::new(), &file).is_err());
}
#[test]
fn missing_age_or_path_is_error() {
    for key in ["HMAIL_DATA_DIR", "HMAIL_INI", "HMAIL_OLDER_THAN"] {
        let mut file = base();
        file.remove(key);
        assert!(load_with_sources(&cli(&[]), &BTreeMap::new(), &file).is_err());
    }
}
#[test]
fn invalid_workers_or_batch_is_error() {
    for (key, value) in [
        ("HMAIL_WORKERS", "9"),
        ("HMAIL_WORKERS", "0"),
        ("HMAIL_BATCH_SIZE", "10001"),
        ("HMAIL_BATCH_SIZE", "0"),
        ("HMAIL_DATA_DIR", ""),
    ] {
        let mut file = base();
        file.insert(key.into(), value.into());
        assert!(load_with_sources(&cli(&[]), &BTreeMap::new(), &file).is_err());
    }
}
#[test]
fn duplicate_recognized_env_key_is_error() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("settings.env");
    std::fs::write(&path, "HMAIL_WORKERS=2\nHMAIL_WORKERS=3\n").unwrap();
    assert!(sources::read_file(&path).is_err());
}
#[test]
fn legacy_env_not_loaded() {
    assert!(load_with_sources(&cli(&[]), &BTreeMap::new(), &BTreeMap::new()).is_err());
}
#[test]
fn diagnostics_never_contain_url_secret() {
    let mut file = base();
    file.insert(
        "DATABASE_URL".into(),
        "mysql://fixture:secret-marker@localhost".into(),
    );
    let err = load_with_sources(&cli(&[]), &BTreeMap::new(), &file).unwrap_err();
    assert!(!format!("{err:?} {err}").contains("secret-marker"));
}
#[test]
fn each_run_recomputes_retention_without_mutating_settings() {
    let settings = load_with_sources(&cli(&[]), &BTreeMap::new(), &base()).unwrap();
    let revised = settings.with_retention(parse_age("1mo").unwrap());
    let intent = RunIntent {
        mode: crate::args::Mode::Preview,
        scan: settings.scan,
        service_policy: ServicePolicy::RequireStopped,
    };
    let first = chrono::Local
        .with_ymd_and_hms(2026, 10, 7, 12, 0, 0)
        .single()
        .unwrap();
    let later = chrono::Local
        .with_ymd_and_hms(2026, 11, 7, 12, 0, 0)
        .single()
        .unwrap();
    assert_eq!(
        revised.for_run(intent, first).unwrap().options.cutoff,
        chrono::NaiveDate::from_ymd_opt(2026, 9, 7)
            .unwrap()
            .and_hms_opt(12, 0, 0)
            .unwrap()
    );
    assert_eq!(
        revised.for_run(intent, later).unwrap().options.cutoff,
        chrono::NaiveDate::from_ymd_opt(2026, 10, 7)
            .unwrap()
            .and_hms_opt(12, 0, 0)
            .unwrap()
    );
    assert!(matches!(
        settings.retention,
        hmail_cleaner_core::selection::Retention::Days(_)
    ));
}
