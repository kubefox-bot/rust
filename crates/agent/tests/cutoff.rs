use chrono::{NaiveDateTime, TimeZone, Utc};
use hmail_cleaner_agent::cutoff::resolve;
use hmail_cleaner_protocol::RetentionRequest;
use std::num::NonZeroU32;
#[test]
fn calendar_month_at_end_of_month_preserves_local_and_utc_meaning() {
    let now = Utc.with_ymd_and_hms(2026, 3, 31, 12, 0, 0).unwrap();
    let zone = chrono_tz::Europe::Berlin;
    let cutoff = resolve(
        &zone,
        now,
        RetentionRequest::Months {
            count: NonZeroU32::new(1).unwrap(),
        },
    )
    .unwrap();
    assert_eq!(
        cutoff.local,
        NaiveDateTime::parse_from_str("2026-02-28T14:00:00", "%Y-%m-%dT%H:%M:%S").unwrap()
    );
    assert_eq!(
        cutoff.file_cutoff_utc,
        Utc.with_ymd_and_hms(2026, 2, 28, 13, 0, 0).unwrap()
    );
    assert_eq!(cutoff.utc_offset_seconds, 3600);
    assert_eq!(cutoff.resolved_at_utc, now);
}
#[test]
fn ambiguous_and_nonexistent_local_times_are_rejected() {
    for raw in ["2026-03-08T02:30:00", "2026-11-01T01:30:00"] {
        let local = NaiveDateTime::parse_from_str(raw, "%Y-%m-%dT%H:%M:%S").unwrap();
        assert!(resolve(
            &chrono_tz::America::New_York,
            Utc::now(),
            RetentionRequest::Before { local }
        )
        .is_err());
    }
}
