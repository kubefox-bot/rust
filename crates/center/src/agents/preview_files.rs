use super::{AgentClient, AgentError};
use hmail_cleaner_protocol::{JobId, PreviewFilesPage, PreviewFilesQuery};
use reqwest::Method;

const MAX_PREVIEW_PAGE_BYTES: usize = 1024 * 1024;

impl AgentClient {
    pub async fn preview_files(
        &self,
        id: JobId,
        query: &PreviewFilesQuery,
    ) -> Result<PreviewFilesPage, AgentError> {
        query.validate().map_err(|_| AgentError::Rejected)?;
        let page: PreviewFilesPage = self
            .control_bounded(
                self.request(Method::GET, &format!("/agent/v1/jobs/{id}/files"))
                    .query(query),
                MAX_PREVIEW_PAGE_BYTES,
            )
            .await?;
        page.validate(query, id).map_err(|_| AgentError::Protocol)?;
        Ok(page)
    }
}
