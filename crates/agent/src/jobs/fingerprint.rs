use super::{JobRepository, RequestFingerprint};
use hmac::{Hmac, Mac};
use hmail_cleaner_protocol::{canonical_job, ValidatedJob};
use sha2::Sha256;
use zeroize::Zeroizing;
impl JobRepository {
    pub fn fingerprint(&self, job: &ValidatedJob) -> RequestFingerprint {
        let bytes = Zeroizing::new(canonical_job(job));
        let mut mac =
            Hmac::<Sha256>::new_from_slice(&*self.key).expect("HMAC accepts a 32-byte key");
        mac.update(&bytes);
        RequestFingerprint(mac.finalize().into_bytes().into())
    }
}
