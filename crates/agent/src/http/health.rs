use crate::execution::{Executor, Worker};
use axum::{extract::State, Json};
use hmail_cleaner_protocol::AgentHealth;
pub(super) async fn get<E: Executor>(State(worker): State<Worker<E>>) -> Json<AgentHealth> {
    Json(worker.health())
}
