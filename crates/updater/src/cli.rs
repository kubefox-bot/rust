use astra_updater::{config::Config, Error};
use std::path::PathBuf;
pub async fn run() -> std::process::ExitCode {
    match execute().await {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            std::process::ExitCode::FAILURE
        }
    }
}
async fn execute() -> Result<(), Error> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() == 1 && args[0] == "version" {
        println!(
            "{}",
            serde_json::json!({"version":1,"build":option_env!("HMAIL_BUILD_VERSION").unwrap_or("development")})
        );
        return Ok(());
    }
    if args.len() != 3 || args[1] != "--config" {
        return Err(Error::Invalid);
    }
    let command = args[0].to_str().ok_or(Error::Invalid)?;
    let config = Config::load(&PathBuf::from(&args[2]))?;
    match command {
        "coordinate-reboot" => astra_updater::reboot::serve(&config).await,
        "check" => astra_updater::run::tick(&config, true).await,
        "apply" | "resume" => astra_updater::run::tick(&config, false).await,
        "prepare-start" => astra_updater::run::prepare_start(&config).await,
        "status" => {
            if let Some(status) = astra_updater::journal::load::<astra_updater::journal::Status>(
                &config.state_dir.join("status.json"),
            )? {
                println!(
                    "{}",
                    serde_json::to_string(&status).map_err(|_| Error::Invalid)?
                );
            }
            Ok(())
        }
        _ => Err(Error::Invalid),
    }
}
