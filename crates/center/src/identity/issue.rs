use super::{issuer, policy, IdentityError};
use crate::{auth::SessionLease, storage::CenterStore};
use hmail_cleaner_protocol::AgentId;
use rcgen::{
    CertificateParams, CertificateSigningRequestParams, ExtendedKeyUsagePurpose, KeyUsagePurpose,
    SanType,
};
use time::{Duration, OffsetDateTime};
#[derive(Clone)]
pub struct IdentityService {
    pub(super) store: CenterStore,
}
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IssuedAgent {
    pub certificate_pem: String,
    pub ca_pem: String,
    pub serial: Vec<u8>,
    pub expires_at_unix: i64,
}
impl IdentityService {
    pub fn new(store: CenterStore) -> Self {
        Self { store }
    }
    pub(super) async fn sign_agent(
        &self,
        agent: AgentId,
        host: &str,
        csr: &str,
        lease: &SessionLease,
    ) -> Result<IssuedAgent, IdentityError> {
        const MAX_CSR_BYTES: usize = 16384;
        if csr.len() > MAX_CSR_BYTES
            || rustls::pki_types::ServerName::try_from(host.to_owned()).is_err()
            || !lease.is_live()
        {
            return Err(IdentityError);
        }
        let mut request =
            CertificateSigningRequestParams::from_pem(csr).map_err(|_| IdentityError)?;
        let account = self
            .store
            .account()
            .await
            .map_err(|_| IdentityError)?
            .ok_or(IdentityError)?;
        let signing = issuer::load(&self.store, &account.material, lease).await?;
        let now = OffsetDateTime::now_utc();
        let mut params = CertificateParams::new(vec![host.into()]).map_err(|_| IdentityError)?;
        params.subject_alt_names.push(SanType::URI(
            format!("urn:hmail-cleaner:agent:{agent}")
                .try_into()
                .map_err(|_| IdentityError)?,
        ));
        params.serial_number = Some(policy::serial()?);
        params.key_usages = vec![KeyUsagePurpose::DigitalSignature];
        params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
        params.not_before = now - Duration::minutes(5);
        params.not_after = now + Duration::days(policy::IDENTITY_DAYS);
        request.params = params;
        let certificate = request
            .signed_by(&signing.issuer)
            .map_err(|_| IdentityError)?;
        let serial = hmail_cleaner_adapters::mtls::certificate_serial(certificate.der())
            .map_err(|_| IdentityError)?;
        let issued = IssuedAgent {
            certificate_pem: format!("{}{}", certificate.pem(), signing.ca_pem),
            ca_pem: signing.ca_pem,
            serial,
            expires_at_unix: request.params.not_after.unix_timestamp(),
        };
        drop(signing.issuer);
        Ok(issued)
    }
    pub async fn issue_agent(
        &self,
        agent: AgentId,
        host: &str,
        csr: &str,
        lease: &SessionLease,
    ) -> Result<IssuedAgent, IdentityError> {
        let issued = self.sign_agent(agent, host, csr, lease).await?;
        super::repository::activate(
            self.store.clone(),
            lease.clone(),
            agent,
            issued,
            super::repository::Activation::Enrollment,
        )
        .await
    }
    pub async fn revoke_agent(
        &self,
        agent: AgentId,
        lease: &SessionLease,
    ) -> Result<(), IdentityError> {
        super::repository::revoke(self.store.clone(), lease.clone(), agent).await
    }
}
