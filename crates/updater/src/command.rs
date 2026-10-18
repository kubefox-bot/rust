use crate::Error;
use std::{ffi::OsString, path::Path, time::Duration};
use tokio::{io::AsyncReadExt, process::Command};
const OUTPUT_LIMIT: u64 = 1024 * 1024;
/// Bounded output and lifetime. Never include argv or process stderr in public errors.
pub async fn run(program: &Path, args: &[OsString], timeout: Duration) -> Result<Vec<u8>, Error> {
    let mut command = Command::new(program);
    command
        .args(args)
        .kill_on_drop(true)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null());
    let result = tokio::time::timeout(timeout, async {
        let mut child = command.spawn().map_err(|_| Error::Command)?;
        let stdout = child.stdout.take().ok_or(Error::Command)?;
        let mut bytes = Vec::new();
        stdout
            .take(OUTPUT_LIMIT + 1)
            .read_to_end(&mut bytes)
            .await
            .map_err(|_| Error::Command)?;
        if bytes.len() as u64 > OUTPUT_LIMIT {
            let _ = child.kill().await;
            return Err(Error::Command);
        }
        if !child.wait().await.map_err(|_| Error::Command)?.success() {
            return Err(Error::Command);
        }
        Ok(bytes)
    })
    .await;
    result.map_err(|_| Error::Command)?
}
