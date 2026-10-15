mod runner;
mod schedule;
use crate::{
    app::{self, RunResult},
    args::Args,
    commands::Report,
    config::{self, RunIntent},
    output::{Output, OutputPolicy},
};
use hmail_cleaner_core::cleanup::{Cancellation, PortResult};
use runner::{ScheduledJob, SystemClock};
use std::sync::Arc;
const SERVICE_NAME: &str = "hMailCleaner";
struct Job {
    settings: config::Settings,
    output: Output,
}
impl ScheduledJob for Job {
    async fn execute(&mut self, intent: RunIntent, stop: &Cancellation) -> RunResult {
        let result = match self.settings.for_run(intent, chrono::Local::now()) {
            Ok(config) => app::run_once(&config, &self.output, stop).await,
            Err(error) => RunResult::Finished(Report::invalid(error)),
        };
        let fatal = result.is_fatal();
        let report = result.into_report();
        self.output.finish(&report);
        if fatal || self.output.failed() {
            RunResult::Fatal(report)
        } else {
            RunResult::Finished(report)
        }
    }
}
pub(crate) fn start(args: &Args) -> PortResult<()> {
    let settings = config::load_service_settings(args)?;
    // Validate the sink before entering the dispatcher; the callback owns its final handle.
    drop(Output::open(
        OutputPolicy::Silent,
        settings.cleanup.log_file.as_deref(),
        Arc::new(Cancellation::default()),
    )?);
    hmail_cleaner_adapters::service::host(
        SERVICE_NAME,
        Box::new(move |stop| {
            let output = Output::open(
                OutputPolicy::Silent,
                settings.cleanup.log_file.as_deref(),
                stop.clone(),
            )?;
            let mut job = Job {
                settings: settings.cleanup.clone(),
                output,
            };
            let runtime = tokio::runtime::Runtime::new()?;
            runtime.block_on(runner::run(&settings, &SystemClock, &mut job, stop))
        }),
    )
}
