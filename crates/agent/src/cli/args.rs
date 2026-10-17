use clap::{Parser, Subcommand};
use hmail_cleaner_protocol::AgentId;
use std::path::PathBuf;
#[derive(Parser)]
#[command(name = "hmail-cleaner agent", about = "Manual native mail agent")]
pub(super) struct Args {
    #[command(subcommand)]
    pub command: Command,
}
#[derive(Subcommand)]
pub(super) enum Command {
    Enroll {
        #[arg(long)]
        data_dir: PathBuf,
        #[arg(long, value_parser = agent_id)]
        agent_id: AgentId,
    },
    Run {
        #[arg(long)]
        config: PathBuf,
    },
    Service {
        #[arg(long)]
        config: PathBuf,
    },
    InstallService {
        #[arg(long)]
        package: PathBuf,
        #[arg(long)]
        executable: PathBuf,
        #[arg(long)]
        config: PathBuf,
        #[arg(long, value_parser = digest)]
        sha256: [u8; 32],
        #[arg(long, requires = "legacy_env")]
        legacy_executable: Option<PathBuf>,
        #[arg(long, requires = "legacy_executable")]
        legacy_env: Option<PathBuf>,
    },
}
fn agent_id(input: &str) -> Result<AgentId, String> {
    serde_json::from_value(serde_json::Value::String(input.into()))
        .map_err(|_| "a canonical nonzero agent UUID is required".into())
}
fn digest(input: &str) -> Result<[u8; 32], String> {
    if input.len() != 64 || !input.is_ascii() {
        return Err("a SHA-256 hex digest is required".into());
    }
    let mut bytes = [0; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&input[index * 2..index * 2 + 2], 16)
            .map_err(|_| "a SHA-256 hex digest is required")?;
    }
    Ok(bytes)
}
