use chrono::NaiveDateTime;
use clap::{ArgGroup, Args as ClapArgs, Parser, Subcommand};
use hmail_cleaner_core::{
    cleanup::{MAX_BATCH_SIZE, MAX_WORKERS},
    selection::Retention,
};
pub(crate) use hmail_cleaner_runner::Mode;
use std::path::PathBuf;
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub(crate) enum Scan {
    Fast,
    Full,
}

#[derive(Debug, Parser)]
#[command(
    name = "hmail-cleaner",
    version = env!("HMAIL_BUILD_VERSION"),
    args_conflicts_with_subcommands = true,
    about = "Очистка hMailServer: сначала файлы, затем записи MySQL"
)]
pub(crate) struct Args {
    #[command(subcommand)]
    command: Option<Command>,
    #[command(flatten)]
    options: Options,
}
#[derive(Debug, Subcommand)]
enum Command {
    Service {
        #[command(subcommand)]
        command: ServiceCommand,
    },
    Menu(Options),
    #[command(about = "Предпросмотр файлов и записей без удаления")]
    Preview(Options),
    #[command(about = "Удаление старых писем при остановленном hMailServer")]
    Clean(Options),
}
#[derive(Debug, Subcommand)]
enum ServiceCommand {
    Run(Options),
}
#[derive(Debug, Default, ClapArgs)]
#[command(group(ArgGroup::new("retention").multiple(false)))]
pub(crate) struct Options {
    #[arg(long, value_enum)]
    pub scan: Option<Scan>,
    #[arg(long,group="retention",value_parser=crate::config::parse_age,help="Период хранения: 30d, 1mo (календарный месяц), 1y")]
    pub older_than: Option<Retention>,
    #[arg(long,group="retention",value_parser=crate::config::parse_before,help="Строгая граница местного времени: YYYY-MM-DD или YYYY-MM-DDTHH:MM:SS")]
    pub before: Option<NaiveDateTime>,
    #[arg(long)]
    pub data_dir: Option<PathBuf>,
    #[arg(long)]
    pub hmail_ini: Option<PathBuf>,
    #[arg(long)]
    pub service_name: Option<String>,
    #[arg(long,value_parser=clap::value_parser!(u32).range(1..=i64::from(MAX_BATCH_SIZE)))]
    pub batch_size: Option<u32>,
    #[arg(long,value_parser=clap::value_parser!(u8).range(1..=i64::from(MAX_WORKERS)))]
    pub workers: Option<u8>,
    #[arg(long)]
    pub log_file: Option<PathBuf>,
    #[arg(long)]
    pub manage_service: bool,
    #[arg(long, conflicts_with = "json")]
    pub silence: bool,
    #[arg(long, conflicts_with_all = ["json", "silence"], help = "Обычный текст без полноэкранной панели")]
    pub plain: bool,
    #[arg(
        long,
        help = "Файл настроек; по умолчанию cleaner.env рядом с EXE; окружение имеет приоритет"
    )]
    pub env_file: Option<PathBuf>,
    #[arg(long, help = "Машинный вывод NDJSON: события и итог")]
    pub json: bool,
}
impl Args {
    pub fn options(&self) -> &Options {
        match &self.command {
            Some(
                Command::Preview(options)
                | Command::Clean(options)
                | Command::Menu(options)
                | Command::Service {
                    command: ServiceCommand::Run(options),
                },
            ) => options,
            None => &self.options,
        }
    }
    pub fn mode(&self) -> Mode {
        match self.command {
            Some(Command::Clean(_)) => Mode::Clean,
            Some(Command::Preview(_) | Command::Menu(_) | Command::Service { .. }) | None => {
                Mode::Preview
            }
        }
    }
    pub fn is_menu(&self) -> bool {
        matches!(self.command, None | Some(Command::Menu(_)))
    }
    pub fn is_service(&self) -> bool {
        matches!(self.command, Some(Command::Service { .. }))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    fn argv(extra: &[&str]) -> Vec<String> {
        [
            "hmail-cleaner",
            "preview",
            "--data-dir",
            "/Data",
            "--hmail-ini",
            "/Bin/hMailServer.ini",
        ]
        .into_iter()
        .chain(extra.iter().copied())
        .map(str::to_owned)
        .collect()
    }
    #[test]
    fn retention_options_are_exclusive() {
        assert!(
            Args::try_parse_from(argv(&["--older-than", "30d", "--before", "2026-09-07"])).is_err()
        );
        assert!(Args::try_parse_from(argv(&[])).is_ok());
    }
    #[test]
    fn zero_retention_is_rejected() {
        for value in ["0d", "0mo", "0y", "30", "-1d"] {
            assert!(Args::try_parse_from(argv(&["--older-than", value])).is_err());
        }
    }
    #[test]
    fn defaults_are_1000_and_2() {
        let args = Args::try_parse_from(argv(&["--older-than", "1mo"])).unwrap();
        assert_eq!(args.options().batch_size, None);
        assert_eq!(args.options().workers, None);
    }
    #[test]
    fn workers_outside_1_to_8_are_rejected() {
        for value in ["0", "9", "255"] {
            assert!(
                Args::try_parse_from(argv(&["--older-than", "30d", "--workers", value])).is_err()
            );
        }
    }
    #[test]
    fn batch_size_outside_1_to_10000_is_rejected() {
        for value in ["0", "10001"] {
            assert!(
                Args::try_parse_from(argv(&["--older-than", "30d", "--batch-size", value]))
                    .is_err()
            );
        }
    }
}
#[cfg(test)]
mod root_regression {
    use super::*;
    #[test]
    fn root_settings_are_not_silently_ignored_by_subcommands() {
        assert!(Args::try_parse_from(["hmail-cleaner", "--older-than", "365d", "clean"]).is_err());
        assert!(Args::try_parse_from(["hmail-cleaner", "--silence", "preview"]).is_err());
        assert!(Args::try_parse_from(["hmail-cleaner", "--older-than", "365d"]).is_ok());
    }
}
