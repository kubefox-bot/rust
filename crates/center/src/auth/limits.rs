use super::Clock;
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
const CAPACITY: u32 = 6;
const REFILL_SECONDS: u64 = 10;
struct Bucket {
    tokens: u32,
    last: Duration,
}
pub struct LoginLimiter {
    state: Mutex<Bucket>,
    clock: Arc<dyn Clock>,
}
impl LoginLimiter {
    pub fn new(clock: Arc<dyn Clock>) -> Self {
        Self {
            state: Mutex::new(Bucket {
                tokens: CAPACITY,
                last: clock.now(),
            }),
            clock,
        }
    }
    pub fn admit(&self) -> bool {
        let Ok(mut bucket) = self.state.lock() else {
            return false;
        };
        let now = self.clock.now();
        let elapsed = now.saturating_sub(bucket.last).as_secs() / REFILL_SECONDS;
        if elapsed > 0 {
            bucket.tokens = CAPACITY.min(
                bucket
                    .tokens
                    .saturating_add(elapsed.min(u64::from(CAPACITY)) as u32),
            );
            bucket.last += Duration::from_secs(elapsed * REFILL_SECONDS);
        }
        if bucket.tokens == 0 {
            return false;
        }
        bucket.tokens -= 1;
        true
    }
}
