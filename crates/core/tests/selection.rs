use chrono::NaiveDateTime;
use hmail_cleaner_core::selection::{cutoff, Retention};
use std::num::NonZeroU32;

fn date(value: &str) -> NaiveDateTime {
    NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S").unwrap()
}

#[test]
fn month_clamps_to_february() {
    assert_eq!(
        cutoff(
            date("2026-03-31 10:00:00"),
            Retention::Months(NonZeroU32::new(1).unwrap())
        )
        .unwrap(),
        date("2026-02-28 10:00:00")
    );
}

#[test]
fn year_clamps_leap_day() {
    assert_eq!(
        cutoff(
            date("2024-02-29 10:00:00"),
            Retention::Years(NonZeroU32::new(1).unwrap())
        )
        .unwrap(),
        date("2023-02-28 10:00:00")
    );
}

#[test]
fn days_preserve_local_time() {
    assert_eq!(
        cutoff(
            date("2026-10-07 12:00:00"),
            Retention::Days(NonZeroU32::new(30).unwrap())
        )
        .unwrap(),
        date("2026-09-07 12:00:00")
    );
}

#[test]
fn explicit_before_is_unchanged() {
    let boundary = date("2026-09-07 08:15:00");
    assert_eq!(
        cutoff(date("2026-10-07 12:00:00"), Retention::Before(boundary)).unwrap(),
        boundary
    );
}
