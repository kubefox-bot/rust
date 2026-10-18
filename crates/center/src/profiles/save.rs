use super::{append::NewVersion, types::TechnicalProfile, ProfileError, ProfileService};
use crate::auth::SessionLease;
use hmail_cleaner_protocol::{
    browser::profiles::{ProfilePatch, ProfileVersion},
    ProfileId,
};
impl ProfileService {
    pub async fn save(
        &self,
        id: ProfileId,
        expected_version: u64,
        patch: ProfilePatch,
        lease: &SessionLease,
    ) -> Result<ProfileVersion, ProfileError> {
        let maintenance = self
            .store
            .maintenance()
            .admit()
            .map_err(|_| ProfileError::Conflict)?;
        let owner = self.clone();
        let lease = lease.clone();
        tokio::spawn(async move {
            let _maintenance = maintenance;
            owner.save_owned(id, expected_version, patch, &lease).await
        })
        .await
        .map_err(|_| ProfileError::Storage)?
    }
    async fn save_owned(
        &self,
        id: ProfileId,
        expected_version: u64,
        patch: ProfilePatch,
        lease: &SessionLease,
    ) -> Result<ProfileVersion, ProfileError> {
        let stored = self.load(id, None, lease).await?;
        if stored.technical.version != expected_version {
            return Err(ProfileError::Conflict);
        }
        let TechnicalProfile {
            server_id,
            profile_id,
            credentials,
            mail_profile,
            ..
        } = stored.technical;
        let mail = super::change::apply(credentials.agent_id, mail_profile, patch)?;
        let version = expected_version
            .checked_add(1)
            .ok_or(ProfileError::Conflict)?;
        let profile = TechnicalProfile {
            server_id,
            profile_id,
            credentials,
            mail_profile: Some(mail),
            version,
        };
        let (record, bytes) = Self::encrypt(&profile, &stored.material, lease)?;
        drop(profile);
        self.append(
            NewVersion {
                id,
                server: server_id,
                version,
                record,
                bytes,
            },
            lease,
        )
        .await?;
        Ok(ProfileVersion {
            id,
            version: version.into(),
        })
    }
}
