use crate::{
    apply::{self, native::NativeHost, Host},
    config::{private_directory, Config},
    control, discovery,
    journal::{self, OperationLock, Phase, Status, Transaction},
    network::Network,
    podman, policy, Error,
};
use hmail_cleaner_protocol::browser::updates::UpdatePolicy;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;
fn now() -> Result<u64, Error> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .map_err(|_| Error::Invalid)
}
pub async fn tick(config: &Config, check_only: bool) -> Result<(), Error> {
    config.prepare()?;
    let _lock = OperationLock::acquire(&config.state_dir.join("operation.lock"))?;
    if config
        .state_dir
        .join("reboot-grant.json")
        .try_exists()
        .map_err(|_| Error::Storage)?
    {
        return Ok(());
    }
    let policy = control::refresh_policy(
        &config.socket_path,
        &config.state_dir.join("effective-policy.json"),
    )
    .await?;
    let host = NativeHost { config, policy };
    if !check_only {
        if let Some(mut tx) = apply::recover::load(&host)? {
            apply::recover::resume(&host, &mut tx).await?;
        }
    }
    let path = config.state_dir.join("status.json");
    let mut status = journal::load::<Status>(&path)?.unwrap_or_default();
    let time = now()?;
    if !check_only
        && !policy::due(
            status.last_check,
            time,
            std::time::Duration::from_secs(u64::from(policy.check_interval_seconds)),
        )
    {
        return Ok(());
    }
    status.last_check = Some(time);
    journal::save(&path, &status)?;
    let result = discover_and_apply(config, &host, &mut status, check_only).await;
    if result.is_err() {
        let mut latest = journal::load::<Status>(&path)?.unwrap_or_default();
        latest.error = Some("delivery_failed".into());
        journal::save(&path, &latest)?;
    }
    result
}
async fn discover_and_apply(
    config: &Config,
    host: &NativeHost<'_>,
    status: &mut Status,
    check_only: bool,
) -> Result<(), Error> {
    let network = Network::new(config)?;
    let Some(candidate) = discovery::discover(config, &network, status).await? else {
        return Ok(());
    };
    let descriptor = candidate.delivery.descriptor();
    status.desired_build = Some(descriptor.build.clone());
    status.error = None;
    journal::save(&config.state_dir.join("status.json"), status)?;
    if check_only {
        return Ok(());
    }
    let _manifest = discovery::manifest(&candidate, &network).await?;
    crate::staging::run(&crate::staging::NativeHost { config, descriptor }).await?;
    let unit = apply::native::read_unit(&config.unit_path)?;
    let previous = podman::unit_identity(&unit)?;
    // Ensure the old executable remains locally available for snapshot and recovery.
    podman::podman(vec![
        "image".into(),
        "exists".into(),
        previous.image.clone().into(),
    ])
    .await?;
    let id = Uuid::new_v4();
    let backup_parent = config.backup_dir.join(id.to_string());
    private_directory(&backup_parent)?;
    let previous_unit = config.state_dir.join(format!("previous-{id}.container"));
    podman::write_unit(&previous_unit, &unit)?;
    let mut tx = Transaction {
        version: 1,
        id,
        phase: Phase::Staged,
        previous_image: previous.image,
        previous_build: previous.build,
        candidate_image: descriptor.image.reference.clone(),
        candidate_build: descriptor.build.clone(),
        sequence: descriptor.sequence,
        backup: backup_parent.join("snapshot"),
        candidate_export: backup_parent.join("candidate.tar"),
        previous_unit,
    };
    host.record(&tx)?;
    match apply::run(host, &mut tx, host.policy).await {
        Ok(apply::Outcome::Committed) => crate::retention::cleanup(config, &tx).await,
        Ok(_) => Ok(()),
        Err(error) => {
            // Recovery from stop/backup failure can restart old code; Exposing never restores.
            if matches!(tx.phase, Phase::Stopped) {
                apply::recover::resume(host, &mut tx).await?;
            }
            Err(error)
        }
    }
}
pub async fn prepare_start(config: &Config) -> Result<(), Error> {
    config.prepare()?;
    let _lock = OperationLock::acquire(&config.state_dir.join("operation.lock"))?;
    let policy = journal::load::<UpdatePolicy>(&config.state_dir.join("effective-policy.json"))?
        .unwrap_or_default();
    policy.validate().map_err(|_| Error::Invalid)?;
    let host = NativeHost { config, policy };
    if let Some(mut tx) = apply::recover::load(&host)? {
        if !matches!(
            tx.phase,
            Phase::Committed | Phase::Failed | Phase::Quarantined | Phase::Staged
        ) {
            let running = podman::podman(vec![
                "ps".into(),
                "--filter".into(),
                "name=^hmail-center$".into(),
                "--format".into(),
                "{{.Names}}".into(),
            ])
            .await?;
            if !running.iter().all(|byte| byte.is_ascii_whitespace()) {
                return Err(Error::Conflict);
            }
        }
        apply::recover::prepare(&host, &mut tx).await?;
    }
    Ok(())
}
