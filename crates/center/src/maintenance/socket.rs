use super::{MaintenanceError, Snapshot};
use crate::storage::CenterStore;
use serde::Deserialize;
use std::{
    os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{UnixListener, UnixStream},
    task::JoinSet,
};
use uuid::Uuid;
const MAX_REQUEST_BYTES: u64 = 1024;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(3);
const MAX_CONNECTIONS: usize = 8;
pub(crate) const DIRECTORY: &str = "/run/astra-maintenance";
const SOCKET: &str = "control.sock";
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    version: u8,
    operation: Operation,
    id: Option<Uuid>,
}
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Operation {
    Begin,
    Status,
    Finish,
    Abort,
    Policy,
}
pub(crate) struct ControlServer {
    task: tokio::task::AbortHandle,
    path: PathBuf,
}
impl Drop for ControlServer {
    fn drop(&mut self) {
        self.task.abort();
        let _ = std::fs::remove_file(&self.path);
    }
}
impl ControlServer {
    pub(crate) async fn start(
        directory: &Path,
        store: CenterStore,
    ) -> Result<Option<Self>, MaintenanceError> {
        let meta = match directory.symlink_metadata() {
            Ok(meta) => meta,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(MaintenanceError::Unavailable),
        };
        // SAFETY: geteuid has no arguments and cannot fail or transfer ownership.
        let uid = unsafe { libc::geteuid() };
        if !meta.is_dir() || meta.uid() != uid || meta.permissions().mode() & 0o077 != 0 {
            return Err(MaintenanceError::Unavailable);
        }
        let path = directory.join(SOCKET);
        if let Ok(meta) = path.symlink_metadata() {
            if !meta.file_type().is_socket() || meta.uid() != uid {
                return Err(MaintenanceError::Unavailable);
            }
            match UnixStream::connect(&path).await {
                Ok(_) => return Err(MaintenanceError::Conflict),
                Err(error) if error.kind() == std::io::ErrorKind::ConnectionRefused => {
                    std::fs::remove_file(&path).map_err(|_| MaintenanceError::Unavailable)?
                }
                Err(_) => return Err(MaintenanceError::Unavailable),
            }
        }
        let listener = UnixListener::bind(&path).map_err(|_| MaintenanceError::Unavailable)?;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
            .map_err(|_| MaintenanceError::Unavailable)?;
        let task = tokio::spawn(async move {
            let mut clients = JoinSet::new();
            loop {
                tokio::select! {
                    Some(_) = clients.join_next(), if !clients.is_empty() => (),
                    incoming = listener.accept(), if clients.len() < MAX_CONNECTIONS => {
                        let Ok((stream, _)) = incoming else { break; };
                        let store = store.clone();
                        clients.spawn(async move { let _ = tokio::time::timeout(REQUEST_TIMEOUT, handle(stream, store, uid)).await; });
                    }
                }
            }
        });
        Ok(Some(Self {
            task: task.abort_handle(),
            path,
        }))
    }
}
async fn handle(
    mut stream: UnixStream,
    store: CenterStore,
    uid: u32,
) -> Result<(), MaintenanceError> {
    if stream
        .peer_cred()
        .map_err(|_| MaintenanceError::Unavailable)?
        .uid()
        != uid
    {
        stream
            .write_all(b"{\"version\":1,\"error\":\"unauthorized\"}\n")
            .await
            .map_err(|_| MaintenanceError::Unavailable)?;
        return Ok(());
    }
    let (reader, mut writer) = stream.split();
    use tokio::io::AsyncReadExt;
    let mut line = Vec::new();
    let count = BufReader::new(reader.take(MAX_REQUEST_BYTES + 1))
        .read_until(b'\n', &mut line)
        .await
        .map_err(|_| MaintenanceError::Invalid)?;
    let result = if count == 0 || count as u64 > MAX_REQUEST_BYTES || line.last() != Some(&b'\n') {
        Err(MaintenanceError::Invalid)
    } else {
        match serde_json::from_slice::<Request>(&line) {
            Ok(request) if request.version == 1 => dispatch(request, &store).await,
            _ => Err(MaintenanceError::Invalid),
        }
    };
    let mut output = match result {
        Ok(status) => serde_json::to_vec(&status).map_err(|_|MaintenanceError::Unavailable)?,
        Err(error) => serde_json::to_vec(&serde_json::json!({"version":1,"error":match error { MaintenanceError::Conflict=>"conflict", MaintenanceError::Unavailable=>"unavailable",MaintenanceError::Invalid=>"invalid" }})).map_err(|_|MaintenanceError::Unavailable)?,
    };
    output.push(b'\n');
    writer
        .write_all(&output)
        .await
        .map_err(|_| MaintenanceError::Unavailable)
}
async fn dispatch(request: Request, store: &CenterStore) -> Result<Snapshot, MaintenanceError> {
    match request.operation {
        Operation::Begin => store
            .maintenance()
            .begin(request.id.ok_or(MaintenanceError::Invalid)?)?,
        Operation::Status => store
            .maintenance()
            .verify(request.id.ok_or(MaintenanceError::Invalid)?)?,
        Operation::Finish | Operation::Abort => store
            .maintenance()
            .end(request.id.ok_or(MaintenanceError::Invalid)?)?,
        Operation::Policy => (),
    }
    store.maintenance().status(store).await
}

