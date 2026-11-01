//! owner: WP-A.5
//!
//! Device side of config bundle delivery (CON-5, CON-6, 02 ADR-B6), without I/O:
//! - [`build_config_bundle_request`]: `GET /v1/config/bundle` with the device bearer and,
//!   when the device holds a verified bundle, `If-None-Match` for it (the backend's ETag is
//!   `"<bundle_version>"`, `config_docs::bundle::etag`);
//! - [`classify_config_bundle_response`]: what the answer means (fresh body, 304, none yet,
//!   re-register, retry, fatal);
//! - [`accept_bundle`]: the trust decision. Signature through
//!   `asd_contract::signing::verify_bundle_with_keys` (the only verifier, CON-6) against the
//!   app's key ring (key rotation by `key_id`), then the device policy: no rollback below the
//!   newest bundle this device accepted, and an optional maximum age of `signed_at`.
//!
//! A rejected bundle is never used; the app keeps its last good bundle or the bundled
//! default.

use std::cmp::Ordering;

use asd_contract::config::ConfigBundle;
use asd_contract::signing::{self, SigningError, VerifyingKey};

use crate::request::{ClientInfo, SyncRequest};
use crate::response::retry_after_seconds;
use crate::SyncError;

/// Path of the device bundle endpoint, relative to the API base URL.
pub const CONFIG_BUNDLE_PATH: &str = "v1/config/bundle";
/// Request timeout of a bundle fetch.
pub const CONFIG_BUNDLE_TIMEOUT_SECS: u32 = 20;
/// Tolerated device clock lag when a maximum age is enforced: a bundle signed up to this
/// far in the future is accepted, anything later is refused.
pub const MAX_CLOCK_SKEW_SECS: i64 = 24 * 3600;

/// One trusted bundle verification key, as shipped in the app.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustedKey {
    /// The `key_id` the backend writes into bundles signed with this key
    /// (`asd_contract::signing::key_id` unless set explicitly).
    pub key_id: String,
    /// The 32-byte Ed25519 public key, base64url or standard base64.
    pub public_key_b64: String,
}

/// The device's acceptance policy for one bundle.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BundlePolicy {
    /// The newest `bundle_version` this device has accepted; an older one is a rollback.
    /// The same version is accepted (re-verifying the cached bundle at launch).
    pub newest_accepted_version: Option<String>,
    /// Now, Unix seconds (the caller's clock).
    pub now_unix_secs: i64,
    /// Refuse a bundle whose `signed_at` is older than this. `None`: no expiry.
    pub max_age_secs: Option<u64>,
}

/// Why a bundle was refused. The variant is the reason the app logs and tests on.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BundleRejection {
    /// The app ships no usable key: remote bundles cannot be trusted at all.
    #[error("no trusted bundle key is configured")]
    NoTrustedKeys,
    /// A shipped key is not a 32-byte Ed25519 public key (an app configuration bug).
    #[error("trusted key `{0}` is malformed")]
    MalformedTrustedKey(String),
    /// Not JSON, not an object, or signed content that is not a `ConfigBundle`.
    #[error("bundle is malformed: {0}")]
    Malformed(String),
    /// No signature, or not `ed25519:` + base64url of 64 bytes.
    #[error("bundle is not signed")]
    Unsigned,
    /// The bundle names a key this app does not trust (or names none).
    #[error("bundle key `{0}` is not trusted")]
    UnknownKey(String),
    /// The signature does not match the content (tampered, or the wrong key behind a
    /// trusted key id).
    #[error("bundle signature does not verify")]
    BadSignature,
    /// `bundle_version` is not `YYYY.MM.DD-N`.
    #[error("bundle version `{0}` is not YYYY.MM.DD-N")]
    InvalidVersion(String),
    /// Older than the newest bundle this device accepted.
    #[error("bundle {offered} is older than the accepted {newest}")]
    Rollback { offered: String, newest: String },
    /// `signed_at` is not an RFC 3339 date.
    #[error("bundle signed_at `{0}` is not an RFC 3339 date")]
    InvalidSignedAt(String),
    /// `signed_at` is older than the maximum age.
    #[error("bundle signed at {0} has expired")]
    Expired(String),
    /// `signed_at` lies further in the future than the tolerated clock skew.
    #[error("bundle signed at {0} lies in the future")]
    SignedInFuture(String),
}

