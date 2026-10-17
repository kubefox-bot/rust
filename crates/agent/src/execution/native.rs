use super::Executor;
use hmail_cleaner_adapters::mysql::DatabaseConfig;
use hmail_cleaner_core::{
    cleanup::{Cancellation, RunOptions},
    maintenance::{recursive::Action, ScanOptions},
};
use hmail_cleaner_protocol::{Operation, Scan, ServicePolicy, ValidatedJob};
use hmail_cleaner_runner::{Report, RunObserver, RunRequest, RunResult};
use std::{num::NonZeroU32, path::PathBuf};

pub struct NativeExecutor;
impl Executor for NativeExecutor {
    async fn execute(
        &self,
        job: &ValidatedJob,
        observer: &impl RunObserver,
        cancel: &Cancellation,
    ) -> RunResult {
        execute(job, observer, cancel).await
    }
}
pub async fn execute(
    job: &ValidatedJob,
    observer: &impl RunObserver,
    cancel: &Cancellation,
) -> RunResult {
    let job = job.command();
    let database = match DatabaseConfig::from_parts(&job.mail_profile.mysql) {
        Ok(database) => database,
        Err(_) => return RunResult::Finished(Report::invalid("Invalid database profile")),
    };
    let request = RunRequest {
        database,
        options: RunOptions {
            cutoff: job.resolved_cutoff.local,
            file_cutoff: job.resolved_cutoff.file_cutoff_utc.into(),
            batch_size: NonZeroU32::new(job.limits.batch_size).expect("validated nonzero batch"),
            workers: job.limits.workers,
            data_directory: PathBuf::from(&job.mail_profile.data_dir),
        },
        scan: match job.scan {
            Scan::Fast => ScanOptions::Fast,
            Scan::Full => ScanOptions::Full,
        },
        action: match job.operation {
            Operation::Preview => Action::Preview,
            Operation::Clean => Action::Clean,
        },
        service_policy: match job.service_policy {
            ServicePolicy::RequireStopped => hmail_cleaner_runner::ServicePolicy::RequireStopped,
            ServicePolicy::Manage => hmail_cleaner_runner::ServicePolicy::Manage,
        },
        hmail_ini: PathBuf::from(&job.mail_profile.hmail_ini),
        service_name: job.mail_profile.service_name.clone(),
    };
    hmail_cleaner_runner::run_once(&request, observer, cancel).await
}
