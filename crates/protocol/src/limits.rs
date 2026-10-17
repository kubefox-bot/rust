use hmail_cleaner_core::cleanup::{
    DEFAULT_BATCH_SIZE, DEFAULT_WORKERS, MAX_BATCH_SIZE, MAX_WORKERS,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
pub const PROTOCOL_VERSION: u16 = 1;
pub(crate) const MAX_PROFILE_TEXT_BYTES: usize = 4096;
pub(crate) const MAX_PASSWORD_BYTES: usize = 16 * 1024;
pub(crate) const MAX_UTC_OFFSET_SECONDS: i32 = 24 * 60 * 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RunLimits {
    pub batch_size: u32,
    pub workers: u8,
}
impl Default for RunLimits {
    fn default() -> Self {
        Self {
            batch_size: DEFAULT_BATCH_SIZE,
            workers: DEFAULT_WORKERS,
        }
    }
}
impl RunLimits {
    pub(crate) fn is_valid(self) -> bool {
        (1..=MAX_BATCH_SIZE).contains(&self.batch_size) && (1..=MAX_WORKERS).contains(&self.workers)
    }
}