/// Verifies `bundle_json` against the key ring and applies the device policy. On success
/// the decoded bundle; its `catalog`, `baselines` and `scoring_profile` are safe to use.
pub fn accept_bundle(
    bundle_json: &[u8],
    keys: &[TrustedKey],
    policy: &BundlePolicy,
) -> Result<ConfigBundle, BundleRejection> {
    if keys.is_empty() {
        return Err(BundleRejection::NoTrustedKeys);
    }
    let ring = keys
        .iter()
        .map(|k| {
            signing::parse_public_key(&k.public_key_b64)
                .map(|key| (k.key_id.as_str(), key))
                .map_err(|_| BundleRejection::MalformedTrustedKey(k.key_id.clone()))
        })
        .collect::<Result<Vec<(&str, VerifyingKey)>, _>>()?;
    let text = std::str::from_utf8(bundle_json)
        .map_err(|e| BundleRejection::Malformed(format!("not UTF-8: {e}")))?;
    let bundle = signing::verify_bundle_with_keys(text, &ring).map_err(|e| match e {
        SigningError::MissingSignature | SigningError::MalformedSignature => {
            BundleRejection::Unsigned
        }
        SigningError::MissingKeyId => BundleRejection::UnknownKey(String::new()),
        SigningError::UnknownKeyId(id) => BundleRejection::UnknownKey(id),
        SigningError::BadSignature => BundleRejection::BadSignature,
        SigningError::InvalidKey(m) => BundleRejection::MalformedTrustedKey(m),
        e @ (SigningError::InvalidJson(_)
        | SigningError::NotAnObject
        | SigningError::InvalidBundle(_)
        | SigningError::Canonicalisation(_)) => BundleRejection::Malformed(e.to_string()),
    })?;

    if parse_bundle_version(&bundle.bundle_version).is_none() {
        return Err(BundleRejection::InvalidVersion(bundle.bundle_version));
    }
    if let Some(newest) = policy.newest_accepted_version.as_deref() {
        // An unparseable stored version cannot anchor anything: ignore it.
        if compare_bundle_versions(&bundle.bundle_version, newest) == Some(Ordering::Less) {
            return Err(BundleRejection::Rollback {
                offered: bundle.bundle_version,
                newest: newest.to_string(),
            });
        }
    }
    if let Some(max_age) = policy.max_age_secs {
        let signed = parse_rfc3339_unix(&bundle.signed_at)
            .ok_or_else(|| BundleRejection::InvalidSignedAt(bundle.signed_at.clone()))?;
        if signed > policy.now_unix_secs.saturating_add(MAX_CLOCK_SKEW_SECS) {
            return Err(BundleRejection::SignedInFuture(bundle.signed_at));
        }
        let age = policy.now_unix_secs.saturating_sub(signed);
        if age > i64::try_from(max_age).unwrap_or(i64::MAX) {
            return Err(BundleRejection::Expired(bundle.signed_at));
        }
    }
    Ok(bundle)
}

/// `YYYY.MM.DD-N` (the backend's bundle numbering) as `(year, month, day, n)`.
pub fn parse_bundle_version(version: &str) -> Option<(u32, u32, u32, u64)> {
    let (date, n) = version.split_once('-')?;
    let mut parts = date.split('.');
    let digits = |s: &str, len: usize| s.len() == len && s.bytes().all(|b| b.is_ascii_digit());
    let (y, m, d) = (parts.next()?, parts.next()?, parts.next()?);
    if parts.next().is_some() || !digits(y, 4) || !digits(m, 2) || !digits(d, 2) {
        return None;
    }
    if n.is_empty() || !n.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let (m, d): (u32, u32) = (m.parse().ok()?, d.parse().ok()?);
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    let n: u64 = n.parse().ok()?;
    (n >= 1).then_some((y.parse().ok()?, m, d, n))
}

/// Orders two bundle versions; `None` when either is not `YYYY.MM.DD-N`.
pub fn compare_bundle_versions(a: &str, b: &str) -> Option<Ordering> {
    Some(parse_bundle_version(a)?.cmp(&parse_bundle_version(b)?))
}

/// Unix seconds of an RFC 3339 date-time (`Z` or `±hh:mm`, optional fraction, which is
/// truncated). `None` for anything else.
pub fn parse_rfc3339_unix(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    if b.len() < 20 {
        return None;
    }
    let digits = |r: std::ops::Range<usize>| b[r].iter().all(u8::is_ascii_digit);
    let shape = digits(0..4)
        && b[4] == b'-'
        && digits(5..7)
        && b[7] == b'-'
        && digits(8..10)
        && (b[10] == b'T' || b[10] == b't')
        && digits(11..13)
        && b[13] == b':'
        && digits(14..16)
        && b[16] == b':'
        && digits(17..19);
    if !shape {
        return None;
    }
    let num = |r: std::ops::Range<usize>| -> i64 { s[r].parse().unwrap_or(-1) };
    let (year, month, day) = (num(0..4), num(5..7), num(8..10));
    let (hour, minute, second) = (num(11..13), num(14..16), num(17..19));
    if !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || hour > 23
        || minute > 59
        || second > 60
    {
        return None;
    }
    let mut i = 19;
    if b.get(i) == Some(&b'.') {
        i += 1;
        let start = i;
        while b.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
        if i == start {
            return None;
        }
    }
    let offset = match &s[i..] {
        "Z" | "z" => 0,
        tz if tz.len() == 6
            && (tz.starts_with('+') || tz.starts_with('-'))
            && tz.as_bytes()[3] == b':'
            && tz[1..3].bytes().all(|c| c.is_ascii_digit())
            && tz[4..6].bytes().all(|c| c.is_ascii_digit()) =>
        {
            let (h, m): (i64, i64) = (tz[1..3].parse().ok()?, tz[4..6].parse().ok()?);
            if h > 23 || m > 59 {
                return None;
            }
            let sign = if tz.starts_with('-') { -1 } else { 1 };
            sign * (h * 3600 + m * 60)
        }
        _ => return None,
    };
    let days = days_from_civil(year, month, day);
    Some(days * 86_400 + hour * 3600 + minute * 60 + second - offset)
}

