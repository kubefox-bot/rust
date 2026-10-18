use chrono::{DateTime, Datelike, Timelike, Utc, Weekday};
const START_HOUR_UTC: u32 = 3;
const END_HOUR_UTC: u32 = 4;
/// Preserve the bootstrap policy: Sunday 03:00 inclusive through 04:00 exclusive UTC.
pub(super) fn allows(now: DateTime<Utc>) -> bool {
    now.weekday() == Weekday::Sun && (START_HOUR_UTC..END_HOUR_UTC).contains(&now.hour())
}
