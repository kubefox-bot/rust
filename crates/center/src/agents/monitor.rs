use super::{AgentClient, AgentError, HealthTracker};
use crate::{auth::SessionLease, profiles::ProfileService, storage::CenterStore};
use futures_util::{stream, StreamExt};
use hmail_cleaner_protocol::{
    browser::{agents::HealthView, profiles::ServerView},
    AgentId,
};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;
const REFRESH_INTERVAL: Duration = Duration::from_secs(15);
const PROBE_TIMEOUT: Duration = Duration::from_secs(5);
const CONCURRENT_PROBES: usize = 2;
#[derive(Clone)]
pub(crate) struct HealthMonitor(Arc<State>);
struct State {
    store: CenterStore,
    tracker: HealthTracker,
    poll: Arc<Mutex<Option<Duration>>>,
    started: Instant,
}
impl HealthMonitor {
    pub fn new(store: CenterStore) -> Self {
        Self(Arc::new(State {
            store,
            tracker: HealthTracker::new(),
            poll: Arc::new(Mutex::new(None)),
            started: Instant::now(),
        }))
    }
    pub fn view(&self, id: AgentId) -> Result<HealthView, AgentError> {
        self.0.tracker.view(id, self.0.started.elapsed())
    }
    pub fn refresh(&self, servers: &[ServerView], lease: &SessionLease) -> bool {
        let Ok(mut permit) = self.0.poll.clone().try_lock_owned() else {
            return false;
        };
        let now = self.0.started.elapsed();
        if permit.is_some_and(|last| now.saturating_sub(last) < REFRESH_INTERVAL)
            || !lease.is_live()
        {
            return false;
        }
        *permit = Some(now);
        let targets = servers
            .iter()
            .map(|server| (server.id, server.agent_id))
            .collect::<Vec<_>>();
        let state = self.0.clone();
        let lease = lease.clone();
        tokio::spawn(async move {
            let _permit = permit;
            let mut revoked = lease.revocation();
            if !lease.is_live() {
                return;
            }
            let probes = stream::iter(targets)
                .map(|(server_id, agent_id)| {
                    let state = state.clone();
                    let lease = lease.clone();
                    async move {
                        let operation = async {
                            let credentials = ProfileService::new(state.store.clone())
                                .credentials_for_server(server_id, &lease)
                                .await
                                .map_err(|_| AgentError::Protocol)?;
                            if credentials.agent_id != agent_id {
                                return Err(AgentError::Protocol);
                            }
                            let client =
                                AgentClient::new(&state.store, credentials, &lease).await?;
                            client.health().await
                        };
                        match tokio::time::timeout(PROBE_TIMEOUT, operation).await {
                            Ok(Ok(health)) => {
                                let _ = state.tracker.observed(
                                    health,
                                    chrono::Utc::now(),
                                    state.started.elapsed(),
                                );
                            }
                            Ok(Err(_)) | Err(_) => {
                                let _ = state.tracker.unreachable(agent_id);
                            }
                        }
                    }
                })
                .buffer_unordered(CONCURRENT_PROBES)
                .collect::<Vec<_>>();
            tokio::select! {biased;_ = revoked.changed()=>(),_ = probes=>()}
        });
        true
    }
}
