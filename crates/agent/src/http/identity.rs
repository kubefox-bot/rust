use super::errors::ApiError;
use crate::{
    execution::{AgentError, Executor, Worker},
    identity::IdentityManager,
};
use axum::{
    extract::{rejection::JsonRejection, State},
    Extension, Json,
};
use hmail_cleaner_protocol::{
    AgentCsr, AgentIdentityInstall, AgentIdentityRequest, IdentityReceipt, PROTOCOL_VERSION,
};
use std::sync::Arc;
pub(super) async fn csr(
    extension: Option<Extension<Arc<IdentityManager>>>,
    payload: Result<Json<AgentIdentityRequest>, JsonRejection>,
) -> Result<Json<AgentCsr>, ApiError> {
    let Extension(manager) = extension.ok_or(AgentError::Unavailable)?;
    let Json(request) = payload.map_err(|_| ApiError::bad_request())?;
    if request.protocol_version != PROTOCOL_VERSION || request.agent_id != manager.agent_id() {
        return Err(ApiError::bad_request());
    }
    Ok(Json(manager.csr()?))
}
pub(super) async fn install<E: Executor>(
    State(worker): State<Worker<E>>,
    extension: Option<Extension<Arc<IdentityManager>>>,
    payload: Result<Json<AgentIdentityInstall>, JsonRejection>,
) -> Result<Json<IdentityReceipt>, ApiError> {
    let Extension(manager) = extension.ok_or(AgentError::Unavailable)?;
    let Json(input) = payload.map_err(|_| ApiError::bad_request())?;
    Ok(Json(
        manager
            .install(input, move || worker.mark_unavailable())
            .await?,
    ))
}
