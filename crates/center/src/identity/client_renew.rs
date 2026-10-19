use super::{client_material, issuer, policy, IdentityError};
use crate::{
    auth::SessionLease,
    storage::CenterStore,
    vault::{self, RecordKind},
};
use rustls::pki_types::{pem::PemObject, CertificateDer};
use zeroize::Zeroizing;
pub(crate) async fn ensure(
    store: &CenterStore,
    lease: &SessionLease,
) -> Result<bool, IdentityError> {
    let mut stored = client_material::load(store, lease).await?;
    let leaf = CertificateDer::from_pem_slice(stored.secret.certificate_pem.as_bytes())
        .map_err(|_| IdentityError)?;
    let (_, certificate) = x509_parser::parse_x509_certificate(&leaf).map_err(|_| IdentityError)?;
    let threshold = chrono::Utc::now() + chrono::Duration::days(policy::EXPIRY_WARNING_DAYS);
    if certificate.validity().not_after.timestamp() > threshold.timestamp() {
        return Ok(false);
    }
    let signing = issuer::load(store, &stored.material, lease).await?;
    let key =
        rcgen::KeyPair::from_pem(&stored.secret.private_key_pem).map_err(|_| IdentityError)?;
    let params = policy::client_params(stored.material.vault_id())?;
    let certificate = params
        .signed_by(&key, &signing.issuer)
        .map_err(|_| IdentityError)?;
    stored.secret.certificate_pem = format!("{}{}", certificate.pem(), signing.ca_pem);
    stored.secret.ca_pem = signing.ca_pem;
    let version = stored.version.checked_add(1).ok_or(IdentityError)?;
    let header = policy::header(
        &stored.material,
        RecordKind::CenterClientKey,
        stored.material.account_id(),
        version,
    )?;
    let plaintext = Zeroizing::new(serde_json::to_vec(&stored.secret).map_err(|_| IdentityError)?);
    let record = lease
        .with_dek(|dek| vault::seal_record(dek, &header, &plaintext))
        .map_err(|_| IdentityError)?
        .map_err(|_| IdentityError)?;
    let bytes = record.encode().map_err(|_| IdentityError)?;
    let id = stored.material.account_id();
    let previous = i64::try_from(stored.version).map_err(|_| IdentityError)?;
    let next = i64::try_from(version).map_err(|_| IdentityError)?;
    drop(plaintext);
    drop(stored.secret);
    drop(key);
    drop(signing.issuer);
    let store = store.clone();
    let lease = lease.clone();
    let writer = store
        .catalog_writer
        .clone()
        .try_lock_owned()
        .map_err(|_| IdentityError)?;
    let maintenance = store
        .maintenance()
        .continuation()
        .map_err(|_| IdentityError)?;
    tokio::spawn(async move {
        let _maintenance = maintenance;
        let _writer=writer;if !lease.is_live(){return Err(IdentityError);}
        let result=sqlx::query("UPDATE encrypted_records SET envelope=?,version=? WHERE id=? AND version=? AND kind='center_client_key'")
            .bind(bytes).bind(next).bind(id.to_string()).bind(previous).execute(&store.pool).await.map_err(|_|IdentityError)?;
        if result.rows_affected()!=1{return Err(IdentityError);}
        store.revoked.invalidate_connections();Ok(true)
    }).await.map_err(|_|IdentityError)?
}
