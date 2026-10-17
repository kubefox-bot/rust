use super::host_key::HostKey;
use crate::{
    auth::SessionLease,
    enrollment::{target, EnrollmentError, HostKeyChallenge, SshCredentials, SshTarget},
};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
pub(super) const CONTROL_TIMEOUT: Duration = Duration::from_secs(30);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const CLOSE_TIMEOUT: Duration = Duration::from_secs(5);
const WINDOW_BYTES: u32 = 64 * 1024;
const PACKET_BYTES: u32 = 32 * 1024;
const CHANNEL_BUFFER: usize = 4;
#[derive(Clone)]
pub struct NativeSsh;
pub struct VerifiedSsh {
    pub(super) session: russh::client::Handle<HostKey>,
    pub(super) lease: SessionLease,
}
impl NativeSsh {
    pub async fn probe(
        target: SshTarget,
        lease: &SessionLease,
    ) -> Result<HostKeyChallenge, EnrollmentError> {
        let (session, observed) = connect(&target, None, lease).await?;
        let fingerprint = observed
            .lock()
            .map_err(|_| EnrollmentError::HostKey)?
            .clone()
            .ok_or(EnrollmentError::HostKey)?;
        let _ = close_session(&session).await;
        Ok(HostKeyChallenge {
            target,
            fingerprint,
        })
    }
    pub async fn connect(
        target: SshTarget,
        fingerprint: String,
        credentials: SshCredentials,
        lease: &SessionLease,
    ) -> Result<VerifiedSsh, EnrollmentError> {
        target::credentials(&credentials)?;
        if fingerprint.len() != 50 || !fingerprint.starts_with("SHA256:") {
            return Err(EnrollmentError::HostKey);
        }
        let (mut session, _) = connect(&target, Some(fingerprint), lease).await?;
        let authentication = async {
            match credentials {
                SshCredentials::Password { username, password } => session
                    .authenticate_password(username, password.as_str())
                    .await
                    .map_err(|_| EnrollmentError::Authentication),
                SshCredentials::Key {
                    username,
                    private_key,
                    passphrase,
                } => {
                    let key = super::key::load(private_key, passphrase).await?;
                    session
                        .authenticate_publickey(
                            username,
                            russh::keys::PrivateKeyWithHashAlg::new(Arc::new(key), None),
                        )
                        .await
                        .map_err(|_| EnrollmentError::Authentication)
                }
            }
        };
        let mut revoked = lease.revocation();
        let result = tokio::select! {biased;_=revoked.changed()=>Err(EnrollmentError::Unauthorized),result=tokio::time::timeout(CONTROL_TIMEOUT,authentication)=>result.map_err(|_|EnrollmentError::Transport).and_then(|v|v)};
        match result {
            Ok(auth) if auth.success() => Ok(VerifiedSsh {
                session,
                lease: lease.clone(),
            }),
            result => {
                let _ = close_session(&session).await;
                Err(result.err().unwrap_or(EnrollmentError::Authentication))
            }
        }
    }
}
type ObservedKey = Arc<Mutex<Option<String>>>;
async fn connect(
    target: &SshTarget,
    expected: Option<String>,
    lease: &SessionLease,
) -> Result<(russh::client::Handle<HostKey>, ObservedKey), EnrollmentError> {
    target::validate(target)?;
    if !lease.is_live() {
        return Err(EnrollmentError::Unauthorized);
    }
    let observed = Arc::new(Mutex::new(None));
    let handler = HostKey {
        expected,
        observed: observed.clone(),
    };
    let config = russh::client::Config {
        window_size: WINDOW_BYTES,
        maximum_packet_size: PACKET_BYTES,
        channel_buffer_size: CHANNEL_BUFFER,
        inactivity_timeout: Some(CONTROL_TIMEOUT),
        keepalive_interval: Some(Duration::from_secs(10)),
        keepalive_max: 3,
        ..Default::default()
    };
    let mut revoked = lease.revocation();
    let result = tokio::select! {biased;_=revoked.changed()=>return Err(EnrollmentError::Unauthorized),result=tokio::time::timeout(CONNECT_TIMEOUT,russh::client::connect(Arc::new(config),(target.host.as_str(),target.port),handler))=>result.map_err(|_|EnrollmentError::Transport)?};
    result
        .map(|session| (session, observed.clone()))
        .map_err(|_| {
            if observed.lock().is_ok_and(|value| value.is_some()) {
                EnrollmentError::HostKey
            } else {
                EnrollmentError::Transport
            }
        })
}
impl VerifiedSsh {
    pub async fn close(self) -> Result<(), EnrollmentError> {
        close_session(&self.session).await
    }
}
async fn close_session(session: &russh::client::Handle<HostKey>) -> Result<(), EnrollmentError> {
    tokio::time::timeout(
        CLOSE_TIMEOUT,
        session.disconnect(russh::Disconnect::ByApplication, "operation ended", ""),
    )
    .await
    .map_err(|_| EnrollmentError::Transport)?
    .map_err(|_| EnrollmentError::Transport)
}
