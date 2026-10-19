use super::{
    material::{CaSecret, ClientSecret},
    policy, IdentityError,
};
use crate::{
    config::BrowserOrigin,
    vault::{
        self, AccountMaterial, Dek, EncryptedRecord, EnvelopeHeader, RecordIdentity, RecordKind,
    },
};
use rcgen::{
    BasicConstraints, CertificateParams, DnType, ExtendedKeyUsagePurpose, IsCa, Issuer, KeyPair,
    KeyUsagePurpose,
};
use std::{io::Write, path::Path};
use time::{Duration, OffsetDateTime};
use zeroize::Zeroizing;
const CA_DAYS: i64 = 3650;
const SERVER_DAYS: i64 = 365;
const CLOCK_SKEW_MINUTES: i64 = 5;
pub(crate) struct PreparedIdentity {
    pub ca: EncryptedRecord,
    pub client: EncryptedRecord,
    cert: String,
    key: Zeroizing<String>,
    root: String,
}
pub(crate) fn prepare(
    material: &AccountMaterial,
    dek: &Dek,
    origin: &BrowserOrigin,
) -> Result<PreparedIdentity, IdentityError> {
    let now = OffsetDateTime::now_utc();
    let mut params = CertificateParams::new(Vec::<String>::new()).map_err(|_| IdentityError)?;
    params.is_ca = IsCa::Ca(BasicConstraints::Constrained(0));
    params.distinguished_name.push(
        DnType::CommonName,
        format!("hMail Cleaner {}", material.vault_id()),
    );
    params.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];
    params.not_before = now - Duration::minutes(CLOCK_SKEW_MINUTES);
    params.not_after = now + Duration::days(CA_DAYS);
    params.serial_number = Some(policy::serial()?);
    let ca_key = KeyPair::generate().map_err(|_| IdentityError)?;
    let ca_cert = params.self_signed(&ca_key).map_err(|_| IdentityError)?;
    let secret = CaSecret {
        certificate_pem: ca_cert.pem(),
        private_key_pem: Zeroizing::new(ca_key.serialize_pem()),
    };
    let private = Zeroizing::new(serde_json::to_vec(&secret).map_err(|_| IdentityError)?);
    let header = EnvelopeHeader::record(RecordIdentity {
        vault_id: material.vault_id(),
        kind: RecordKind::CaKey,
        record_id: material.vault_id(),
        record_version: 1,
        key_id: material.dek_id(),
        key_generation: 1,
    })
    .map_err(|_| IdentityError)?;
    let ca = vault::seal_record(dek, &header, &private).map_err(|_| IdentityError)?;
    let issuer = Issuer::new(params, ca_key);
    let mut server =
        CertificateParams::new(vec![origin.host().into()]).map_err(|_| IdentityError)?;
    server.key_usages = vec![KeyUsagePurpose::DigitalSignature];
    server.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
    server.not_before = now - Duration::minutes(CLOCK_SKEW_MINUTES);
    server.not_after = now + Duration::days(SERVER_DAYS);
    server.serial_number = Some(policy::serial()?);
    let server_key = KeyPair::generate().map_err(|_| IdentityError)?;
    let cert = server
        .signed_by(&server_key, &issuer)
        .map_err(|_| IdentityError)?;
    let client_key = KeyPair::generate().map_err(|_| IdentityError)?;
    let client_params = policy::client_params(material.vault_id())?;
    let client_cert = client_params
        .signed_by(&client_key, &issuer)
        .map_err(|_| IdentityError)?;
    let client_secret = ClientSecret {
        certificate_pem: format!("{}{}", client_cert.pem(), ca_cert.pem()),
        private_key_pem: Zeroizing::new(client_key.serialize_pem()),
        ca_pem: ca_cert.pem(),
    };
    let client_plaintext =
        Zeroizing::new(serde_json::to_vec(&client_secret).map_err(|_| IdentityError)?);
    let client_header = policy::header(
        material,
        RecordKind::CenterClientKey,
        material.account_id(),
        1,
    )?;
    let client =
        vault::seal_record(dek, &client_header, &client_plaintext).map_err(|_| IdentityError)?;
    Ok(PreparedIdentity {
        ca,
        client,
        cert: format!("{}{}", cert.pem(), ca_cert.pem()),
        key: Zeroizing::new(server_key.serialize_pem()),
        root: ca_cert.pem(),
    })
}
impl PreparedIdentity {
    pub fn persist(&self, directory: &Path) -> Result<(), IdentityError> {
        for (name, bytes) in [
            ("ca-cert.pem", self.root.as_bytes()),
            ("https-cert.pem", self.cert.as_bytes()),
            ("https-key.pem", self.key.as_bytes()),
        ] {
            let mut options = std::fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options
                .open(directory.join(name))
                .map_err(|_| IdentityError)?;
            file.write_all(bytes)
                .and_then(|_| file.sync_all())
                .map_err(|_| IdentityError)?;
        }
        #[cfg(unix)]
        std::fs::File::open(directory)
            .and_then(|file| file.sync_all())
            .map_err(|_| IdentityError)?;
        Ok(())
    }
}
