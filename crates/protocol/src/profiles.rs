use crate::limits::{MAX_PASSWORD_BYTES, MAX_PROFILE_TEXT_BYTES};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DatabaseParts {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    pub password: Zeroizing<String>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MailProfile {
    pub mysql: DatabaseParts,
    pub data_dir: String,
    pub hmail_ini: String,
    pub service_name: String,
}
impl MailProfile {
    pub(crate) fn is_valid(&self) -> bool {
        let fields = [
            &self.mysql.host,
            &self.mysql.database,
            &self.mysql.username,
            &self.data_dir,
            &self.hmail_ini,
            &self.service_name,
        ];
        fields.iter().all(|value| {
            !value.trim().is_empty()
                && value.len() <= MAX_PROFILE_TEXT_BYTES
                && !value.contains('\0')
        }) && self.mysql.port != 0
            && self.mysql.password.len() <= MAX_PASSWORD_BYTES
            && !self.mysql.password.contains('\0')
    }
}
