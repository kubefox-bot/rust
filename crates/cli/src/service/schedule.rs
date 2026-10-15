use chrono::{DateTime, NaiveDate, NaiveTime, TimeZone};
#[derive(Debug, thiserror::Error)]
#[error("cannot find a future local daily slot within the supported date range")]
pub(super) struct ScheduleError;
const SEARCH_DAYS: usize = 370;
pub(super) fn next_run<Tz: TimeZone>(
    now: DateTime<Tz>,
    at: NaiveTime,
    last_local_date: Option<NaiveDate>,
) -> Result<DateTime<Tz>, ScheduleError> {
    let zone = now.timezone();
    let mut date = now.date_naive();
    for _ in 0..SEARCH_DAYS {
        if last_local_date.is_none_or(|last| date > last) {
            if let Some(candidate) = zone.from_local_datetime(&date.and_time(at)).earliest() {
                if candidate > now {
                    return Ok(candidate);
                }
            }
        }
        date = date.succ_opt().ok_or(ScheduleError)?;
    }
    Err(ScheduleError)
}
#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Timelike};
    fn at() -> chrono::NaiveTime {
        chrono::NaiveTime::from_hms_opt(2, 30, 0).unwrap()
    }
    #[test]
    fn startup_waits_for_future_slot() {
        let now = chrono::Utc.with_ymd_and_hms(2026, 10, 15, 3, 0, 0).unwrap();
        assert_eq!(
            next_run(now, at(), None).unwrap(),
            chrono::Utc
                .with_ymd_and_hms(2026, 10, 16, 2, 30, 0)
                .unwrap()
        );
    }
    #[test]
    fn missed_days_are_not_replayed() {
        let now = chrono::Utc.with_ymd_and_hms(2026, 10, 15, 1, 0, 0).unwrap();
        let next = next_run(now, at(), chrono::NaiveDate::from_ymd_opt(2026, 10, 1)).unwrap();
        assert!(next > now);
        assert_eq!(next.date_naive(), now.date_naive());
    }
    #[test]
    fn repeated_hour_runs_once() {
        let zone = chrono_tz::America::New_York;
        let now = zone
            .with_ymd_and_hms(2026, 11, 1, 1, 45, 0)
            .earliest()
            .unwrap();
        let next = next_run(
            now,
            chrono::NaiveTime::from_hms_opt(1, 30, 0).unwrap(),
            None,
        )
        .unwrap();
        assert_eq!(
            next.date_naive(),
            chrono::NaiveDate::from_ymd_opt(2026, 11, 2).unwrap()
        );
    }
    #[test]
    fn nonexistent_hour_is_skipped() {
        let zone = chrono_tz::America::New_York;
        let now = zone.with_ymd_and_hms(2026, 3, 8, 0, 0, 0).unwrap();
        let next = next_run(now, at(), None).unwrap();
        assert_eq!(
            next.date_naive(),
            chrono::NaiveDate::from_ymd_opt(2026, 3, 9).unwrap()
        );
        assert_eq!(next.hour(), 2);
    }
    #[test]
    fn completed_date_has_no_second_slot() {
        let now = chrono::Utc.with_ymd_and_hms(2026, 10, 15, 1, 0, 0).unwrap();
        let next = next_run(now, at(), Some(now.date_naive())).unwrap();
        assert_ne!(next.date_naive(), now.date_naive());
    }
}
