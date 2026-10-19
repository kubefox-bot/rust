mod args;
use crate::{
    config::AgentConfig,
    enrollment::{self, AgentBootstrap},
    service::{self, InstallServiceInput, ServiceRegistration},
};
use args::{Args, Command};
use clap::{CommandFactory, FromArgMatches};
use hmail_cleaner_core::cleanup::Cancellation;
use std::{ffi::OsString, path::PathBuf, process::ExitCode, sync::Arc};

pub async fn run(
    arguments: impl Iterator<Item = OsString>,
    build_version: &'static str,
) -> ExitCode {
    let parser = Args::command().version(build_version);
    let argv = std::iter::once(OsString::from("hmail-cleaner agent")).chain(arguments);
    let matches = match parser.try_get_matches_from(argv) {
        Ok(matches) => matches,
        Err(error) => {
            let code = error.exit_code() as u8;
            let _ = error.print();
            return ExitCode::from(code);
        }
    };
    let command = match Args::from_arg_matches(&matches) {
        Ok(args) => args.command,
        Err(_) => return ExitCode::from(2),
    };
    match execute(command, build_version).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Agent operation failed: {error}");
            ExitCode::FAILURE
        }
    }
}
async fn execute(
    command: Command,
    build_version: &'static str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match command {
        Command::Enroll { data_dir, agent_id } => {
            service::security::directory(&data_dir)?;
            let csr = enrollment::prepare(&AgentBootstrap {
                data_directory: data_dir,
                agent_id,
            })?;
            serde_json::to_writer(std::io::stdout().lock(), &csr)?;
            Ok(())
        }
        Command::Run { config } => {
            let config = AgentConfig::load(&config, build_version)?;
            let stop = Arc::new(Cancellation::default());
            let signal_stop = stop.clone();
            let signal = tokio::spawn(async move {
                if tokio::signal::ctrl_c().await.is_ok() {
                    signal_stop.request();
                }
            });
            let result = crate::run(config, stop).await;
            signal.abort();
            let _ = signal.await;
            result.map_err(Into::into)
        }
        Command::Service { config } => host(config, build_version),
        Command::PrepareRecovery { config } => {
            let validated = AgentConfig::load(&config, build_version)?;
            service::prepare_recovery(&validated.data_directory).await?;
            serde_json::to_writer(std::io::stdout().lock(), &serde_json::json!({"ok":true}))?;
            Ok(())
        }
        Command::InstallService {
            operation_id,
            package,
            executable,
            config,
            sha256,
            legacy_executable,
            legacy_env,
        } => {
            let validated = AgentConfig::load(&config, build_version)?;
            let legacy = match (legacy_executable, legacy_env) {
                (Some(executable), Some(env)) if executable.is_absolute() && env.is_absolute() => {
                    Some(ServiceRegistration {
                        name: "hMailCleaner".into(),
                        executable,
                        arguments: vec![
                            "service".into(),
                            "run".into(),
                            "--env-file".into(),
                            env.into_os_string(),
                        ],
                    })
                }
                (None, None) => None,
                _ => return Err(service::InstallError::Invalid.into()),
            };
            let receipt = service::install(InstallServiceInput {
                operation_id,
                package,
                executable,
                config,
                data_directory: validated.data_directory,
                expected_sha256: sha256,
                legacy,
            })
            .await?;
            serde_json::to_writer(std::io::stdout().lock(), &receipt)?;
            Ok(())
        }
    }
}
fn host(
    path: PathBuf,
    build_version: &'static str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // The existing SCM host owns STOP and waits until the async worker restores mail.
    let runtime = tokio::runtime::Handle::current();
    hmail_cleaner_adapters::service::host(
        service::SERVICE_NAME,
        Box::new(move |stop| {
            let config = AgentConfig::load(&path, build_version)?;
            runtime
                .block_on(crate::run(config, stop))
                .map_err(Into::into)
        }),
    )
}
