//! owner: WP-R.3
//!
//! What an HTTP answer means for the upload queue (`SessionSyncClient.postSession`,
//! `ResearchKitSyncService.processSession`, `retryAfterSeconds`).

use asd_contract::api_v1::RegisterDeviceResponse;
use serde::Deserialize;

/// Upper bound on an honoured `Retry-After`: a hostile or broken server must not park the
/// queue for days.
pub const MAX_RETRY_AFTER_SECS: f64 = 3600.0;

/// The useful part of a `2xx` session response. Every field is optional so a sparse or
/// problem+json body never fails an accepted upload.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct IngestAck {
    #[serde(default, rename = "sessionId")]
    pub session_id: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub duplicate: Option<bool>,
    #[serde(default)]
    pub frames_upload_url: Option<String>,
    #[serde(default)]
    pub report_upload_url: Option<String>,
}

/// Classification of a session post.
#[derive(Debug, Clone, PartialEq)]
pub enum SyncOutcome {
    /// `2xx`, including a replay answered `200` with `duplicate: true` (CON-17).
    Accepted(IngestAck),
    /// `409`: the key was used before. Counted as delivered: `sessionId` is globally
    /// unique, so this is a replay (known iOS limitation, CON-17).
    Duplicate,
    /// `401`/`403`: re-register once with the stored enrol code, then retry.
    Reauthenticate,
    /// `408`, `429`, `5xx`: try again later. `Some(secs)` when the server sent a usable
    /// `Retry-After` (capped at [`MAX_RETRY_AFTER_SECS`]); combine with
    /// [`crate::next_retry_delay`].
    RetryAfter(Option<f64>),
    /// Any other status: the payload will never be accepted; quarantine it.
    Fatal { status: u16 },
}

/// Seconds from a `Retry-After` header (name case-insensitive). Only the delta-seconds form
/// counts; an HTTP-date, a negative value or garbage yields `None` (the client's own
/// schedule applies).
pub fn retry_after_seconds(headers: &[(String, String)]) -> Option<f64> {
    let raw = headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("Retry-After"))?
        .1
        .trim();
    if raw.is_empty() || !raw.bytes().all(|b| b.is_ascii_digit() || b == b'.') {
        return None;
    }
    let seconds: f64 = raw.parse().ok()?;
    (seconds.is_finite() && seconds >= 0.0).then(|| seconds.min(MAX_RETRY_AFTER_SECS))
}

/// Classifies the answer to `POST /v1/sessions`.
pub fn classify_response(status: u16, headers: &[(String, String)], body: &[u8]) -> SyncOutcome {
    match status {
        200..=299 => SyncOutcome::Accepted(serde_json::from_slice(body).unwrap_or_default()),
        401 | 403 => SyncOutcome::Reauthenticate,
        409 => SyncOutcome::Duplicate,
        408 | 429 | 500..=599 => SyncOutcome::RetryAfter(retry_after_seconds(headers)),
        _ => SyncOutcome::Fatal { status },
    }
}

/// Classification of a registration answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegisterOutcome {
    /// Store the credential (token bound to the API host).
    Registered(RegisterDeviceResponse),
    /// The server refused the code (`4xx`/`5xx`); `410` = exhausted or expired (CON-11).
    Rejected { status: u16 },
    /// `2xx` without a usable token or org id.
    Invalid,
}

/// Classifies the answer to `POST /v1/devices/register`.
pub fn classify_register_response(status: u16, body: &[u8]) -> RegisterOutcome {
    if !(200..=299).contains(&status) {
        return RegisterOutcome::Rejected { status };
    }
    match serde_json::from_slice::<RegisterDeviceResponse>(body) {
        Ok(r) if !r.device_token.is_empty() && !r.org_id.is_empty() => {
            RegisterOutcome::Registered(r)
        }
        _ => RegisterOutcome::Invalid,
    }
}
