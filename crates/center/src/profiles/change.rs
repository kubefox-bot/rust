use super::ProfileError;
use hmail_cleaner_protocol::{
    browser::profiles::{PasswordPatch, ProfilePatch},
    AgentId, DatabaseParts, MailProfile, ProfileValidationRequest, PROTOCOL_VERSION,
};
pub(super) fn apply(
    agent: AgentId,
    previous: Option<MailProfile>,
    patch: ProfilePatch,
) -> Result<MailProfile, ProfileError> {
    let password = match patch.password {
        PasswordPatch::Keep => previous.ok_or(ProfileError::Invalid)?.mysql.password,
        PasswordPatch::Replace(value) => value,
    };
    let mail_profile = MailProfile {
        mysql: DatabaseParts {
            host: patch.mysql_host,
            port: patch.mysql_port,
            database: patch.mysql_database,
            username: patch.mysql_username,
            password,
            tls: patch.mysql_tls,
        },
        data_dir: patch.data_dir,
        hmail_ini: patch.hmail_ini,
        service_name: patch.service_name,
    };
    ProfileValidationRequest {
        protocol_version: PROTOCOL_VERSION,
        agent_id: agent,
        mail_profile,
    }
    .validate()
    .map(|profile| profile.into_profile())
    .map_err(|_| ProfileError::Invalid)
}
