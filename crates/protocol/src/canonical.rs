use crate::{Operation, Scan, ServicePolicy, ValidatedJob};

/// V1: fixed-order fields, big-endian integers, UUID bytes, UTF-8 with u32 lengths.
/// Credential values are included only for HMAC input, never stored as a receipt.
pub fn canonical_job(job: &ValidatedJob) -> Vec<u8> {
    let command = job.command();
    let mut bytes = b"hmail-cleaner/job/1".to_vec();
    bytes.extend_from_slice(&command.protocol_version.to_be_bytes());
    for id in [
        command.job_id.uuid(),
        command.agent_id.uuid(),
        command.profile_id.uuid(),
    ] {
        bytes.extend_from_slice(id.as_bytes());
    }
    bytes.extend_from_slice(&command.profile_version.get().to_be_bytes());
    bytes.push(match command.operation {
        Operation::Preview => 1,
        Operation::Clean => 2,
    });
    bytes.push(match command.scan {
        Scan::Fast => 1,
        Scan::Full => 2,
    });
    text(&mut bytes, &command.resolved_cutoff.local.to_string());
    text(
        &mut bytes,
        &command.resolved_cutoff.file_cutoff_utc.to_rfc3339(),
    );
    bytes.extend_from_slice(&command.resolved_cutoff.utc_offset_seconds.to_be_bytes());
    text(
        &mut bytes,
        &command.resolved_cutoff.resolved_at_utc.to_rfc3339(),
    );
    bytes.push(match command.service_policy {
        ServicePolicy::RequireStopped => 1,
        ServicePolicy::Manage => 2,
    });
    bytes.extend_from_slice(&command.limits.batch_size.to_be_bytes());
    bytes.push(command.limits.workers);
    let profile = &command.mail_profile;
    for value in [
        &profile.mysql.host,
        &profile.mysql.database,
        &profile.mysql.username,
        &*profile.mysql.password,
        &profile.data_dir,
        &profile.hmail_ini,
        &profile.service_name,
    ] {
        text(&mut bytes, value);
    }
    bytes.extend_from_slice(&profile.mysql.port.to_be_bytes());
    bytes
}
fn text(bytes: &mut Vec<u8>, value: &str) {
    bytes.extend_from_slice(&(value.len() as u32).to_be_bytes());
    bytes.extend_from_slice(value.as_bytes());
}
