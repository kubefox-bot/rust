use super::control::Control;
use hmail_cleaner_core::cleanup::PortResult;
use std::time::{Duration, Instant};
use windows_service::service::ServiceState;
const POLL_INTERVAL: Duration = Duration::from_millis(250);
async fn pause(deadline: Instant) -> PortResult<()> {
    let remaining = deadline
        .checked_duration_since(Instant::now())
        .ok_or("mail service transition timed out")?;
    tokio::time::sleep(POLL_INTERVAL.min(remaining)).await;
    Ok(())
}
pub(super) async fn stopped(service: &Control, timeout: Duration) -> PortResult<()> {
    let deadline = Instant::now() + timeout;
    loop {
        match service.raw_state()? {
            ServiceState::Stopped => return Ok(()),
            ServiceState::StopPending => pause(deadline).await?,
            _ => return Err("unexpected service state while waiting for Stop".into()),
        }
    }
}
pub(super) async fn running(service: &Control, timeout: Duration) -> PortResult<()> {
    let deadline = Instant::now() + timeout;
    let mut start_sent = false;
    loop {
        if Instant::now() >= deadline {
            return Err("mail service restore timed out".into());
        }
        match service.raw_state()? {
            ServiceState::Running => return Ok(()),
            ServiceState::Stopped if !start_sent => {
                service.start()?;
                start_sent = true;
            }
            ServiceState::StopPending | ServiceState::StartPending => pause(deadline).await?,
            _ => return Err("unexpected service state during restore".into()),
        }
    }
}
