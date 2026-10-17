use super::{EnrollmentError, SshCredentials, SshTarget};
pub(super) fn validate(target: &SshTarget) -> Result<(), EnrollmentError> {
    if target.port == 0
        || target.host.len() > 253
        || target.host.is_empty()
        || target.host.trim() != target.host
    {
        return Err(EnrollmentError::Invalid);
    }
    if target.host.parse::<std::net::IpAddr>().is_ok() {
        return Ok(());
    }
    let url = url::Url::parse(&format!("https://{}", target.host))
        .map_err(|_| EnrollmentError::Invalid)?;
    if url.host_str() != Some(target.host.as_str())
        || url.port().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
        || !target
            .host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-'))
    {
        return Err(EnrollmentError::Invalid);
    }
    Ok(())
}
pub(super) fn credentials(input: &SshCredentials) -> Result<(), EnrollmentError> {
    let (user, valid) = match input {
        SshCredentials::Password { username, password } => {
            (username, !password.is_empty() && password.len() <= 16384)
        }
        SshCredentials::Key {
            username,
            private_key,
            passphrase,
        } => (
            username,
            !private_key.is_empty()
                && private_key.len() <= 65536
                && passphrase.as_ref().is_none_or(|p| p.len() <= 4096),
        ),
    };
    if !valid || user.is_empty() || user.len() > 256 || user.chars().any(char::is_control) {
        return Err(EnrollmentError::Invalid);
    }
    Ok(())
}
