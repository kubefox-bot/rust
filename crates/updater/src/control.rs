use crate::{journal, Error};
use hmail_cleaner_protocol::browser::updates::UpdatePolicy;
use serde::Deserialize;
use std::{path::Path, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::UnixStream,
};
use uuid::Uuid;
const CONTROL_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_RESPONSE: u64 = 8192;
#[derive(Clone, Copy, Deserialize, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Idle,
    Draining,
    Ready,
}
#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub version: u8,
    pub vault_id: Option<Uuid>,
    pub status: State,
    pub active_jobs: u64,
    pub active_enrollments: u64,
    pub active_writers: u64,
    pub policy: UpdatePolicy,
}
pub async fn request(path: &Path, operation: &str, id: Option<Uuid>) -> Result<Snapshot, Error> {
    if !matches!(
        operation,
        "begin" | "status" | "finish" | "abort" | "policy"
    ) {
        return Err(Error::Invalid);
    }
    tokio::time::timeout(CONTROL_TIMEOUT, async {
        let mut socket = UnixStream::connect(path)
            .await
            .map_err(|_| Error::Transport)?;
        if socket.peer_cred().map_err(|_| Error::Transport)?.uid() != unsafe { libc::geteuid() } {
            return Err(Error::Verification);
        }
        let mut bytes =
            serde_json::to_vec(&serde_json::json!({"version":1,"operation":operation,"id":id}))
                .map_err(|_| Error::Invalid)?;
        bytes.push(b'\n');
        socket
            .write_all(&bytes)
            .await
            .map_err(|_| Error::Transport)?;
        let mut output = Vec::new();
        BufReader::new(socket.take(MAX_RESPONSE + 1))
            .read_until(b'\n', &mut output)
            .await
            .map_err(|_| Error::Transport)?;
        if output.len() as u64 > MAX_RESPONSE || output.last() != Some(&b'\n') {
            return Err(Error::Transport);
        }
        let status: Snapshot = serde_json::from_slice(&output).map_err(|_| Error::Transport)?;
        if status.version != 1 {
            return Err(Error::Invalid);
        }
        status.policy.validate().map_err(|_| Error::Invalid)?;
        Ok(status)
    })
    .await
    .map_err(|_| Error::Transport)?
}
/// A missing service preserves the last acknowledged policy; it never resets it.
pub async fn refresh_policy(socket: &Path, cache: &Path) -> Result<UpdatePolicy, Error> {
    match request(socket, "policy", None).await {
        Ok(snapshot) => {
            journal::save(cache, &snapshot.policy)?;
            Ok(snapshot.policy)
        }
        Err(error) => {
            if let Some(policy) = journal::load::<UpdatePolicy>(cache)? {
                policy.validate().map_err(|_| Error::Invalid)?;
                Ok(policy)
            } else {
                Err(error)
            }
        }
    }
}
