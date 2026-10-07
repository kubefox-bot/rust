mod retention;
use chrono::NaiveDateTime;
use clap::{ArgGroup, Args as ClapArgs, Parser, Subcommand};
use hmail_cleaner_core::{
    cleanup::{DEFAULT_BATCH_SIZE, DEFAULT_WORKERS, MAX_BATCH_SIZE, MAX_WORKERS},
    selection::Retention,
};
use std::path::PathBuf;
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub(crate) enum Scan {
    Fast,
    Full,
}

#[derive(Debug, Parser)]
#[command(
    name = "hmail-cleaner",
    version,
    about = "Delete old hMailServer messages: files first, then MySQL rows"
)]
pub(crate) struct Args {
    #[command(subcommand)]
    command: Command,
}
#[derive(Debug, Subcommand)]
enum Command {
    #[command(about = "Report database-selected old messages without deletion")]
    Preview(Options),
    #[command(about = "Delete old messages while hMailServer is STOPPED")]
    Clean(Options),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    Preview,
    Clean,
}
#[derive(Debug, ClapArgs)]
#[command(group(ArgGroup::new("retention").required(true).multiple(false)))]
pub(crate) struct Options {
    #[arg(long, value_enum, default_value = "fast")]
    pub scan: Scan,
    #[arg(long,group="retention",value_parser=retention::parse,help="Retention age: 30d, 1mo (calendar month), 1y")]
    pub older_than: Option<Retention>,
    #[arg(long,group="retention",value_parser=retention::before,help="Strict local cutoff: YYYY-MM-DD or YYYY-MM-DDTHH:MM:SS")]
    pub before: Option<NaiveDateTime>,
    #[arg(long)]
    pub data_dir: PathBuf,
    #[arg(long)]
    pub hmail_ini: PathBuf,
    #[arg(long, default_value = "hMailServer")]
    pub service_name: String,
    #[arg(long,default_value_t=DEFAULT_BATCH_SIZE,value_parser=clap::value_parser!(u32).range(1..=i64::from(MAX_BATCH_SIZE)))]
    pub batch_size: u32,
    #[arg(long,default_value_t=DEFAULT_WORKERS,value_parser=clap::value_parser!(u8).range(1..=i64::from(MAX_WORKERS)))]
    pub workers: u8,
    #[arg(
        long,
        help = "Read DATABASE_URL only from this explicitly selected env file (system env overrides it)"
    )]
    pub env_file: Option<PathBuf>,
    #[arg(long, help = "Emit NDJSON events and final result")]
    pub json: bool,
}
impl Args {
    pub fn options(&self) -> &Options {
        match &self.command {
            Command::Preview(options) | Command::Clean(options) => options,
        }
    }
    pub fn mode(&self) -> Mode {
        match self.command {
            Command::Preview(_) => Mode::Preview,
            Command::Clean(_) => Mode::Clean,
        }
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
        assert!(Args::try_parse_from(argv(&[])).is_err());
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
        assert_eq!(args.options().batch_size, 1000);
        assert_eq!(args.options().workers, 2);
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
