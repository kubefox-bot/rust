use super::errors::ApiError;
use crate::execution::{Executor, Worker};
use axum::{
    extract::{rejection::JsonRejection, Path, State},
    Json,
};
use hmail_cleaner_protocol::{validate_job, JobCommand, JobId, JobSnapshot};

pub(super) async fn put<E: Executor>(
    State(worker): State<Worker<E>>,
    Path(id): Path<JobId>,
    payload: Result<Json<JobCommand>, JsonRejection>,
) -> Result<Json<JobSnapshot>, ApiError> {
    let Json(command) = payload.map_err(|_| ApiError::bad_request())?;
    if command.job_id != id {
        return Err(ApiError::bad_request());
    }
    let job = validate_job(command).map_err(|_| ApiError::bad_request())?;
    Ok(Json(worker.submit(job).await?))
}
pub(super) async fn get<E: Executor>(
    State(worker): State<Worker<E>>,
    Path(id): Path<JobId>,
) -> Result<Json<JobSnapshot>, ApiError> {
    worker
        .lookup(id)
        .await?
        .map(Json)
        .ok_or_else(ApiError::not_found)
}
pub(super) async fn cancel<E: Executor>(
    State(worker): State<Worker<E>>,
    Path(id): Path<JobId>,
) -> Result<Json<JobSnapshot>, ApiError> {
    Ok(Json(worker.cancel(id).await?))
}
