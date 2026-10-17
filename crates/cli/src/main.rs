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

#[tokio::main]
async fn main() -> ExitCode {
    #[cfg(feature = "agent")]
    {
        let arguments = std::env::args_os().skip(1).collect::<Vec<_>>();
        let is_agent = arguments
            .first()
            .is_some_and(|argument| argument == "agent");
        if is_agent || !cfg!(feature = "local-cli") {
            let skip = usize::from(is_agent);
            return hmail_cleaner_agent::cli::run(
                arguments.into_iter().skip(skip),
                env!("HMAIL_BUILD_VERSION"),
            )
            .await;
        }
    }
    #[cfg(feature = "local-cli")]
    {
        local::run().await
    }
    #[cfg(not(feature = "local-cli"))]
    {
        eprintln!("No application component enabled");
        ExitCode::from(2)
    }
}
