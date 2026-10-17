use crate::{
    account::{self, InitInput, SecretPrompt},
    config::{CenterArgs, CenterConfig},
    storage::{CenterStore, DataDirectory},
    vault::SecretText,
};
use clap::{CommandFactory, FromArgMatches, Parser, Subcommand};
use std::{
    ffi::OsString,
    io::{self, Write},
    path::PathBuf,
    process::ExitCode,
};
#[derive(Parser)]
struct Args {
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    Init {
        #[arg(long)]
        data_dir: PathBuf,
        #[arg(long)]
        origin: Option<String>,
        #[arg(long)]
        listen: Option<std::net::SocketAddr>,
    },
    Run {
        #[arg(long)]
        data_dir: PathBuf,
    },
}
struct TerminalPrompt;
fn line(label: &str) -> io::Result<String> {
    print!("{label}");
    io::stdout().flush()?;
    let mut value = String::new();
    io::stdin().read_line(&mut value)?;
    Ok(value.trim_end_matches(['\r', '\n']).to_owned())
}
impl SecretPrompt for TerminalPrompt {
    fn password(&mut self) -> Result<SecretText, String> {
        let password =
            SecretText::new(rpassword::prompt_password("Пароль: ").map_err(|_| "input")?);
        let confirmation =
            SecretText::new(rpassword::prompt_password("Повторите пароль: ").map_err(|_| "input")?);
        if *password != *confirmation {
            return Err("password mismatch".into());
        }
        Ok(password)
    }
    fn confirm_recovery(&mut self, recovery: &SecretText) -> Result<bool, String> {
        println!(
            "Сохраните ключ восстановления в безопасном месте:\n{}",
            recovery.as_str()
        );
        let confirmation = SecretText::new(
            rpassword::prompt_password("Введите сохранённый recovery key для подтверждения: ")
                .map_err(|_| "input")?,
        );
        Ok(*confirmation == **recovery)
    }
}
pub async fn run(
    arguments: impl Iterator<Item = OsString>,
    build_version: &'static str,
) -> ExitCode {
    let argv = std::iter::once(OsString::from("hmail-cleaner center")).chain(arguments);
    let matches = match Args::command()
        .version(build_version)
        .try_get_matches_from(argv)
    {
        Ok(value) => value,
        Err(error) => {
            let code = error.exit_code() as u8;
            let _ = error.print();
            return ExitCode::from(code);
        }
    };
    let args = match Args::from_arg_matches(&matches) {
        Ok(args) => args,
        Err(_) => return ExitCode::from(2),
    };
    match execute(args.command).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Center operation failed: {error}");
            ExitCode::FAILURE
        }
    }
}
async fn execute(command: Command) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let data_dir = match &command {
        Command::Init { data_dir, .. } | Command::Run { data_dir } => data_dir.clone(),
    };
    let config = CenterConfig::load(&CenterArgs { data_dir })?;
    match command {
        Command::Init { origin, listen, .. } => {
            let lease = DataDirectory::acquire(&config.data_dir)?;
            let store = CenterStore::open(&lease).await?;
            if store.account().await?.is_some() {
                return Err("Center is already initialized".into());
            }
            if origin.is_some() || listen.is_some() {
                CenterConfig::create(
                    &config.data_dir,
                    origin.as_deref().unwrap_or(config.origin.as_str()),
                    listen.unwrap_or(config.listen),
                )?;
            }
            let login = line("Логин суперадмина: ")?;
            account::initialize(&store, &InitInput { login }, &mut TerminalPrompt).await?;
            println!("Суперадмин создан. Ключ данных не оставлен в сессии.");
        }
        Command::Run { .. } => {
            let cancel = std::sync::Arc::new(hmail_cleaner_core::cleanup::Cancellation::default());
            let signal_cancel = cancel.clone();
            let signal = tokio::spawn(async move {
                if tokio::signal::ctrl_c().await.is_ok() {
                    signal_cancel.request();
                }
            });
            let result = crate::run(config, cancel).await;
            signal.abort();
            let _ = signal.await;
            result?;
        }
    }
    Ok(())
}
