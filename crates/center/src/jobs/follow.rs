use super::JobService;
use crate::auth::SessionLease;
use hmail_cleaner_protocol::JobId;
use std::time::Duration;
const RECONCILE_INTERVAL: Duration = Duration::from_secs(5);
impl JobService {
    /// One bounded observer for the global slot. It only looks up receipts; it never dispatches.
    pub fn resume(&self, lease: &SessionLease) {
        let Ok(mut generation) = self.follower_generation.lock() else {
            return;
        };
        if *generation == Some(lease.generation()) || !lease.is_live() {
            return;
        }
        *generation = Some(lease.generation());
        drop(generation);
        let service = self.clone();
        let lease = lease.clone();
        tokio::spawn(async move {
            let mut revoked = lease.revocation();
            if !lease.is_live() {
                return;
            }
            let _permit = tokio::select! {biased;_=revoked.changed()=>return,permit=service.follower.clone().lock_owned()=>permit};
            let mut interval = tokio::time::interval(RECONCILE_INTERVAL);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                if !lease.is_live() {
                    break;
                }
                tokio::select! {biased;_=revoked.changed()=>break,_=interval.tick()=>()}
                let id = sqlx::query_scalar::<_, Option<String>>(
                    "SELECT job_id FROM job_slot WHERE singleton=1",
                )
                .fetch_one(&service.store.pool)
                .await;
                match id {
                    Ok(Some(id)) => {
                        let Ok(id) = serde_json::from_value::<JobId>(serde_json::Value::String(id))
                        else {
                            break;
                        };
                        let _ = service.reconcile(id, &lease).await;
                        let _ = service.watch(id, &lease).await;
                    }
                    Ok(None) => (),
                    Err(_) => continue,
                }
            }
        });
    }
}
