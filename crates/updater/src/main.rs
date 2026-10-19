#[cfg(unix)]
mod cli;
#[cfg(unix)]
#[tokio::main(flavor = "current_thread")]
async fn main() -> std::process::ExitCode {
    cli::run().await
}
#[cfg(not(unix))]
fn main() -> std::process::ExitCode {
    eprintln!("astra-updater requires a Linux host");
    std::process::ExitCode::FAILURE
}
