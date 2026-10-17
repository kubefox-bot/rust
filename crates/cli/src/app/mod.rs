use crate::{config, output::Output};
use hmail_cleaner_core::cleanup::{Cancellation, Progress, RunOptions};
pub(crate) use hmail_cleaner_runner::RunResult;
use hmail_cleaner_runner::{RunObserver, RunRequest};

struct Observer<'a> {
    output: &'a Output,
    config: &'a config::Config,
}

impl RunObserver for Observer<'_> {
    fn begin(&self, _: &RunRequest) {
        self.output.begin(self.config);
    }
    fn progress(&self, progress: Progress) {
        self.output.progress(progress);
    }
    fn server(&self, version: &str, cutoff: chrono::NaiveDateTime) {
        self.output.server(version, cutoff);
    }
    fn mail_state(&self, state: &str) {
        self.output.mail_state(state);
    }
    fn action(&self, message: &str) {
        self.output.action(message);
    }
    fn failed(&self) -> bool {
        self.output.failed()
    }
}

pub(crate) async fn run_once(
    config: &config::Config,
    output: &Output,
    cancel: &Cancellation,
) -> RunResult {
    let request = RunRequest {
        database: config.database.clone(),
        options: RunOptions {
            cutoff: config.options.cutoff,
            file_cutoff: config.options.file_cutoff,
            batch_size: config.options.batch_size,
            workers: config.options.workers,
            data_directory: config.options.data_directory.clone(),
        },
        scan: config.scan,
        action: config.mode.action(),
        service_policy: config.service_policy,
        hmail_ini: config.hmail_ini.clone(),
        service_name: config.service_name.clone(),
    };
    hmail_cleaner_runner::run_once(&request, &Observer { output, config }, cancel).await
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        args::{Args, Mode},
        config::{RunIntent, ServicePolicy},
        output::OutputPolicy,
    };
    use clap::Parser;
    use std::{collections::BTreeMap, sync::Arc};
    #[tokio::test]
    async fn preflight_failure_never_stops_mail() {
        let temp = tempfile::tempdir().unwrap();
        let ini = temp.path().join("invalid.ini");
        std::fs::write(&ini, "[Directories]\n").unwrap();
        let args = Args::try_parse_from(["hmail-cleaner", "clean", "--manage-service"]).unwrap();
        let source = BTreeMap::from([
            (
                "DATABASE_URL".into(),
                "mysql://fixture:dummy@127.0.0.1:1/hmaildb".into(),
            ),
            ("HMAIL_DATA_DIR".into(), temp.path().display().to_string()),
            ("HMAIL_INI".into(), ini.display().to_string()),
            ("HMAIL_OLDER_THAN".into(), "30d".into()),
        ]);
        let settings = config::load_with_sources(&args, &source, &BTreeMap::new()).unwrap();
        let config = settings
            .for_run(
                RunIntent {
                    mode: Mode::Clean,
                    scan: settings.scan,
                    service_policy: ServicePolicy::Manage,
                },
                chrono::Local::now(),
            )
            .unwrap();
        let cancel = Arc::new(Cancellation::default());
        let log = temp.path().join("run.ndjson");
        let output = Output::open(OutputPolicy::Silent, Some(&log), cancel.clone()).unwrap();
        let result = run_once(&config, &output, &cancel).await;
        assert!(!result.is_fatal());
        let report = result.into_report();
        assert_eq!(report.exit_code, 2);
        assert!(report.error.unwrap().contains("DataFolder"));
    }
}
