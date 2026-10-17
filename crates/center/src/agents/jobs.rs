use super::{AgentClient, AgentError};
use hmail_cleaner_protocol::{JobCommand, JobId, JobSnapshot};
use reqwest::Method;
impl AgentClient {
    pub async fn installation(
        &self,
    ) -> Result<hmail_cleaner_protocol::InstallationStatus, AgentError> {
        self.control(self.request(Method::GET, "/agent/v1/installation"))
            .await
    }
    pub async fn submit_job(&self, command: &JobCommand) -> Result<JobSnapshot, AgentError> {
        if command.agent_id != self.credentials.agent_id {
            return Err(AgentError::Protocol);
        }
        self.control(
            self.request(Method::PUT, &format!("/agent/v1/jobs/{}", command.job_id))
                .json(command),
        )
        .await
    }
    pub async fn job(&self, id: JobId) -> Result<JobSnapshot, AgentError> {
        self.control(self.request(Method::GET, &format!("/agent/v1/jobs/{id}")))
            .await
    }
    pub async fn cancel_job(&self, id: JobId) -> Result<JobSnapshot, AgentError> {
        self.control(
            self.request(Method::PUT, &format!("/agent/v1/jobs/{id}/cancel"))
                .json(&serde_json::json!({})),
        )
        .await
    }
}
