use super::errors::ApiError;
use crate::execution::{Executor, Worker};
use axum::{
    extract::{rejection::JsonRejection, State},
    Json,
};
use hmail_cleaner_protocol::{ResolveRequest, ResolvedCutoff, PROTOCOL_VERSION};
pub(super) async fn post<E: Executor>(
    State(worker): State<Worker<E>>,
    request: Result<Json<ResolveRequest>, JsonRejection>,
) -> Result<Json<ResolvedCutoff>, ApiError> {
    let Json(request) = request.map_err(|_| ApiError::bad_request())?;
    if request.agent_id != worker.agent_id() || request.protocol_version != PROTOCOL_VERSION {
        return Err(ApiError::bad_request());
    }
    Ok(Json(crate::cutoff::resolve(
        &chrono::Local,
        chrono::Utc::now(),
        request.retention,
    )?))
}
