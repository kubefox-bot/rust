use super::Host;
use crate::{
    backup,
    config::Config,
    control::{self, State},
    journal::{self, Status, Transaction},
    podman, Error,
};
use hmail_cleaner_protocol::browser::updates::UpdatePolicy;
use std::{path::Path, time::Duration};
pub struct NativeHost<'a> {
    pub config: &'a Config,
    pub policy: UpdatePolicy,
}
impl NativeHost<'_> {
    pub async fn wait_health(&self, build: &str) -> Result<(), Error> {
        tokio::time::timeout(Duration::from_secs(120), async {
            loop {
                if podman::health(build).await.is_ok() {
                    return Ok(());
                }
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        })
        .await
        .map_err(|_| Error::Attention)?
    }
    pub fn tx_path(&self) -> std::path::PathBuf {
        self.config.state_dir.join("transaction.json")
    }
    fn select_previous(&self, tx: &Transaction) -> Result<(), Error> {
        let old = std::fs::read_to_string(&tx.previous_unit).map_err(|_| Error::Storage)?;
        let previous = podman::unit_identity(&old)?;
        if previous.image != tx.previous_image || previous.build != tx.previous_build {
            return Err(Error::Verification);
        }
        podman::write_unit(&self.config.unit_path, &old)
    }
    async fn choose_previous(&self, tx: &Transaction) -> Result<(), Error> {
        self.select_previous(tx)?;
        podman::systemctl("daemon-reload").await
    }
    pub async fn restart_previous(&self, tx: &Transaction) -> Result<(), Error> {
        self.choose_previous(tx).await?;
        podman::systemctl("start").await?;
        self.wait_health(&tx.previous_build).await?;
        Ok(())
    }
}
impl Host for NativeHost<'_> {
    fn record(&self, tx: &Transaction) -> Result<(), Error> {
        journal::save(&self.tx_path(), tx)?;
        crate::retention::record(self.config, tx)
    }
    fn graceful(&self) -> Duration {
        Duration::from_secs(u64::from(self.policy.graceful_timeout_minutes) * 60)
    }
    async fn begin(&self, tx: &Transaction) -> Result<State, Error> {
        Ok(
            control::request(&self.config.socket_path, "begin", Some(tx.id))
                .await?
                .status,
        )
    }
    async fn status(&self, tx: &Transaction) -> Result<State, Error> {
        Ok(
            control::request(&self.config.socket_path, "status", Some(tx.id))
                .await?
                .status,
        )
    }
    async fn abort(&self, tx: &Transaction) -> Result<(), Error> {
        control::request(&self.config.socket_path, "abort", Some(tx.id)).await?;
        Ok(())
    }
    async fn stop(&self) -> Result<(), Error> {
        podman::systemctl("stop").await?;
        let bytes = podman::podman(vec![
            "ps".into(),
            "--filter".into(),
            "name=^hmail-center$".into(),
            "--format".into(),
            "{{.Names}}".into(),
        ])
        .await?;
        if !bytes.iter().all(|byte| byte.is_ascii_whitespace()) {
            return Err(Error::Conflict);
        }
        Ok(())
    }
    async fn backup(&self, tx: &Transaction) -> Result<(), Error> {
        backup::create(tx).await
    }
    async fn start_candidate(&self, tx: &Transaction) -> Result<(), Error> {
        let old = std::fs::read_to_string(&tx.previous_unit).map_err(|_| Error::Storage)?;
        let candidate = podman::candidate_unit(&old, &tx.candidate_image, &tx.candidate_build)?;
        podman::write_unit(&self.config.unit_path, &candidate)?;
        podman::systemctl("daemon-reload").await?;
        podman::systemctl("start").await
    }
    async fn verify(&self, tx: &Transaction) -> Result<(), Error> {
        self.wait_health(&tx.candidate_build).await?;
        backup::assert_identity(tx).await?;
        let images: Vec<serde_json::Value> = serde_json::from_slice(
            &podman::podman(vec!["inspect".into(), "hmail-center".into()]).await?,
        )
        .map_err(|_| Error::Invalid)?;
        let expected_images: Vec<serde_json::Value> = serde_json::from_slice(
            &podman::podman(vec![
                "image".into(),
                "inspect".into(),
                tx.candidate_image.clone().into(),
            ])
            .await?,
        )
        .map_err(|_| Error::Invalid)?;
        let local = control::request(&self.config.socket_path, "status", Some(tx.id)).await?;
        let backup_manifest: serde_json::Value =
            journal::load(&tx.backup.join("manifest.json"))?.ok_or(Error::Verification)?;
        let expected_vault = backup_manifest["vault_id"]
            .as_str()
            .and_then(|value| uuid::Uuid::parse_str(value).ok());
        if images.len() != 1
            || expected_images.len() != 1
            || images[0]["Image"] != expected_images[0]["Id"]
            || expected_vault.is_none()
            || local.vault_id != expected_vault
        {
            return Err(Error::Verification);
        }
        Ok(())
    }
    async fn expose(&self, tx: &Transaction) -> Result<(), Error> {
        control::request(&self.config.socket_path, "finish", Some(tx.id)).await?;
        Ok(())
    }
    async fn restart_previous(&self, tx: &Transaction) -> Result<(), Error> {
        NativeHost::restart_previous(self, tx).await
    }
    async fn restore(&self, tx: &Transaction) -> Result<(), Error> {
        backup::export_candidate(tx).await?;
        backup::restore::run(tx).await?;
        self.restart_previous(tx).await
    }
    fn quarantine(&self, tx: &Transaction) -> Result<(), Error> {
        let path = self.config.state_dir.join("status.json");
        let mut status = journal::load::<Status>(&path)?.unwrap_or_default();
        status.quarantined_digest = tx
            .candidate_image
            .rsplit_once('@')
            .map(|(_, hash)| hash.to_owned());
        status.error = Some("candidate_failed_restored".into());
        journal::save(&path, &status)
    }
    fn commit(&self, tx: &Transaction) -> Result<(), Error> {
        let path = self.config.state_dir.join("status.json");
        let mut status = journal::load::<Status>(&path)?.unwrap_or_default();
        status.installed_sequence = tx.sequence;
        status.installed_build = Some(tx.candidate_build.clone());
        status.installed_digest = tx
            .candidate_image
            .rsplit_once('@')
            .map(|(_, digest)| digest.to_owned());
        status.error = None;
        journal::save(&path, &status)
    }
}
pub fn read_unit(path: &Path) -> Result<String, Error> {
    let meta = std::fs::symlink_metadata(path).map_err(|_| Error::Storage)?;
    if !meta.is_file() || meta.len() > 65536 {
        return Err(Error::Invalid);
    }
    std::fs::read_to_string(path).map_err(|_| Error::Storage)
}

impl super::boot::Host for NativeHost<'_> {
    fn select_previous(&self, tx: &Transaction) -> Result<(), Error> {
        NativeHost::select_previous(self, tx)
    }
    async fn reload(&self) -> Result<(), Error> {
        podman::systemctl("daemon-reload").await
    }
    async fn mark(&self, tx: &Transaction) -> Result<(), Error> {
        podman::mark(tx.id).await
    }
    async fn restore_offline(&self, tx: &mut Transaction) -> Result<(), Error> {
        backup::verify_hashes(tx)?;
        backup::export_candidate(tx).await?;
        tx.phase = crate::journal::Phase::Restoring;
        self.record(tx)?;
        backup::restore::run(tx).await
    }
}
