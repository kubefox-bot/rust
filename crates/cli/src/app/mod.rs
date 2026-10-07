use crate::{
    args::Mode,
    commands::{self, Report},
    config, output,
};
use hmail_cleaner_adapters::{
    filesystem::LocalFiles,
    mysql::MySqlRepository,
    offline::{verify_data_folder, WindowsOffline},
};
use hmail_cleaner_core::cleanup::{Cancellation, Dependencies};
pub(crate) async fn run_once(
    config: &config::Config,
    output: &output::Output,
    cancel: &Cancellation,
) -> Report {
    if let Err(error) = verify_data_folder(&config.options.data_directory, &config.hmail_ini) {
        return Report::invalid(error);
    }
    let files = match LocalFiles::new(config.options.data_directory.clone()) {
        Ok(files) => files,
        Err(error) => return Report::invalid(error),
    };
    let guard = WindowsOffline::new(config.service_name.clone());
    // Native preflight runs before opening a database for a destructive command.
    if config.mode == Mode::Clean
        || matches!(
            config.scan,
            hmail_cleaner_core::maintenance::ScanOptions::Full { .. }
        )
    {
        use hmail_cleaner_core::cleanup::OfflineGuard;
        if let Err(error) = guard.ensure_stopped().await {
            return Report::invalid(error);
        }
    }
    let mut repository = match MySqlRepository::connect(&config.database).await {
        Ok(repository) => repository,
        Err(error) => return Report::invalid(error),
    };
    output.server(repository.server_version(), config.options.cutoff);
    if config.mode == Mode::Clean {
        if let Err(error) = repository.verify_delete_access().await {
            let report = Report::invalid(error);
            return match repository.close().await {
                Ok(()) => report,
                Err(error) => report.with_close_failure(error),
            };
        }
    }
    let report = commands::execute_scan(
        config.mode,
        Dependencies {
            repository: &mut repository,
            files: &files,
            guard: &guard,
        },
        &config.options,
        config.scan,
        cancel,
        |progress| output.progress(progress),
    )
    .await;
    match repository.close().await {
        Ok(()) => report,
        Err(error) => report.with_close_failure(error),
    }
}
