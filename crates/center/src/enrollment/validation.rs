use super::{remote::Bootstrap, AgentPackage, EnrollmentError};
use crate::{agents::AgentEndpoint, auth::SessionLease, storage::CenterStore};
use hmail_cleaner_protocol::browser::enrollment::{
    EnrollmentPhase, EnrollmentView, InstallRequest,
};
pub(super) fn install(input: &InstallRequest) -> Result<(String, u16), EnrollmentError> {
    if input.operation_id.is_nil()
        || input.name.trim().is_empty()
        || input.name.len() > 128
        || input.name.chars().any(char::is_control)
    {
        return Err(EnrollmentError::Invalid);
    }
    super::target::validate(&input.target)?;
    if input.legacy.as_ref().is_some_and(|legacy| {
        [&legacy.executable, &legacy.env_file]
            .iter()
            .any(|path| path.is_empty() || path.len() > 4096 || path.chars().any(char::is_control))
    }) {
        return Err(EnrollmentError::Invalid);
    }
    let endpoint =
        AgentEndpoint::try_from(input.endpoint.clone()).map_err(|_| EnrollmentError::Invalid)?;
    let url = url::Url::parse(endpoint.as_str()).map_err(|_| EnrollmentError::Invalid)?;
    let host = endpoint.host().map_err(|_| EnrollmentError::Invalid)?;
    if host != input.target.host {
        return Err(EnrollmentError::Invalid);
    }
    Ok((
        host,
        url.port_or_known_default()
            .ok_or(EnrollmentError::Invalid)?,
    ))
}
pub(super) fn authorize(lease: &SessionLease) -> Result<(), EnrollmentError> {
    if lease.is_live() {
        Ok(())
    } else {
        Err(EnrollmentError::Unauthorized)
    }
}
pub(super) async fn bootstrap(
    store: &CenterStore,
    view: &EnrollmentView,
    lease: &SessionLease,
) -> Result<Bootstrap, EnrollmentError> {
    if !lease.is_live() {
        return Err(EnrollmentError::Unauthorized);
    }
    let account = store
        .account()
        .await
        .map_err(|_| EnrollmentError::Storage)?
        .ok_or(EnrollmentError::Storage)?;
    Ok(Bootstrap {
        operation_id: view.id,
        agent_id: view.agent_id,
        center_id: account.material.vault_id(),
        sha256: view.expected_sha256.clone(),
    })
}
pub(super) fn view(input: &InstallRequest, package: &AgentPackage) -> EnrollmentView {
    EnrollmentView {
        id: input.operation_id,
        agent_id: input.agent_id,
        server_id: input.server_id,
        profile_id: input.profile_id,
        phase: EnrollmentPhase::Preparing,
        fingerprint: input.fingerprint.clone(),
        package_version: package.metadata().build.clone(),
        expected_sha256: package.metadata().sha256.clone(),
        slot_held: true,
    }
}
