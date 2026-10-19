use std::sync::{Arc, Mutex};
pub(super) struct HostKey {
    pub expected: Option<String>,
    pub observed: Arc<Mutex<Option<String>>>,
}
impl russh::client::Handler for HostKey {
    type Error = russh::Error;
    async fn check_server_key(
        &mut self,
        key: &russh::keys::PublicKeyOrCertificate,
    ) -> Result<bool, Self::Error> {
        // Raw host keys only. A server certificate cannot silently substitute its embedded key.
        if key.certificate().is_some() {
            return Ok(false);
        }
        let fingerprint = key
            .public_key()
            .fingerprint(russh::keys::HashAlg::Sha256)
            .to_string();
        let accepted = self
            .expected
            .as_ref()
            .is_none_or(|expected| expected == &fingerprint);
        if let Ok(mut observed) = self.observed.lock() {
            *observed = Some(fingerprint);
        } else {
            return Ok(false);
        }
        Ok(accepted)
    }
}
