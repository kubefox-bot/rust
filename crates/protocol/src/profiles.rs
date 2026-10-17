use crate::limits::{MAX_PASSWORD_BYTES, MAX_PROFILE_TEXT_BYTES};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DatabaseTls {
    #[default]
    Preferred,
    Required,
    DisabledLocal,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DatabaseParts {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    pub password: Zeroizing<String>,
    #[serde(default)]
    pub tls: DatabaseTls,
}
impl DatabaseParts {
    pub fn permits_transport(&self) -> bool {
        match self.tls {
            DatabaseTls::Preferred | DatabaseTls::Required => true,
            DatabaseTls::DisabledLocal => {
                self.host.eq_ignore_ascii_case("localhost")
                    || self
                        .host
                        .parse::<std::net::IpAddr>()
                        .is_ok_and(|ip| ip.is_loopback())
            }
        }
    }
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
            && !hmail_cleaner_core::maintenance::mail_service::is_reserved_service_name(
                &self.service_name,
            )
            && self.mysql.permits_transport()
            && self.mysql.password.len() <= MAX_PASSWORD_BYTES
            && !self.mysql.password.contains('\0')
    }
}
