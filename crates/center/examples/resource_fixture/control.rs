use crate::fixtures::{agents::TlsAgent, job_agent::Control};
use hmail_cleaner_protocol::{JobPhase, ObservedMailState, ServerId};
use serde::Deserialize;
use std::sync::atomic::Ordering;
const MAX_COMMAND_BYTES: usize = 4096;
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Command {
    Stats,
    FinishLatest {
        server_id: ServerId,
    },
    Phase {
        server_id: ServerId,
        phase: JobPhase,
    },
    MailStopped {
        server_id: ServerId,
    },
}
pub fn execute(agents: &[(ServerId, TlsAgent, Control)], line: &str) -> serde_json::Value {
    if line.len() > MAX_COMMAND_BYTES {
        return serde_json::json!({"error":"oversized command"});
    }
    let command = match serde_json::from_str::<Command>(line) {
        Ok(command) => command,
        Err(_) => return serde_json::json!({"error":"invalid command"}),
    };
    let (id, phase) = match command {
        Command::Stats => {
            return serde_json::json!({"stats":agents.iter().map(|(id,agent,control)|serde_json::json!({"id":id,"puts":control.puts(),"cancels":control.cancels(),"subscriptions":control.subscriptions(),"health_calls":agent.health_calls.load(Ordering::SeqCst)})).collect::<Vec<_>>() })
        }
        Command::FinishLatest { server_id } => (server_id, Some(JobPhase::Completed)),
        Command::Phase { server_id, phase } => (server_id, Some(phase)),
        Command::MailStopped { server_id } => (server_id, None),
    };
    let Some((_, _, control)) = agents.iter().find(|(server, _, _)| *server == id) else {
        return serde_json::json!({"error":"unknown server"});
    };
    match phase {
        Some(phase) => {
            let Some(job) = control.latest() else {
                return serde_json::json!({"error":"no job"});
            };
            control.progress(job.job_id, phase);
        }
        None => control.mail(ObservedMailState::Stopped),
    }
    serde_json::json!({"ok":true})
}
