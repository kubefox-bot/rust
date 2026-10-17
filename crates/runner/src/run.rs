use crate::{
    lifecycle::{self, PreparedRun},
    report, Mode, Report, RunObserver, RunRequest, RunResult, ServicePolicy,
};
use hmail_cleaner_adapters::{
    filesystem::LocalFiles,
    mysql::MySqlRepository,
    offline::{verify_data_folder, WindowsMailControl, WindowsOffline},
};
use hmail_cleaner_core::{
    cleanup::{Cancellation, Dependencies, PortResult},
    maintenance::recursive::{self, Action},
};

struct Prepared<'a, O> {
    request: &'a RunRequest,
    observer: &'a O,
    cancel: &'a Cancellation,
    repository: MySqlRepository,
    files: LocalFiles,
    guard: WindowsOffline,
}

impl<O: RunObserver> PreparedRun for Prepared<'_, O> {
    async fn execute(&mut self) -> Report {
        self.observer
            .mail_state(if self.request.scan.requires_stopped(self.request.action) {
                "остановлена; обработка файлов"
            } else {
                "быстрый предпросмотр без управления службой"
            });
        let result = recursive::run(
            Dependencies {
                repository: &mut self.repository,
                files: &self.files,
                guard: &self.guard,
            },
            &self.request.options,
            self.request.scan,
            self.cancel,
            |progress| self.observer.progress(progress),
            self.request.action,
        )
        .await;
        report::from_result(Mode::from_action(self.request.action), result)
    }
    fn mail_state(&self, state: &str) {
        self.observer.mail_state(state);
    }
    fn action(&self, message: &str) {
        self.observer.action(message);
    }
    fn output_failed(&self) -> bool {
        self.observer.failed()
    }
    async fn close(self) -> PortResult<()> {
        self.repository.close().await
    }
}

pub async fn run_once(
    request: &RunRequest,
    observer: &impl RunObserver,
    cancel: &Cancellation,
) -> RunResult {
    if observer.failed() {
        return RunResult::Fatal(Report::invalid("Вывод недоступен"));
    }
    observer.begin(request);
    let (prepared, control) = match prepare(request, observer, cancel).await {
        Ok(prepared) => prepared,
        Err(report) => return RunResult::Finished(*report),
    };
    let observed = control
        .as_ref()
        .map(|control| crate::service_observer::ObservedControl { control, observer });
    lifecycle::run(prepared, observed.as_ref(), cancel).await
}

async fn prepare<'a, O: RunObserver>(
    request: &'a RunRequest,
    observer: &'a O,
    cancel: &'a Cancellation,
) -> Result<(Prepared<'a, O>, Option<WindowsMailControl>), Box<Report>> {
    verify_data_folder(&request.options.data_directory, &request.hmail_ini).map_err(|error| {
        Report::invalid(format!(
            "Не удалось проверить DataFolder и INI {}: {error}",
            request.hmail_ini.display()
        ))
    })?;
    let files = LocalFiles::new(request.options.data_directory.clone()).map_err(|error| {
        Report::invalid(format!(
            "Не удалось открыть корень {}: {error}",
            request.options.data_directory.display()
        ))
    })?;
    if request.action == Action::Preview && request.service_policy == ServicePolicy::Manage {
        return Err(Box::new(Report::invalid(
            "--manage-service допустим только для clean",
        )));
    }
    let control = match request.service_policy {
        ServicePolicy::Manage => {
            Some(WindowsMailControl::new(&request.service_name).map_err(Report::invalid)?)
        }
        ServicePolicy::RequireStopped => None,
    };
    let guard = WindowsOffline::new(request.service_name.clone());
    if control.is_none() && request.scan.requires_stopped(request.action) {
        use hmail_cleaner_core::cleanup::OfflineGuard;
        guard.ensure_stopped().await.map_err(|error| {
            Report::invalid(format!(
                "Для работы почтовая служба должна быть остановлена: {error}"
            ))
        })?;
        observer.service_state(crate::MailObservation::State(
            hmail_cleaner_core::maintenance::mail_service::MailState::Stopped,
        ));
    }
    let mut repository = MySqlRepository::connect(&request.database)
        .await
        .map_err(|error| {
            Report::invalid(format!(
                "Не удалось подключиться к БД или проверить её схему: {error}"
            ))
        })?;
    observer.server(repository.server_version(), request.options.cutoff);
    if request.action == Action::Clean {
        if let Err(error) = repository.verify_delete_access().await {
            return Err(Box::new(
                close_failed_prepare(repository, Report::invalid(error)).await,
            ));
        }
    }
    if observer.failed() || cancel.is_requested() {
        return Err(Box::new(
            close_failed_prepare(
                repository,
                Report::invalid("Запуск отменён до остановки почтовой службы"),
            )
            .await,
        ));
    }
    Ok((
        Prepared {
            request,
            observer,
            cancel,
            repository,
            files,
            guard,
        },
        control,
    ))
}

async fn close_failed_prepare(repository: MySqlRepository, report: Report) -> Report {
    match repository.close().await {
        Ok(()) => report,
        Err(error) => report.with_close_failure(error),
    }
}