#[cfg(test)]
mod tests {
    use super::*;
    async fn request(path: &Path, value: &[u8]) -> serde_json::Value {
        let mut stream = UnixStream::connect(path).await.unwrap();
        stream.write_all(value).await.unwrap();
        let mut line = String::new();
        BufReader::new(stream).read_line(&mut line).await.unwrap();
        serde_json::from_str(&line).unwrap()
    }
    #[tokio::test]
    async fn local_protocol_is_bounded_versioned_and_operation_owned() {
        let root = tempfile::tempdir().unwrap();
        let directory = crate::storage::DataDirectory::acquire(&root.path().join("data")).unwrap();
        let store = CenterStore::open(&directory).await.unwrap();
        let control = root.path().join("control");
        std::fs::create_dir(&control).unwrap();
        std::fs::set_permissions(&control, std::fs::Permissions::from_mode(0o700)).unwrap();
        let _server = ControlServer::start(&control, store.clone())
            .await
            .unwrap()
            .unwrap();
        let path = control.join(SOCKET);
        let id = Uuid::new_v4();
        assert_eq!(
            request(&path, b"{\"version\":2,\"operation\":\"policy\"}\n").await["error"],
            "invalid"
        );
        assert_eq!(
            request(&path, b"{\"version\":1,\"operation\":\"policy\"}\n").await["policy"]
                ["check_interval_seconds"],
            60
        );
        let begin = format!("{{\"version\":1,\"operation\":\"begin\",\"id\":\"{id}\"}}\n");
        assert_eq!(request(&path, begin.as_bytes()).await["status"], "ready");
        assert!(store.maintenance().admit().is_err());
        let other = format!(
            "{{\"version\":1,\"operation\":\"abort\",\"id\":\"{}\"}}\n",
            Uuid::new_v4()
        );
        assert_eq!(request(&path, other.as_bytes()).await["error"], "conflict");
        let other_status = other.replace("abort", "status");
        assert_eq!(
            request(&path, other_status.as_bytes()).await["error"],
            "conflict"
        );
        let end = format!("{{\"version\":1,\"operation\":\"finish\",\"id\":\"{id}\"}}\n");
        assert_eq!(request(&path, end.as_bytes()).await["status"], "idle");
        let oversized = vec![b'a'; MAX_REQUEST_BYTES as usize + 1];
        assert_eq!(request(&path, &oversized).await["error"], "invalid");
    }
    #[tokio::test]
    async fn untrusted_uid_receives_only_safe_unauthorized_response() {
        let root = tempfile::tempdir().unwrap();
        let directory = crate::storage::DataDirectory::acquire(root.path()).unwrap();
        let store = CenterStore::open(&directory).await.unwrap();
        let (client, server) = UnixStream::pair().unwrap();
        let expected_uid = server.peer_cred().unwrap().uid().wrapping_add(1);
        let task = tokio::spawn(handle(server, store, expected_uid));
        let mut line = String::new();
        BufReader::new(client).read_line(&mut line).await.unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&line).unwrap(),
            serde_json::json!({"version":1,"error":"unauthorized"})
        );
        task.await.unwrap().unwrap();
    }
}
