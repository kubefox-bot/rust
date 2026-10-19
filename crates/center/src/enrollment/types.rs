#[derive(Debug, thiserror::Error)]
pub enum EnrollmentError {
    #[error("enrollment or server not found")]
    NotFound,
    #[error("invalid enrollment input")]
    Invalid,
    #[error("SSH host key was not confirmed or changed")]
    HostKey,
    #[error("SSH authentication failed")]
    Authentication,
    #[error("remote connection unavailable")]
    Transport,
    #[error("remote operation did not return a verified result")]
    Remote,
    #[error("session revoked")]
    Unauthorized,
    #[error("another operation is reserved")]
    Busy,
    #[error("enrollment conflicts with an existing installation")]
    Conflict,
    #[error("enrollment storage failed")]
    Storage,
    #[error("trusted agent package unavailable or invalid")]
    Package,
    #[error("remote platform is not supported")]
    Platform,
}
impl From<crate::profiles::ProfileError> for EnrollmentError {
    fn from(error: crate::profiles::ProfileError) -> Self {
        use crate::profiles::ProfileError;
        match error {
            ProfileError::Unauthorized => Self::Unauthorized,
            ProfileError::NotFound => Self::NotFound,
            ProfileError::Storage => Self::Storage,
            ProfileError::Invalid => Self::Invalid,
            ProfileError::Conflict => Self::Conflict,
        }
    }
}
