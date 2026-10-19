use super::{PeerIdentity, RevokedSerials, TlsError};
use rustls::pki_types::CertificateDer;
use x509_parser::{extensions::GeneralName, parse_x509_certificate};

pub(super) enum Usage {
    Client,
    Server,
}

pub fn certificate_serial(certificate: &CertificateDer<'_>) -> Result<Vec<u8>, TlsError> {
    let (remaining, parsed) =
        parse_x509_certificate(certificate.as_ref()).map_err(|_| TlsError::InvalidCertificate)?;
    if !remaining.is_empty() {
        return Err(TlsError::InvalidCertificate);
    }
    Ok(parsed.raw_serial().to_vec())
}

pub(super) fn verify(
    certificate: &CertificateDer<'_>,
    expected: PeerIdentity,
    usage: Usage,
    revoked: &RevokedSerials,
) -> Result<(), rustls::Error> {
    let invalid = || {
        rustls::Error::InvalidCertificate(rustls::CertificateError::ApplicationVerificationFailure)
    };
    let (remaining, parsed) =
        parse_x509_certificate(certificate.as_ref()).map_err(|_| invalid())?;
    if !remaining.is_empty() || expected.id().is_nil() || revoked.contains(parsed.raw_serial()) {
        return Err(invalid());
    }
    let alternative = parsed
        .subject_alternative_name()
        .map_err(|_| invalid())?
        .ok_or_else(invalid)?;
    let mut names = alternative
        .value
        .general_names
        .iter()
        .filter_map(|name| match name {
            GeneralName::URI(uri) if uri.starts_with("urn:hmail-cleaner:") => Some(*uri),
            _ => None,
        });
    if names.next() != Some(expected.uri().as_str()) || names.next().is_some() {
        return Err(invalid());
    }
    let eku = parsed
        .extended_key_usage()
        .map_err(|_| invalid())?
        .ok_or_else(invalid)?;
    let allowed = match usage {
        Usage::Client => eku.value.client_auth,
        Usage::Server => eku.value.server_auth,
    };
    if !allowed {
        return Err(invalid());
    }
    Ok(())
}
