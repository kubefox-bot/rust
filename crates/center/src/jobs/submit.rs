use super::{JobService, SubmitError};
use crate::{agents::AgentClient, auth::SessionLease, profiles::ProfileService};
use hmail_cleaner_protocol::{
    browser::jobs::{JobView, SubmitRequest},
    validate_job, JobCommand, Operation, ServicePolicy, PROTOCOL_VERSION,
};
impl JobService {
    pub async fn submit(
        &self,
        lease: &SessionLease,
        input: SubmitRequest,
    ) -> Result<JobView, SubmitError> {
        let maintenance = self
            .store
            .maintenance()
            .admit()
            .map_err(|_| SubmitError::Busy)?;
        if !lease.is_live() {
            return Err(SubmitError::Unauthorized);
        }
        match self.get(input.job_id, lease).await {
            Ok(existing) => {
                return if existing.intent == input {
                    Ok(existing)
                } else {
                    Err(SubmitError::Conflict)
                }
            }
            Err(SubmitError::NotFound) => (),
            Err(error) => return Err(error),
        }
        let permit = self
            .operation
            .clone()
            .try_lock_owned()
            .map_err(|_| SubmitError::Busy)?;
        let service = self.clone();
        let lease = lease.clone();
        tokio::spawn(async move {
            let _maintenance = maintenance;
            let _permit = permit;
            service.submit_owned(&lease, input).await
        })
        .await
        .map_err(|_| SubmitError::Storage)?
    }
    async fn submit_owned(
        &self,
        lease: &SessionLease,
        input: SubmitRequest,
    ) -> Result<JobView, SubmitError> {
        self.validate_preview(&input, lease).await?;
        let profiles = ProfileService::new(self.store.clone());
        let active = profiles.profile_for_server(input.server_id, lease).await?;
        if active.version != input.profile_version {
            return Err(SubmitError::Conflict);
        }
        let profile = profiles
            .for_job(
                input.server_id,
                active.id,
                input.profile_version.get(),
                lease,
            )
            .await?;
        let command = validate_job(JobCommand {
            protocol_version: PROTOCOL_VERSION,
            job_id: input.job_id,
            agent_id: profile.credentials.agent_id,
            profile_id: profile.profile_id,
            profile_version: input.profile_version,
            operation: input.operation,
            scan: input.scan,
            resolved_cutoff: input.resolved_cutoff,
            service_policy: match input.operation {
                Operation::Preview => ServicePolicy::RequireStopped,
                Operation::Clean => input.service_policy,
            },
            limits: input.limits,
            mail_profile: profile.mail_profile,
        })
        .map_err(|_| SubmitError::Invalid)?;
        if !self.reserve(&input, active.id, lease).await? {
            return self.load(input.job_id).await;
        }
        self.publish(input.job_id).await?;
        if let Ok(client) = AgentClient::new(&self.store, profile.credentials, lease).await {
            let result = client.submit_job(command.command()).await;
            drop(command);
            if let Ok(snapshot) = result {
                self.accept_snapshot(input.job_id, snapshot, &client, lease)
                    .await?;
            }
        }
        self.load(input.job_id).await
    }
}
