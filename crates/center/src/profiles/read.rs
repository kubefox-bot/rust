use super::{ProfileError, ProfileService, SelectedProfile};
use crate::auth::SessionLease;
use hmail_cleaner_protocol::{
    browser::profiles::{EditableProfile, ServerView},
    ProfileId, ServerId,
};
use sqlx::Row;
impl ProfileService {
    pub async fn credentials_for_server(
        &self,
        server: ServerId,
        lease: &SessionLease,
    ) -> Result<crate::agents::AgentCredentials, ProfileError> {
        let id = self.active_profile(server).await?;
        let profile = self.load(id, None, lease).await?.technical;
        if profile.server_id != server {
            return Err(ProfileError::Storage);
        }
        Ok(profile.credentials)
    }
    pub async fn read(
        &self,
        id: ProfileId,
        lease: &SessionLease,
    ) -> Result<EditableProfile, ProfileError> {
        let profile = self.load(id, None, lease).await?.technical;
        Ok(super::editable::map(profile))
    }
    pub async fn profile_for_server(
        &self,
        server: ServerId,
        lease: &SessionLease,
    ) -> Result<EditableProfile, ProfileError> {
        let id = self.active_profile(server).await?;
        let stored = self.load(id, None, lease).await?;
        if stored.technical.server_id != server {
            return Err(ProfileError::Storage);
        }
        Ok(super::editable::map(stored.technical))
    }
    pub async fn active_profile(&self, server: ServerId) -> Result<ProfileId, ProfileError> {
        let id: String = sqlx::query_scalar("SELECT active_profile_id FROM servers WHERE id=?")
            .bind(server.to_string())
            .fetch_optional(&self.store.pool)
            .await
            .map_err(|_| ProfileError::Storage)?
            .ok_or(ProfileError::NotFound)?;
        serde_json::from_value(serde_json::Value::String(id)).map_err(|_| ProfileError::Storage)
    }
    pub async fn for_job(
        &self,
        server: ServerId,
        id: ProfileId,
        version: u64,
        lease: &SessionLease,
    ) -> Result<SelectedProfile, ProfileError> {
        let profile = self.load(id, Some(version), lease).await?.technical;
        if profile.server_id != server {
            return Err(ProfileError::Invalid);
        }
        Ok(SelectedProfile {
            server_id: server,
            profile_id: id,
            version,
            credentials: profile.credentials,
            mail_profile: profile.mail_profile.ok_or(ProfileError::NotFound)?,
        })
    }
    pub async fn servers(&self, lease: &SessionLease) -> Result<Vec<ServerView>, ProfileError> {
        if !lease.is_live() {
            return Err(ProfileError::Unauthorized);
        }
        let rows = sqlx::query(
            "SELECT id,name,agent_id,active_profile_id FROM servers ORDER BY name,id LIMIT 1000",
        )
        .fetch_all(&self.store.pool)
        .await
        .map_err(|_| ProfileError::Storage)?;
        rows.into_iter()
            .map(|row| {
                let id: String = row.try_get("id").map_err(|_| ProfileError::Storage)?;
                let agent: String = row.try_get("agent_id").map_err(|_| ProfileError::Storage)?;
                let profile: String = row
                    .try_get("active_profile_id")
                    .map_err(|_| ProfileError::Storage)?;
                Ok(ServerView {
                    id: serde_json::from_value(serde_json::Value::String(id))
                        .map_err(|_| ProfileError::Storage)?,
                    name: row.try_get("name").map_err(|_| ProfileError::Storage)?,
                    agent_id: serde_json::from_value(serde_json::Value::String(agent))
                        .map_err(|_| ProfileError::Storage)?,
                    profile_id: serde_json::from_value(serde_json::Value::String(profile))
                        .map_err(|_| ProfileError::Storage)?,
                })
            })
            .collect()
    }
}
