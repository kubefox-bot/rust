use super::{
    metadata, package, security, InstallError, InstallServiceInput, InstallationReceipt, Presence,
    ServiceRegistration, ServiceRegistry, ServiceState,
};
use crate::jobs::JobRepository;
pub use hmail_cleaner_core::maintenance::mail_service::AGENT_SERVICE_NAME as SERVICE_NAME;
use hmail_cleaner_core::maintenance::mail_service::CLEANER_SERVICE_NAME as LEGACY_SERVICE_NAME;

pub async fn install_with(
    input: InstallServiceInput,
    registry: &impl ServiceRegistry,
) -> Result<InstallationReceipt, InstallError> {
    if input.operation_id.is_some_and(|id| id.is_nil()) {
        return Err(InstallError::Invalid);
    }
    if [
        &input.package,
        &input.executable,
        &input.config,
        &input.data_directory,
    ]
    .iter()
    .any(|path| !path.is_absolute())
        || !input.config.is_file()
        || input.package == input.executable
    {
        return Err(InstallError::Invalid);
    }
    if package::digest(&input.package)? != input.expected_sha256 {
        return Err(InstallError::Invalid);
    }
    let registration = ServiceRegistration {
        name: SERVICE_NAME.into(),
        executable: input.executable.clone(),
        arguments: vec![
            "agent".into(),
            "service".into(),
            "--config".into(),
            input.config.as_os_str().into(),
        ],
    };
    let presence = registry.inspect(&registration)?;
    if presence == Presence::Collision {
        return Err(InstallError::Collision);
    }
    if matches!(presence, Presence::Owned(ServiceState::Transitioning)) {
        return Err(InstallError::Busy);
    }
    let legacy = match &input.legacy {
        Some(legacy) if legacy.name == LEGACY_SERVICE_NAME => {
            Some((legacy, registry.inspect(legacy)?))
        }
        Some(_) => return Err(InstallError::Invalid),
        None => {
            // The registry must reject an unrecognized legacy service before activation.
            let unknown = ServiceRegistration {
                name: LEGACY_SERVICE_NAME.into(),
                executable: input.executable.clone(),
                arguments: Vec::new(),
            };
            if registry.inspect(&unknown)? != Presence::Absent {
                return Err(InstallError::Collision);
            }
            None
        }
    };
    if legacy.is_some_and(|(_, presence)| presence == Presence::Collision) {
        return Err(InstallError::Collision);
    }
    security::directory(&input.data_directory)?;
    let lock = super::acquire_update_lock(&input.data_directory)?;
    let repository = JobRepository::open(&input.data_directory.join("jobs.sqlite")).await?;
    if !repository.begin_maintenance().await? {
        repository.close().await;
        return Err(InstallError::Busy);
    }
    let prior = metadata::read(&input.data_directory, &input.executable)?;
    package::validate(&input.executable, prior.as_ref())?;
    if let Some((legacy, presence)) = legacy {
        match presence {
            Presence::Absent => {}
            Presence::Owned(ServiceState::Running | ServiceState::Transitioning) => {
                registry.stop_and_wait(legacy)?;
                registry.disable(legacy)?;
            }
            Presence::Owned(ServiceState::Stopped) => registry.disable(legacy)?,
            Presence::Collision => return Err(InstallError::Collision),
        }
    }
    if matches!(presence, Presence::Owned(ServiceState::Running)) {
        registry.stop_and_wait(&registration)?;
    }
    security::directory(input.executable.parent().ok_or(InstallError::Invalid)?)?;
    let previous_sha256 = package::publish(
        &input.package,
        &input.executable,
        input.expected_sha256,
        prior.as_ref(),
    )?;
    let receipt = InstallationReceipt {
        operation_id: input.operation_id,
        executable: input.executable,
        sha256: package::hex(&input.expected_sha256),
        previous_sha256,
        started: false,
    };
    metadata::write(&input.data_directory, &receipt)?;
    repository.end_maintenance().await?;
    repository.close().await;
    registry.configure_and_start(&registration)?;
    let receipt = InstallationReceipt {
        started: true,
        ..receipt
    };
    metadata::write(&input.data_directory, &receipt)?;
    drop(lock);
    Ok(receipt)
}
