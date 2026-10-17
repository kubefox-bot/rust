use crate::fixtures::job_agent::Control;
use hmail_cleaner_protocol::{JobPhase, ServerId};
use serde::Deserialize;
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
    LoseResponse {
        server_id: ServerId,
        enabled: bool,
    },
    HideLookup {
        server_id: ServerId,
        enabled: bool,
    },
}
enum Action {
    Phase(JobPhase),
    LoseResponse(bool),
    HideLookup(bool),
}
pub fn execute(agents: &[(ServerId, Control)], line: &str) -> serde_json::Value {
    if line.len() > MAX_COMMAND_BYTES {
        return serde_json::json!({"error":"oversized command"});
    }
    let command = match serde_json::from_str::<Command>(line) {
        Ok(value) => value,
        Err(_) => return serde_json::json!({"error":"invalid command"}),
    };
    let (id, action) = match command {
        Command::Stats => {
            return serde_json::json!({"stats":agents.iter().map(|(id,control)|serde_json::json!({"id":id,"puts":control.puts(),"cancels":control.cancels(),"subscriptions":control.subscriptions()})).collect::<Vec<_>>() })
        }
        Command::FinishLatest { server_id } => (server_id, Action::Phase(JobPhase::Completed)),
        Command::Phase { server_id, phase } => (server_id, Action::Phase(phase)),
        Command::LoseResponse { server_id, enabled } => (server_id, Action::LoseResponse(enabled)),
        Command::HideLookup { server_id, enabled } => (server_id, Action::HideLookup(enabled)),
    };
    let Some((_, control)) = agents.iter().find(|(server, _)| *server == id) else {
        return serde_json::json!({"error":"unknown server"});
    };
    match action {
        Action::Phase(phase) => {
            let Some(job) = control.latest() else {
                return serde_json::json!({"error":"no job"});
            };
            control.progress(job.job_id, phase);
        }
        Action::LoseResponse(enabled) => control.lose_response(enabled),
        Action::HideLookup(enabled) => control.hide_lookup(enabled),
    }
    serde_json::json!({"ok":true})
}
