use super::errors::ApiError;
use crate::execution::{AgentError, Executor, Worker};
use axum::{
    extract::{rejection::JsonRejection, State},
    Json,
};
use hmail_cleaner_adapters::offline::WindowsMailControl;
use hmail_cleaner_core::maintenance::mail_service::MailControl;
use hmail_cleaner_protocol::{ServiceStateRequest, ServiceStateResponse};
pub(super) async fn post<E: Executor>(
    State(worker): State<Worker<E>>,
    payload: Result<Json<ServiceStateRequest>, JsonRejection>,
) -> Result<Json<ServiceStateResponse>, ApiError> {
    let Json(request) = payload.map_err(|_| ApiError::bad_request())?;
    request.validate().map_err(|_| ApiError::bad_request())?;
    if request.agent_id != worker.agent_id() {
        return Err(ApiError::bad_request());
    }
    let control =
        WindowsMailControl::new(&request.service_name).map_err(|_| AgentError::Unavailable)?;
    observe(&control).await.map(Json).map_err(Into::into)
}
async fn observe(control: &(impl MailControl + Sync)) -> Result<ServiceStateResponse, AgentError> {
    let state = control.state().await.map_err(|_| AgentError::Unavailable)?;
    Ok(ServiceStateResponse {
        mail_state: state.into(),
        observed_at: chrono::Utc::now(),
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    use hmail_cleaner_core::{cleanup::PortResult, maintenance::mail_service::MailState};
    struct ReadOnly;
    impl MailControl for ReadOnly {
        async fn state(&self) -> PortResult<MailState> {
            Ok(MailState::Running)
        }
        async fn stop(&self) -> PortResult<()> {
            panic!("read-only observation must not stop")
        }
        async fn wait_for_stopped(&self, _: std::time::Duration) -> PortResult<()> {
            panic!("read-only observation must not wait")
        }
        async fn restore_running(&self, _: std::time::Duration) -> PortResult<()> {
            panic!("read-only observation must not start")
        }
    }
    #[tokio::test]
    async fn service_observation_needs_no_mysql_credentials_or_service_effects() {
        assert_eq!(
            observe(&ReadOnly).await.unwrap().mail_state,
            hmail_cleaner_protocol::ObservedMailState::Running
        );
    }
}
