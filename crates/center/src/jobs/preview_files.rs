use super::{JobService, SubmitError};
use crate::{
    agents::{AgentClient, AgentError},
    auth::SessionLease,
    http::errors::ApiError,
};
use hmail_cleaner_protocol::{JobId, Operation, PreviewFilesPage, PreviewFilesQuery};

impl JobService {
    pub(crate) async fn preview_files(
        &self,
        id: JobId,
        query: PreviewFilesQuery,
        lease: &SessionLease,
    ) -> Result<PreviewFilesPage, ApiError> {
        query.validate().map_err(|_| ApiError::bad_request())?;
        let view = self.get(id, lease).await?;
        if view.intent.operation != Operation::Preview {
            return Err(ApiError::bad_request());
        }
        let selected = self.selected(&view, lease).await?;
        let client = AgentClient::new(&self.store, selected.credentials.clone(), lease).await?;
        // The file list is bound to the immutable historical job/profile, never the current selection.
        let snapshot = client.job(id).await?;
        super::receipt::validate(&view, &snapshot, &selected)?;
        let page = client
            .preview_files(id, &query)
            .await
            .map_err(|failure| match failure {
                AgentError::NotFound => ApiError::from(axum::http::StatusCode::NOT_IMPLEMENTED),
                failure => ApiError::from(failure),
            })?;
        if !lease.is_live() {
            return Err(SubmitError::Unauthorized.into());
        }
        if page
            .rows
            .iter()
            .any(|file| file.created_at >= view.intent.resolved_cutoff.file_cutoff_utc)
        {
            return Err(SubmitError::Protocol.into());
        }
        Ok(page)
    }
}
