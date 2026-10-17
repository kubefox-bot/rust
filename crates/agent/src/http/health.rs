use crate::execution::{Executor, Worker};
use axum::{extract::State, Json};
use hmail_cleaner_protocol::{AgentHealth, PROTOCOL_VERSION};
pub(super) async fn get<E: Executor>(State(worker): State<Worker<E>>) -> Json<AgentHealth> {
    Json(AgentHealth {
        agent_id: worker.agent_id(),
        version: env!("CARGO_PKG_VERSION").into(),
        protocol_min: PROTOCOL_VERSION,
        protocol_max: PROTOCOL_VERSION,
        stream_epoch: worker.epoch(),
        revision: 1.into(),
        healthy: worker.is_healthy(),
    })
}
