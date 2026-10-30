//! owner: WP-R.3
//!
//! URL rules of the device plane (F7): only `https` with a DNS host name; no IP literals
//! (v4, v6 or numeric shorthand), no loopback names, no embedded credentials. A bearer is
//! attached only for the API host (`SessionSyncClient.isAPIHost`).

use url::{Host, Url};

/// Why a URL was refused (`ResearchStudyConfiguration.ValidationError`).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum UrlError {
    #[error("not a URL: {0}")]
    Invalid(String),
    #[error("scheme `{0}` is not https")]
    InsecureScheme(String),
    #[error("credentials in URL")]
    CredentialsInUrl,
    #[error("URL has no host")]
    MissingHost,
    #[error("IP literal host `{0}`")]
    IpLiteralHost(String),
    #[error("loopback host `{0}`")]
    LoopbackHost(String),
}

/// Where to PUT a blob and whether the device bearer may go with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UploadTarget {
    /// Absolute URL. An absolute input is returned unchanged (pre-signed query strings are
    /// never re-encoded); a relative input is resolved against the API base.
    pub url: String,
    /// True only when the host is the API host.
    pub attach_bearer: bool,
}

/// True for numeric last labels (`127.1`, `2130706433`) and hex labels (`0x7f`): a real DNS
/// name never ends in a numeric label because top-level domains are alphabetic.
fn is_numeric_host(host: &str) -> bool {
    if host.contains(':') {
        return true;
    }
    let trimmed = host.strip_suffix('.').unwrap_or(host);
    let Some(last) = trimmed.rsplit('.').next() else {
        return false;
    };
    if !last.is_empty() && last.bytes().all(|b| b.is_ascii_digit()) {
        return true;
    }
    last.strip_prefix("0x")
        .is_some_and(|hex| hex.bytes().all(|b| b.is_ascii_hexdigit()))
}

fn check_host(url: &Url) -> Result<String, UrlError> {
    match url.host() {
        None => Err(UrlError::MissingHost),
        Some(Host::Ipv4(ip)) => Err(UrlError::IpLiteralHost(ip.to_string())),
        Some(Host::Ipv6(ip)) => Err(UrlError::IpLiteralHost(ip.to_string())),
        Some(Host::Domain(d)) => {
            let host = d.to_ascii_lowercase();
            if host.is_empty() {
                return Err(UrlError::MissingHost);
            }
            if is_numeric_host(&host) {
                return Err(UrlError::IpLiteralHost(host));
            }
            let bare = host.strip_suffix('.').unwrap_or(&host);
            if bare == "localhost" || bare.ends_with(".localhost") {
                return Err(UrlError::LoopbackHost(host));
            }
            Ok(host)
        }
    }
}

/// Parses an absolute URL and applies every rule; returns the parsed URL and its host.
fn checked_absolute(raw: &str) -> Result<(Url, String), UrlError> {
    if raw.is_empty() || raw.trim() != raw {
        return Err(UrlError::Invalid(raw.to_string()));
    }
    let url = Url::parse(raw).map_err(|e| UrlError::Invalid(format!("{raw}: {e}")))?;
    let scheme = url.scheme().to_string();
    if scheme != "https" {
        return Err(UrlError::InsecureScheme(scheme));
    }
    // WHATWG parsing accepts `https:host` and `https:///host` for special schemes; Apple's
    // `URL` (the reference) sees no host there. Require a non-empty authority.
    let after_scheme = &raw[scheme.len()..];
    let authority = after_scheme
        .strip_prefix("://")
        .ok_or(UrlError::MissingHost)?;
    if authority.is_empty() || authority.starts_with('/') {
        return Err(UrlError::MissingHost);
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(UrlError::CredentialsInUrl);
    }
    let host = check_host(&url)?;
    Ok((url, host))
}

/// `ResearchStudyConfiguration.validateIngestURL`: accepts only `https` URLs with a DNS host.
pub fn validate_ingest_url(raw: &str) -> Result<(), UrlError> {
    checked_absolute(raw).map(|_| ())
}

/// Joins `path` (e.g. [`crate::SESSIONS_PATH`]) onto the API base like
/// `URL.appendingPathComponent`: a base path prefix is kept (`https://h/base` +
/// `v1/sessions` = `https://h/base/v1/sessions`). The base must pass the ingest rules.
pub fn endpoint_url(api_base: &str, path: &str) -> Result<String, UrlError> {
    let (mut url, _) = checked_absolute(api_base)?;
    let prefix = url.path().trim_end_matches('/').to_string();
    url.set_path(&format!("{prefix}/{}", path.trim_start_matches('/')));
    Ok(url.to_string())
}

/// `SessionSyncClient.resolveUploadURL` + `isAPIHost`: absolute https URLs are used as
/// given, relative ones resolve against the API base; either way the ingest rules apply.
/// The bearer may be attached only when the resolved host is the API host.
pub fn validate_upload_url(raw: &str, api_base: &str) -> Result<UploadTarget, UrlError> {
    let (base, api_host) = checked_absolute(api_base)?;
    let (url, host) = match Url::parse(raw) {
        Ok(_) => {
            let (_, host) = checked_absolute(raw)?;
            (raw.to_string(), host)
        }
        Err(url::ParseError::RelativeUrlWithoutBase) => {
            if raw.trim() != raw {
                return Err(UrlError::Invalid(raw.to_string()));
            }
            let joined = base
                .join(raw)
                .map_err(|e| UrlError::Invalid(format!("{raw}: {e}")))?;
            let (_, host) = checked_absolute(joined.as_str())?;
            (joined.to_string(), host)
        }
        Err(e) => return Err(UrlError::Invalid(format!("{raw}: {e}"))),
    };
    Ok(UploadTarget {
        url,
        attach_bearer: host == api_host,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_hosts() {
        assert!(is_numeric_host("127.1"));
        assert!(is_numeric_host("10.0.0.0x7f"));
        assert!(is_numeric_host("::1"));
        assert!(!is_numeric_host("api.example.org"));
        assert!(!is_numeric_host("v1.api.example.org"));
        assert!(!is_numeric_host("0xcafe.example.org"));
    }

    #[test]
    fn authority_must_be_present() {
        assert_eq!(
            validate_ingest_url("https:ingest.example.org/x"),
            Err(UrlError::MissingHost)
        );
        assert_eq!(
            validate_ingest_url("https:///x"),
            Err(UrlError::MissingHost)
        );
    }

    #[test]
    fn trailing_dot_localhost_is_loopback() {
        assert!(matches!(
            validate_ingest_url("https://localhost./x"),
            Err(UrlError::LoopbackHost(_))
        ));
    }
}
