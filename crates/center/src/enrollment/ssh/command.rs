use super::VerifiedSsh;
use crate::enrollment::EnrollmentError;
use base64ct::{Base64, Encoding};
use serde::{de::DeserializeOwned, Serialize};
use std::{future::Future, time::Duration};
const MAX_INPUT_BYTES: usize = 64 * 1024;
const MAX_OUTPUT_BYTES: usize = 32 * 1024;
pub(super) const INSTALL_TIMEOUT: Duration = Duration::from_secs(180);
pub(super) enum Step {
    Preflight,
    Prepare,
    Enroll,
    Configure,
    Install,
    Cleanup,
    PrepareRecovery,
    RecoverIdentity,
}
impl Step {
    fn script(&self) -> &'static str {
        match self {
            Self::Preflight => include_str!("scripts/preflight.ps1"),
            Self::Prepare => include_str!("scripts/prepare.ps1"),
            Self::Enroll => include_str!("scripts/enroll.ps1"),
            Self::Configure => include_str!("scripts/configure.ps1"),
            Self::Install => include_str!("scripts/install.ps1"),
            Self::Cleanup => include_str!("scripts/cleanup.ps1"),
            Self::PrepareRecovery => include_str!("scripts/prepare-recovery.ps1"),
            Self::RecoverIdentity => include_str!("scripts/recover-identity.ps1"),
        }
    }
    fn timeout(&self) -> Duration {
        match self {
            Self::Install => INSTALL_TIMEOUT,
            _ => super::connect::CONTROL_TIMEOUT,
        }
    }
}
fn command(step: &Step) -> String {
    let script = [include_str!("scripts/common.ps1"), step.script()].join("\n");
    let bytes = script
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    format!(
        "powershell.exe -NoLogo -NoProfile -NonInteractive -EncodedCommand {}",
        Base64::encode_string(&bytes)
    )
}
impl VerifiedSsh {
    pub(super) async fn guarded<T>(
        &self,
        duration: Duration,
        operation: impl Future<Output = Result<T, EnrollmentError>>,
    ) -> Result<T, EnrollmentError> {
        if !self.lease.is_live() {
            return Err(EnrollmentError::Unauthorized);
        }
        let mut revoked = self.lease.revocation();
        tokio::select! {biased;_=revoked.changed()=>Err(EnrollmentError::Unauthorized),result=tokio::time::timeout(duration,operation)=>result.map_err(|_|EnrollmentError::Transport)?}
    }
    pub(super) async fn execute<T: DeserializeOwned>(
        &self,
        step: Step,
        input: &impl Serialize,
    ) -> Result<T, EnrollmentError> {
        let bytes = serde_json::to_vec(input).map_err(|_| EnrollmentError::Invalid)?;
        if bytes.len() > MAX_INPUT_BYTES {
            return Err(EnrollmentError::Invalid);
        }
        let command = command(&step);
        self.guarded(step.timeout(), async {
            let mut channel = self
                .session
                .channel_open_session()
                .await
                .map_err(|_| EnrollmentError::Transport)?;
            channel
                .exec(true, command)
                .await
                .map_err(|_| EnrollmentError::Transport)?;
            channel
                .data(bytes.as_slice())
                .await
                .map_err(|_| EnrollmentError::Transport)?;
            channel
                .eof()
                .await
                .map_err(|_| EnrollmentError::Transport)?;
            let mut stdout = Vec::new();
            let mut stderr_bytes = 0;
            let mut exit = None;
            while let Some(message) = channel.wait().await {
                match message {
                    russh::ChannelMsg::Data { data } => {
                        if data.len() > MAX_OUTPUT_BYTES.saturating_sub(stdout.len()) {
                            return Err(EnrollmentError::Remote);
                        }
                        stdout.extend_from_slice(&data);
                    }
                    russh::ChannelMsg::ExtendedData { data, .. } => {
                        stderr_bytes += data.len();
                        if stderr_bytes > MAX_OUTPUT_BYTES {
                            return Err(EnrollmentError::Remote);
                        }
                    }
                    russh::ChannelMsg::ExitStatus { exit_status } => exit = Some(exit_status),
                    russh::ChannelMsg::Failure | russh::ChannelMsg::ExitSignal { .. } => {
                        return Err(EnrollmentError::Remote)
                    }
                    _ => (),
                }
            }
            if exit != Some(0) {
                return Err(EnrollmentError::Remote);
            }
            serde_json::from_slice(&stdout).map_err(|_| EnrollmentError::Remote)
        })
        .await
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bootstrap_command_is_only_static_utf16_base64_and_takes_values_on_stdin() {
        let command = command(&Step::Install);
        let encoded = command.split_whitespace().last().unwrap();
        let bytes = Base64::decode_vec(encoded).unwrap();
        let text = String::from_utf16(
            &bytes
                .as_chunks::<2>()
                .0
                .iter()
                .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
                .collect::<Vec<_>>(),
        )
        .unwrap();
        assert!(text.contains("[Console]::In.ReadToEnd()"));
        assert!(text.contains("& $package @arguments"));
        assert!(!text.contains("Invoke-Expression"));
        assert!(!text.contains("Stop-Service"));
    }
}
