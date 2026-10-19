use super::errors::ApiError;
use crate::{
    execution::{Executor, Worker},
    jobs::JobStoreError,
};
use axum::{
    extract::{rejection::QueryRejection, Path, Query, State},
    response::{IntoResponse, Response},
};
use hmail_cleaner_protocol::{JobId, PreviewFilesQuery};

const MAX_PREVIEW_PAGE_BYTES: usize = 1024 * 1024;

pub(super) async fn get<E: Executor>(
    State(worker): State<Worker<E>>,
    Path(id): Path<JobId>,
    query: Result<Query<PreviewFilesQuery>, QueryRejection>,
) -> Result<Response, ApiError> {
    let Query(query) = query.map_err(|_| ApiError::bad_request())?;
    query.validate().map_err(|_| ApiError::bad_request())?;
    match worker.preview_files(id, &query).await {
        Ok(page) => {
            let bytes = serde_json::to_vec(&page).map_err(|_| ApiError::unavailable())?;
            if bytes.len() > MAX_PREVIEW_PAGE_BYTES {
                return Err(ApiError::unavailable());
            }
            Ok((
                [(axum::http::header::CONTENT_TYPE, "application/json")],
                bytes,
            )
                .into_response())
        }
        Err(crate::execution::AgentError::Storage(JobStoreError::NotFound)) => {
            Err(ApiError::not_found())
        }
        Err(crate::execution::AgentError::Storage(JobStoreError::InvalidTransition)) => {
            Err(ApiError::bad_request())
        }
        Err(crate::execution::AgentError::Storage(JobStoreError::PreviewUnavailable)) => {
            Err(ApiError(
                axum::http::StatusCode::GONE,
                hmail_cleaner_protocol::ErrorCode::Unavailable,
            ))
        }
        Err(error) => Err(error.into()),
    }
}
