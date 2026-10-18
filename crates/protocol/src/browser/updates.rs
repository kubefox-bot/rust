//! Public operational policy; no release trust roots or credentials are browser writable.
use serde::{Deserialize, Serialize};

pub const MIN_CHECK_INTERVAL_SECONDS: u32 = 30;
pub const MAX_CHECK_INTERVAL_SECONDS: u32 = 86_400;
pub const MIN_GRACEFUL_TIMEOUT_MINUTES: u32 = 1;
pub const MAX_GRACEFUL_TIMEOUT_MINUTES: u32 = 1_440;
const DEFAULT_CHECK_INTERVAL_SECONDS: u32 = 60;
const DEFAULT_GRACEFUL_TIMEOUT_MINUTES: u32 = 15;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdatePolicy {
    #[schemars(range(min = 30, max = 86400))]
    pub check_interval_seconds: u32,
    #[schemars(range(min = 1, max = 1440))]
    pub graceful_timeout_minutes: u32,
}
#[derive(Debug, thiserror::Error)]
#[error("update policy is outside supported bounds")]
pub struct InvalidUpdatePolicy;
impl Default for UpdatePolicy {
    fn default() -> Self {
        Self {
            check_interval_seconds: DEFAULT_CHECK_INTERVAL_SECONDS,
            graceful_timeout_minutes: DEFAULT_GRACEFUL_TIMEOUT_MINUTES,
        }
    }
}
impl UpdatePolicy {
    pub fn validate(&self) -> Result<(), InvalidUpdatePolicy> {
        if !(MIN_CHECK_INTERVAL_SECONDS..=MAX_CHECK_INTERVAL_SECONDS)
            .contains(&self.check_interval_seconds)
            || !(MIN_GRACEFUL_TIMEOUT_MINUTES..=MAX_GRACEFUL_TIMEOUT_MINUTES)
                .contains(&self.graceful_timeout_minutes)
        {
            return Err(InvalidUpdatePolicy);
        }
        Ok(())
    }
}
