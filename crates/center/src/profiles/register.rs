use super::{types::TechnicalProfile, ProfileError, ProfileService, ServerRegistration};
use crate::{
    agents::{AgentCredentials, AgentEndpoint},
    auth::SessionLease,
};
use hmail_cleaner_protocol::browser::profiles::ProfileVersion;
impl ProfileService {
    pub async fn register(
        &self,
        input: ServerRegistration,
        lease: &SessionLease,
    ) -> Result<ProfileVersion, ProfileError> {
        let permit = self
            .store
            .maintenance()
            .admit()
            .map_err(|_| ProfileError::Conflict)?;
        self.register_with_permit(input, lease, permit).await
    }
    pub(crate) async fn register_continuing(
        &self,
        input: ServerRegistration,
        lease: &SessionLease,
    ) -> Result<ProfileVersion, ProfileError> {
        let permit = self
            .store
            .maintenance()
            .continuation()
            .map_err(|_| ProfileError::Conflict)?;
        self.register_with_permit(input, lease, permit).await
    }
    async fn register_with_permit(
        &self,
        input: ServerRegistration,
        lease: &SessionLease,
        permit: crate::maintenance::WritePermit,
    ) -> Result<ProfileVersion, ProfileError> {
        let owner = self.clone();
        let lease = lease.clone();
        tokio::spawn(async move {
            let _maintenance = permit;
            owner.register_owned(input, &lease).await
        })
        .await
        .map_err(|_| ProfileError::Storage)?
    }
    async fn register_owned(
        &self,
        input: ServerRegistration,
        lease: &SessionLease,
    ) -> Result<ProfileVersion, ProfileError> {
        if input.name.trim().is_empty()
            || input.name.len() > 128
            || input.name.chars().any(char::is_control)
        {
            return Err(ProfileError::Invalid);
        }
        let endpoint = AgentEndpoint::try_from(input.endpoint)?;
        let account = self
            .store
            .account()
            .await
            .map_err(|_| ProfileError::Storage)?
            .ok_or(ProfileError::Storage)?;
        let profile = TechnicalProfile {
            server_id: input.server_id,
            profile_id: input.profile_id,
            version: 1,
            credentials: AgentCredentials {
                agent_id: input.agent_id,
                center_id: account.material.vault_id(),
                endpoint,
            },
            mail_profile: None,
        };
        let (record, bytes) = Self::encrypt(&profile, &account.material, lease)?;
        let _writer = self.store.catalog_writer.lock().await;
        if !lease.is_live() {
            return Err(ProfileError::Unauthorized);
        }
        let mut transaction = self
            .store
            .pool
            .begin()
            .await
            .map_err(|_| ProfileError::Storage)?;
        let trusted: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM trust WHERE agent_id=? AND revoked=0)")
                .bind(input.agent_id.to_string())
                .fetch_one(&mut *transaction)
                .await
                .map_err(|_| ProfileError::Storage)?;
        if !trusted {
            return Err(ProfileError::Invalid);
        }
        sqlx::query("INSERT INTO servers(id,name,agent_id,active_profile_id) VALUES(?,?,?,?)")
            .bind(input.server_id.to_string())
            .bind(input.name)
            .bind(input.agent_id.to_string())
            .bind(input.profile_id.to_string())
            .execute(&mut *transaction)
            .await
            .map_err(super::repository::write_error)?;
        sqlx::query("INSERT INTO encrypted_records VALUES(?,'profile',1,?)")
            .bind(record.to_string())
            .bind(bytes)
            .execute(&mut *transaction)
            .await
            .map_err(|_| ProfileError::Storage)?;
        sqlx::query("INSERT INTO profile_versions VALUES(?,1,?,?)")
            .bind(input.profile_id.to_string())
            .bind(input.server_id.to_string())
            .bind(record.to_string())
            .execute(&mut *transaction)
            .await
            .map_err(|_| ProfileError::Storage)?;
        transaction
            .commit()
            .await
            .map_err(|_| ProfileError::Storage)?;
        Ok(ProfileVersion {
            id: input.profile_id,
            version: 1.into(),
        })
    }
}
