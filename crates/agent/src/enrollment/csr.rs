use super::{keys, AgentBootstrap, AgentCsr, EnrollmentError};
use rcgen::CertificateParams;

/// Produces a public CSR only. The issuer assigns SAN/EKU and checks its signature.
pub fn prepare(bootstrap: &AgentBootstrap) -> Result<AgentCsr, EnrollmentError> {
    let key = keys::load_or_create(bootstrap)?;
    let params = CertificateParams::new(Vec::<String>::new())?;
    let csr = params.serialize_request(&key)?;
    Ok(AgentCsr {
        agent_id: bootstrap.agent_id,
        csr_pem: csr.pem()?,
    })
}
