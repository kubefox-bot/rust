use super::VerifiedSsh;
use crate::enrollment::{package::TRANSFER_BYTES, AgentPackage, EnrollmentError};
use russh_sftp::{
    client::{Config, SftpSession},
    protocol::OpenFlags,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
const MAX_SFTP_PACKET: u32 = 64 * 1024;
const MAX_PENDING_REQUESTS: usize = 2;
impl VerifiedSsh {
    pub(super) async fn transfer(
        &self,
        path: &str,
        package: &mut AgentPackage,
    ) -> Result<(), EnrollmentError> {
        if path.len() > 4096
            || path.contains('\0')
            || path.contains("..")
            || !path.contains("/hMailCleanerAgentStage/")
            || !path.ends_with(".exe")
        {
            return Err(EnrollmentError::Invalid);
        }
        self.guarded(super::command::INSTALL_TIMEOUT, async {
            let channel = self
                .session
                .channel_open_session()
                .await
                .map_err(|_| EnrollmentError::Transport)?;
            channel
                .request_subsystem(true, "sftp")
                .await
                .map_err(|_| EnrollmentError::Transport)?;
            let sftp = SftpSession::new_with_config(
                channel.into_stream(),
                Config {
                    max_packet_len: MAX_SFTP_PACKET,
                    max_concurrent_reads: MAX_PENDING_REQUESTS,
                    max_concurrent_writes: MAX_PENDING_REQUESTS,
                    max_write_packet_len: TRANSFER_BYTES as u32,
                    request_timeout_secs: 15,
                },
            )
            .await
            .map_err(|_| EnrollmentError::Transport)?;
            let result = upload(&sftp, path, package).await;
            let closed = sftp.close().await.map_err(|_| EnrollmentError::Transport);
            result.and(closed)
        })
        .await
    }
}
async fn upload(
    sftp: &SftpSession,
    path: &str,
    package: &mut AgentPackage,
) -> Result<(), EnrollmentError> {
    let mut file = sftp
        .open_with_flags(
            path,
            OpenFlags::CREATE | OpenFlags::EXCLUDE | OpenFlags::WRITE,
        )
        .await
        .map_err(|_| EnrollmentError::Remote)?;
    let mut buffer = [0u8; TRANSFER_BYTES];
    let mut sent = 0;
    loop {
        let count = package
            .file
            .read(&mut buffer)
            .await
            .map_err(|_| EnrollmentError::Package)?;
        if count == 0 {
            break;
        }
        sent += count as u64;
        if sent > package.size {
            return Err(EnrollmentError::Package);
        }
        file.write_all(&buffer[..count])
            .await
            .map_err(|_| EnrollmentError::Transport)?;
    }
    if sent != package.size {
        return Err(EnrollmentError::Package);
    }
    file.flush().await.map_err(|_| EnrollmentError::Transport)?;
    file.shutdown()
        .await
        .map_err(|_| EnrollmentError::Transport)
}
