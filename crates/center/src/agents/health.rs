use super::AgentError;
use hmail_cleaner_protocol::{browser::agents::HealthView, AgentHealth, AgentId};
use std::{collections::HashMap, sync::Mutex, time::Duration};
const HEALTH_FRESHNESS: Duration = Duration::from_secs(30);
const MAX_OBSERVATIONS: usize = 1000;
struct Observation {
    health: AgentHealth,
    observed_at: chrono::DateTime<chrono::Utc>,
    at: Duration,
    reachable: bool,
}
pub struct HealthTracker(Mutex<HashMap<AgentId, Observation>>);
impl Default for HealthTracker {
    fn default() -> Self {
        Self::new()
    }
}
impl HealthTracker {
    pub fn new() -> Self {
        Self(Mutex::new(HashMap::new()))
    }
    pub fn observed(
        &self,
        health: AgentHealth,
        observed_at: chrono::DateTime<chrono::Utc>,
        at: Duration,
    ) -> Result<(), AgentError> {
        let mut observations = self.0.lock().map_err(|_| AgentError::Unavailable)?;
        if health.version.len() > 128
            || (!observations.contains_key(&health.agent_id)
                && observations.len() >= MAX_OBSERVATIONS)
        {
            return Err(AgentError::Protocol);
        }
        observations.insert(
            health.agent_id,
            Observation {
                health,
                observed_at,
                at,
                reachable: true,
            },
        );
        Ok(())
    }
    pub fn unreachable(&self, id: AgentId) -> Result<(), AgentError> {
        if let Some(observation) = self
            .0
            .lock()
            .map_err(|_| AgentError::Unavailable)?
            .get_mut(&id)
        {
            observation.reachable = false;
        }
        Ok(())
    }
    pub fn view(&self, id: AgentId, now: Duration) -> Result<HealthView, AgentError> {
        let observations = self.0.lock().map_err(|_| AgentError::Unavailable)?;
        let observation = observations.get(&id);
        let fresh = observation
            .is_some_and(|seen| seen.reachable && now.saturating_sub(seen.at) < HEALTH_FRESHNESS);
        Ok(HealthView {
            agent_id: id,
            online: fresh,
            stale: !fresh,
            healthy: observation
                .filter(|_| fresh)
                .map(|seen| seen.health.healthy),
            version: observation.map(|seen| seen.health.version.clone()),
            observed_at: observation.map(|seen| seen.observed_at),
        })
    }
}
