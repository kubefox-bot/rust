use super::types::TechnicalProfile;
use hmail_cleaner_protocol::browser::profiles::EditableProfile;
const DEFAULT_MYSQL_PORT: u16 = 3306;
pub(super) fn map(profile: TechnicalProfile) -> EditableProfile {
    let configured = profile.mail_profile.is_some();
    let mail = profile.mail_profile;
    EditableProfile {
        id: profile.profile_id,
        version: profile.version.into(),
        agent_id: profile.credentials.agent_id,
        configured,
        mysql_host: mail
            .as_ref()
            .map(|m| m.mysql.host.clone())
            .unwrap_or_default(),
        mysql_port: mail
            .as_ref()
            .map(|m| m.mysql.port)
            .unwrap_or(DEFAULT_MYSQL_PORT),
        mysql_database: mail
            .as_ref()
            .map(|m| m.mysql.database.clone())
            .unwrap_or_default(),
        mysql_username: mail
            .as_ref()
            .map(|m| m.mysql.username.clone())
            .unwrap_or_default(),
        password_set: mail.as_ref().is_some_and(|m| !m.mysql.password.is_empty()),
        mysql_tls: mail.as_ref().map(|m| m.mysql.tls).unwrap_or_default(),
        data_dir: mail
            .as_ref()
            .map(|m| m.data_dir.clone())
            .unwrap_or_default(),
        hmail_ini: mail
            .as_ref()
            .map(|m| m.hmail_ini.clone())
            .unwrap_or_default(),
        service_name: mail
            .as_ref()
            .map(|m| m.service_name.clone())
            .unwrap_or_default(),
    }
}
