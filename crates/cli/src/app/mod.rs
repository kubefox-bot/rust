mod lifecycle;
use crate::{
    args::Mode,
    commands::{self, Report},
    config::{self, ServicePolicy},
    output::Output,
};
use hmail_cleaner_adapters::{
    filesystem::LocalFiles,
    mysql::MySqlRepository,
    offline::{verify_data_folder, WindowsMailControl, WindowsOffline},
};
use hmail_cleaner_core::cleanup::{Cancellation, Dependencies, PortResult};
pub(crate) use lifecycle::RunResult;
struct Prepared<'a> {
    config: &'a config::Config,
    output: &'a Output,
    cancel: &'a Cancellation,
    repository: MySqlRepository,
    files: LocalFiles,
    guard: WindowsOffline,
}
impl lifecycle::PreparedRun for Prepared<'_> {
    async fn execute(&mut self) -> Report {
        self.output.mail_state(
            if self.config.scan.requires_stopped(self.config.mode.action()) {
                "остановлена; обработка файлов"
            } else {
                "быстрый предпросмотр без управления службой"
            },
        );
        commands::execute_scan(
            self.config.mode,
            Dependencies {
                repository: &mut self.repository,
                files: &self.files,
                guard: &self.guard,
            },
            &self.config.options,
            self.config.scan,
            self.cancel,
            |p| self.output.progress(p),
        )
        .await
    }
    fn mail_state(&self, state: &str) {
        self.output.mail_state(state);
    }
    fn action(&self, message: &str) {
        self.output.action(message);
    }
    fn output_failed(&self) -> bool {
        self.output.failed()
    }
    async fn close(self) -> PortResult<()> {
        self.repository.close().await
    }
}
pub(crate) async fn run_once(
    config: &config::Config,
    output: &Output,
    cancel: &Cancellation,
) -> RunResult {
    if output.failed() {
        return RunResult::Fatal(Report::invalid("Вывод недоступен"));
    }
    output.begin(config);
    let prepared = match prepare(config, output, cancel).await {
        Ok(prepared) => prepared,
        Err(report) => return RunResult::Finished(*report),
    };
    let (prepared, control) = prepared;
    lifecycle::run(prepared, control.as_ref(), cancel).await
}
async fn prepare<'a>(
    config: &'a config::Config,
    output: &'a Output,
    cancel: &'a Cancellation,
) -> Result<(Prepared<'a>, Option<WindowsMailControl>), Box<Report>> {
    verify_data_folder(&config.options.data_directory, &config.hmail_ini).map_err(|error| {
        Report::invalid(format!(
            "Не удалось проверить DataFolder и INI {}: {error}",
            config.hmail_ini.display()
        ))
    })?;
    let files = LocalFiles::new(config.options.data_directory.clone()).map_err(|error| {
        Report::invalid(format!(
            "Не удалось открыть корень {}: {error}",
            config.options.data_directory.display()
        ))
    })?;
    if config.mode == Mode::Preview && config.service_policy == ServicePolicy::Manage {
        return Err(Box::new(Report::invalid(
            "--manage-service допустим только для clean",
        )));
    }
    let control = match config.service_policy {
        ServicePolicy::Manage => {
            Some(WindowsMailControl::new(&config.service_name).map_err(Report::invalid)?)
        }
        ServicePolicy::RequireStopped => None,
    };
    let guard = WindowsOffline::new(config.service_name.clone());
    if control.is_none() && config.scan.requires_stopped(config.mode.action()) {
        use hmail_cleaner_core::cleanup::OfflineGuard;
        guard.ensure_stopped().await.map_err(|error| {
            Report::invalid(format!(
                "Для работы почтовая служба должна быть остановлена: {error}"
            ))
        })?;
    }
    let mut repository = MySqlRepository::connect(&config.database)
        .await
        .map_err(|error| {
            Report::invalid(format!(
                "Не удалось подключиться к БД или проверить её схему: {error}"
            ))
        })?;
    output.server(repository.server_version(), config.options.cutoff);
    if config.mode == Mode::Clean {
        if let Err(error) = repository.verify_delete_access().await {
            let report = Report::invalid(error);
            return Err(Box::new(match repository.close().await {
                Ok(()) => report,
                Err(error) => report.with_close_failure(error),
            }));
        }
    }
    if output.failed() || cancel.is_requested() {
        let report = Report::invalid("Запуск отменён до остановки почтовой службы");
        return Err(Box::new(match repository.close().await {
            Ok(()) => report,
            Err(error) => report.with_close_failure(error),
        }));
    }
    Ok((
        Prepared {
            config,
            output,
            cancel,
            repository,
            files,
            guard,
        },
        control,
    ))
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        args::Args,
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
