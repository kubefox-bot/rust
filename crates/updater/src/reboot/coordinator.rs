use super::{
    grant::{self, Grant, Phase},
    window, GRANT_FILENAME,
};
use crate::{
    config::private_directory,
    control::{self, State as CenterState},
    journal::{self, OperationLock},
    Error,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Arc};
use tokio::sync::Mutex;
use uuid::Uuid;
const REBOOT_GROUP: &str = "astra";
const OPERATION_LOCK: &str = "operation.lock";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClientIdentity {
    pub id: Uuid,
    pub group: String,
}
pub trait Clock: Send + Sync {
    fn now(&self) -> DateTime<Utc>;
}
struct State {
    grant: Option<Grant>,
    lease: Option<OperationLock>,
}
pub struct Coordinator {
    state_dir: PathBuf,
    socket: PathBuf,
    client: ClientIdentity,
    boot_id: Uuid,
    clock: Arc<dyn Clock>,
    state: Mutex<State>,
}
impl Coordinator {
    pub fn new(
        state_dir: PathBuf,
        socket: PathBuf,
        client: ClientIdentity,
        boot_id: Uuid,
        clock: Arc<dyn Clock>,
    ) -> Result<Self, Error> {
        if client.id.is_nil() || client.group != REBOOT_GROUP || boot_id.is_nil() {
            return Err(Error::Invalid);
        }
        private_directory(&state_dir)?;
        let grant = grant::load(&state_dir.join(GRANT_FILENAME))?;
        Ok(Self {
            state_dir,
            socket,
            client,
            boot_id,
            clock,
            state: Mutex::new(State { grant, lease: None }),
        })
    }
    pub(super) fn authorized(&self, id: &str, group: &str) -> bool {
        id == self.client.id.to_string() && group == self.client.group
    }
    pub(super) async fn pre_reboot(&self) -> Result<(), Error> {
        let mut state = self.state.lock().await;
        if state
            .grant
            .as_ref()
            .is_some_and(|grant| grant.boot_id != self.boot_id)
        {
            return Err(Error::Conflict);
        }
        if state.grant.is_none() {
            if !window::allows(self.clock.now()) {
                return Err(Error::Conflict);
            }
            let lease = OperationLock::acquire(&self.state_dir.join(OPERATION_LOCK))?;
            let status = control::request(&self.socket, "policy", None).await?;
            // Never claim a marker owned by application-update recovery.
            if status.status != CenterState::Idle {
                return Err(Error::Attention);
            }
            let grant = Grant {
                version: 1,
                operation: Uuid::new_v4(),
                boot_id: self.boot_id,
                phase: Phase::Pending,
            };
            journal::save(&self.state_dir.join(GRANT_FILENAME), &grant)?;
            state.grant = Some(grant);
            state.lease = Some(lease);
        } else if state.lease.is_none() {
            state.lease = Some(OperationLock::acquire(
                &self.state_dir.join(OPERATION_LOCK),
            )?);
        }
        let current = state.grant.as_ref().ok_or(Error::Invalid)?.clone();
        if current.phase == Phase::Pending && !window::allows(self.clock.now()) {
            self.release(&mut state).await?;
            return Err(Error::Conflict);
        }
        let status = control::request(&self.socket, "begin", Some(current.operation)).await?;
        if status.status != CenterState::Ready
            || status.active_jobs != 0
            || status.active_enrollments != 0
            || status.active_writers != 0
        {
            return Err(Error::Conflict);
        }
        // A persisted grant precedes HTTP 200. Keep both center admission and host lock closed.
        let granted = Grant {
            phase: Phase::Granted,
            ..current
        };
        journal::save(&self.state_dir.join(GRANT_FILENAME), &granted)?;
        state.grant = Some(granted);
        Ok(())
    }
    pub(super) async fn steady_state(&self) -> Result<(), Error> {
        let mut state = self.state.lock().await;
        if state.grant.is_none() {
            return Ok(());
        }
        if state.lease.is_none() {
            state.lease = Some(OperationLock::acquire(
                &self.state_dir.join(OPERATION_LOCK),
            )?);
        }
        self.release(&mut state).await
    }
    async fn release(&self, state: &mut State) -> Result<(), Error> {
        let current = state.grant.as_ref().ok_or(Error::Invalid)?;
        let status = control::request(&self.socket, "finish", Some(current.operation)).await?;
        if status.status != CenterState::Idle {
            return Err(Error::Attention);
        }
        grant::remove(&self.state_dir.join(GRANT_FILENAME))?;
        state.grant = None;
        state.lease = None;
        Ok(())
    }
}
