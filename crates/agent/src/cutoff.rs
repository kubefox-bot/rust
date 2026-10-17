use crate::execution::AgentError;
use chrono::{DateTime, Datelike, Offset, TimeZone, Utc};
use hmail_cleaner_core::selection::{self, Retention};
use hmail_cleaner_protocol::{ResolvedCutoff, RetentionRequest};

/// Uses the target agent's zone, rejecting DST gaps/overlaps rather than guessing.
pub fn resolve<T: TimeZone>(
    zone: &T,
    now: DateTime<Utc>,
    request: RetentionRequest,
) -> Result<ResolvedCutoff, AgentError> {
    let retention = match request {
        RetentionRequest::Days { count } => Retention::Days(count),
        RetentionRequest::Months { count } => Retention::Months(count),
        RetentionRequest::Years { count } => Retention::Years(count),
        RetentionRequest::Before { local } => Retention::Before(local),
    };
    let local = selection::cutoff(now.with_timezone(zone).naive_local(), retention)
        .map_err(|_| AgentError::Invalid)?;
    if !(1000..=9999).contains(&local.year()) {
        return Err(AgentError::Invalid);
    }
    let zoned = zone
        .from_local_datetime(&local)
        .single()
        .ok_or(AgentError::Invalid)?;
    Ok(ResolvedCutoff {
        local,
        file_cutoff_utc: zoned.with_timezone(&Utc),
        utc_offset_seconds: zoned.offset().fix().local_minus_utc(),
        resolved_at_utc: now,
    })
}