/// Days since 1970-01-01 of a proleptic Gregorian date (H. Hinnant's algorithm).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Builds `GET /v1/config/bundle` with the device bearer. `held_version` is the version of
/// the verified bundle the device holds: it is sent as `If-None-Match` so an unchanged
/// bundle answers `304`. Pass `None` when the device holds none (or only the bundled
/// default), so the server always sends the body.
pub fn build_config_bundle_request(
    device_token: &str,
    held_version: Option<&str>,
    client: &ClientInfo,
) -> Result<SyncRequest, SyncError> {
    let token = device_token.trim();
    if token.is_empty() {
        return Err(SyncError::NotRegistered);
    }
    let mut headers = vec![
        ("Accept".to_string(), "application/json".to_string()),
        ("Authorization".to_string(), format!("Bearer {token}")),
        ("User-Agent".to_string(), client.user_agent()),
    ];
    if let Some(v) = held_version.filter(|v| parse_bundle_version(v).is_some()) {
        headers.push(("If-None-Match".to_string(), format!("\"{v}\"")));
    }
    Ok(SyncRequest {
        method: "GET".into(),
        path: CONFIG_BUNDLE_PATH.into(),
        headers,
        body: Vec::new(),
        timeout_secs: CONFIG_BUNDLE_TIMEOUT_SECS,
    })
}

/// What the answer to `GET /v1/config/bundle` means.
#[derive(Debug, Clone, PartialEq)]
pub enum ConfigBundleOutcome {
    /// `200`: a bundle body; still untrusted until [`accept_bundle`] says so.
    Fresh,
    /// `304`: the held bundle is still current.
    NotModified,
    /// `404`: the server has no bundle for this device yet; keep what is held.
    NoBundle,
    /// `401`/`403`: re-register once, then retry.
    Reauthenticate,
    /// `408`, `429`, `5xx`: try later (`Some(secs)` from a usable `Retry-After`).
    RetryAfter(Option<f64>),
    /// Anything else.
    Fatal { status: u16 },
}

/// Classifies the answer to `GET /v1/config/bundle`.
pub fn classify_config_bundle_response(
    status: u16,
    headers: &[(String, String)],
) -> ConfigBundleOutcome {
    match status {
        200 => ConfigBundleOutcome::Fresh,
        304 => ConfigBundleOutcome::NotModified,
        404 => ConfigBundleOutcome::NoBundle,
        401 | 403 => ConfigBundleOutcome::Reauthenticate,
        408 | 429 | 500..=599 => ConfigBundleOutcome::RetryAfter(retry_after_seconds(headers)),
        _ => ConfigBundleOutcome::Fatal { status },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundle_versions_order_by_day_then_number() {
        assert_eq!(parse_bundle_version("2026.11.01-3"), Some((2026, 11, 1, 3)));
        assert_eq!(
            compare_bundle_versions("2026.11.01-10", "2026.11.01-9"),
            Some(Ordering::Greater)
        );
        assert_eq!(
            compare_bundle_versions("2026.10.31-9", "2026.11.01-1"),
            Some(Ordering::Less)
        );
        for bad in [
            "",
            "2026.11.01",
            "2026.11.1-1",
            "2026.13.01-1",
            "2026.11.01-0",
            "2026.11.01-x",
            "v2026.11.01-1",
            "2026.11.01.1-1",
        ] {
            assert_eq!(parse_bundle_version(bad), None, "{bad}");
        }
    }

    #[test]
    fn rfc3339_dates() {
        assert_eq!(parse_rfc3339_unix("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(
            parse_rfc3339_unix("2026-10-30T00:00:00.000Z"),
            Some(1_793_318_400)
        );
        assert_eq!(
            parse_rfc3339_unix("2026-10-30T02:00:00+02:00"),
            Some(1_793_318_400)
        );
        assert_eq!(
            parse_rfc3339_unix("2000-02-29T12:00:00Z"),
            Some(951_825_600)
        );
        for bad in [
            "2026-10-30",
            "2026-10-30T00:00:00",
            "2026-10-30T00:00:00.Z",
            "2026-10-30T24:00:00Z",
            "2026-10-30T00:00:00+0200",
        ] {
            assert_eq!(parse_rfc3339_unix(bad), None, "{bad}");
        }
    }
}
