use hmail_cleaner_adapters::mtls::ServerTls;
use hmail_cleaner_protocol::AgentId;
use std::{net::SocketAddr, path::PathBuf};

pub struct AgentConfig {
    pub listen: SocketAddr,
    pub data_directory: PathBuf,
    pub agent_id: AgentId,
    pub tls: ServerTls,
}
