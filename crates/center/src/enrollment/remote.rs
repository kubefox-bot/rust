use super::{AgentPackage, EnrollmentError, SshCredentials, SshTarget};
use crate::auth::SessionLease;
use hmail_cleaner_protocol::{browser::enrollment::LegacyService, AgentCsr, AgentId};
use serde::Serialize;
use std::future::Future;
use uuid::Uuid;
#[derive(Clone, Serialize)]
pub struct Bootstrap {
    pub operation_id: Uuid,
    pub agent_id: AgentId,
    pub center_id: Uuid,
    pub sha256: String,
}
#[derive(Serialize)]
pub struct BootstrapIdentity<'a> {
    pub bootstrap: &'a Bootstrap,
    pub certificate_pem: &'a str,
    pub ca_pem: &'a str,
    pub listen_port: u16,
}
pub trait Connector: Send + Sync + 'static {
    type Connection: Remote;
    fn connect(
        &self,
        target: SshTarget,
        fingerprint: String,
        credentials: SshCredentials,
        lease: &SessionLease,
    ) -> impl Future<Output = Result<Self::Connection, EnrollmentError>> + Send;
}
pub trait Remote: Send + Sync {
    fn prepare_recovery(
        &self,
        input: &Bootstrap,
    ) -> impl Future<Output = Result<(), EnrollmentError>> + Send;
    fn recover_identity(
        &self,
        input: &BootstrapIdentity<'_>,
    ) -> impl Future<Output = Result<(), EnrollmentError>> + Send;
    fn preflight(&self) -> impl Future<Output = Result<(), EnrollmentError>> + Send;
    fn prepare(
        &self,
        input: &Bootstrap,
    ) -> impl Future<Output = Result<String, EnrollmentError>> + Send;
    fn upload(
        &self,
        path: &str,
        package: &mut AgentPackage,
    ) -> impl Future<Output = Result<(), EnrollmentError>> + Send;
    fn enroll(
        &self,
        input: &Bootstrap,
    ) -> impl Future<Output = Result<AgentCsr, EnrollmentError>> + Send;
    fn configure(
        &self,
        input: &BootstrapIdentity<'_>,
    ) -> impl Future<Output = Result<(), EnrollmentError>> + Send;
    fn install(
        &self,
        input: &Bootstrap,
        legacy: Option<&LegacyService>,
    ) -> impl Future<Output = Result<(), EnrollmentError>> + Send;
    fn cleanup(
        &self,
        input: &Bootstrap,
    ) -> impl Future<Output = Result<(), EnrollmentError>> + Send;
    fn close(self) -> impl Future<Output = Result<(), EnrollmentError>> + Send;
}
