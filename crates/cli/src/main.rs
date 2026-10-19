#[cfg(feature = "local-cli")]
mod app;
#[cfg(feature = "local-cli")]
mod args;
#[cfg(feature = "local-cli")]
mod commands;
#[cfg(feature = "local-cli")]
mod config;
#[cfg(feature = "local-cli")]
mod local;
#[cfg(feature = "local-cli")]
mod menu;
#[cfg(feature = "local-cli")]
mod output;
use std::process::ExitCode;
#[cfg(feature = "agent")]
const AGENT_WORKER_THREADS: usize = 2;

fn main() -> ExitCode {
    #[cfg(feature = "center")]
    {
        let arguments = std::env::args_os().skip(1).collect::<Vec<_>>();
        let is_center = arguments
            .first()
            .is_some_and(|argument| argument == "center");
        if is_center || !cfg!(any(feature = "local-cli", feature = "agent")) {
            return run(
                hmail_cleaner_center::cli::run(
                    arguments.into_iter().skip(usize::from(is_center)),
                    env!("HMAIL_BUILD_VERSION"),
                ),
                &mut tokio::runtime::Builder::new_multi_thread(),
            );
        }
    }
    #[cfg(feature = "agent")]
    {
        let arguments = std::env::args_os().skip(1).collect::<Vec<_>>();
        let is_agent = arguments
            .first()
            .is_some_and(|argument| argument == "agent");
        if is_agent || !cfg!(feature = "local-cli") {
            let skip = usize::from(is_agent);
            return run(
                hmail_cleaner_agent::cli::run(
                    arguments.into_iter().skip(skip),
                    env!("HMAIL_BUILD_VERSION"),
                ),
                tokio::runtime::Builder::new_multi_thread().worker_threads(AGENT_WORKER_THREADS),
            );
        }
    }
    #[cfg(feature = "local-cli")]
    {
        run(
            local::run(),
            &mut tokio::runtime::Builder::new_multi_thread(),
        )
    }
    #[cfg(not(feature = "local-cli"))]
    {
        eprintln!("No application component enabled");
        ExitCode::from(2)
    }
}

#[cfg(any(feature = "local-cli", feature = "agent", feature = "center"))]
fn run(
    application: impl std::future::Future<Output = ExitCode>,
    runtime: &mut tokio::runtime::Builder,
) -> ExitCode {
    match runtime.enable_all().build() {
        Ok(runtime) => runtime.block_on(application),
        Err(error) => {
            eprintln!("Cannot start application runtime: {error}");
            ExitCode::FAILURE
        }
    }
}
