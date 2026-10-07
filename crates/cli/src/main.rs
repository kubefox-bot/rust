mod args;
mod commands;
mod config;
mod output;
use args::{Args, Mode};
use clap::Parser;
use commands::Report;
use hmail_cleaner_adapters::{
    filesystem::LocalFiles,
    mysql::MySqlRepository,
    offline::{verify_data_folder, WindowsOffline},
};
use hmail_cleaner_core::cleanup::{Cancellation, Dependencies};
use std::{process::ExitCode, sync::Arc};

#[tokio::main]
async fn main() -> ExitCode {
    let args = match Args::try_parse() {
        Ok(args) => args,
        Err(error) => {
            let code = error.exit_code() as u8;
            let _ = error.print();
            return ExitCode::from(code);
        }
    };
    let cancel = Arc::new(Cancellation::default());
    let output = output::Output::new(args.options().json, cancel.clone());
    let config = match config::load_config(&args) {
        Ok(config) => config,
        Err(error) => {
            output.finish(&Report::invalid(error));
            return ExitCode::from(2);
        }
    };
    let signal = if config.mode == Mode::Clean {
        let cancel = cancel.clone();
        Some(tokio::spawn(async move {
            if let Err(error) = tokio::signal::ctrl_c().await {
                eprintln!("Ctrl+C handler failed: {error}");
            }
            cancel.request();
        }))
    } else {
        None
    };
    let report = run(&config, &cancel, &output).await;
    if let Some(signal) = signal {
        signal.abort();
        let _ = signal.await;
    }
    output.finish(&report);
    ExitCode::from(if output.failed() { 1 } else { report.exit_code })
}
async fn run(config: &config::Config, cancel: &Cancellation, output: &output::Output) -> Report {
    if let Err(error) = verify_data_folder(&config.options.data_directory, &config.hmail_ini) {
        return Report::invalid(error);
    }
    let files = match LocalFiles::new(config.options.data_directory.clone()) {
        Ok(files) => files,
        Err(error) => return Report::invalid(error),
    };
    let guard = WindowsOffline::new(config.service_name.clone());
    // Native preflight runs before opening a database for a destructive command.
    if config.mode == Mode::Clean {
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
    let report = commands::execute(
        config.mode,
        Dependencies {
            repository: &mut repository,
            files: &files,
            guard: &guard,
        },
        &config.options,
        cancel,
        |progress| output.progress(progress),
    )
    .await;
    match repository.close().await {
        Ok(()) => report,
        Err(error) => report.with_close_failure(error),
    }
}
