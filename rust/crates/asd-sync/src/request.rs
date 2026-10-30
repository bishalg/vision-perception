//! owner: WP-R.3
//!
//! Request building for the device plane (`SessionSyncClient.makeSessionRequest`,
//! `DeviceRegistrationService.register`).

use asd_contract::api_v1::RegisterDeviceRequest;
use asd_contract::session_v1_6::{
    SessionIngestRequest, IDEMPOTENCY_KEY_HEADER, SCHEMA_VERSION, SCHEMA_VERSION_HEADER,
};
use asd_contract::signing::canonical_bytes;
use asd_core::types::CaptureSource;

use crate::SyncError;

/// Path of the session endpoint, relative to the API base URL.
pub const SESSIONS_PATH: &str = "v1/sessions";
/// Path of the registration endpoint, relative to the API base URL.
pub const REGISTER_PATH: &str = "v1/devices/register";
/// Request timeout of a session post.
pub const SESSION_TIMEOUT_SECS: u32 = 20;
/// Request timeout of a registration.
pub const REGISTER_TIMEOUT_SECS: u32 = 15;

const JSON: &str = "application/json";

/// App identity sent as `User-Agent`. No device model, name or identifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientInfo {
    /// Product token, `EyeGaze-iOS`.
    pub product: String,
    pub app_version: String,
    pub build: String,
    /// `iOS`, `Android`.
    pub os_name: String,
    pub os_version: String,
}

impl ClientInfo {
    /// The iOS app's identity (`SyncClientInfo`).
    pub fn ios(app_version: &str, build: &str, os_version: &str) -> Self {
        Self {
            product: "EyeGaze-iOS".into(),
            app_version: app_version.into(),
            build: build.into(),
            os_name: "iOS".into(),
            os_version: os_version.into(),
        }
    }

    /// `<product>/<version> (<build>; <os> <os version>)`.
    pub fn user_agent(&self) -> String {
        format!(
            "{}/{} ({}; {} {})",
            self.product, self.app_version, self.build, self.os_name, self.os_version
        )
    }
}

/// A request for the app's transport to send. `path` is relative to the API base URL
/// (join it with [`crate::endpoint_url`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncRequest {
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    pub timeout_secs: u32,
}

fn header(name: &str, value: impl Into<String>) -> (String, String) {
    (name.to_string(), value.into())
}

/// `YYYY-MM-DDTHH:MM:SS.sssZ` with plausible field ranges (what `SessionPayloadCoding`
/// writes).
fn is_rfc3339_utc_millis(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 24 {
        return false;
    }
    let digits = |range: std::ops::Range<usize>| b[range].iter().all(u8::is_ascii_digit);
    let num = |range: std::ops::Range<usize>| -> u32 { s[range].parse().unwrap_or(u32::MAX) };
    let shape = digits(0..4)
        && b[4] == b'-'
        && digits(5..7)
        && b[7] == b'-'
        && digits(8..10)
        && b[10] == b'T'
        && digits(11..13)
        && b[13] == b':'
        && digits(14..16)
        && b[16] == b':'
        && digits(17..19)
        && b[19] == b'.'
        && digits(20..23)
        && b[23] == b'Z';
    shape
        && (1..=12).contains(&num(5..7))
        && (1..=31).contains(&num(8..10))
        && num(11..13) <= 23
        && num(14..16) <= 59
        && num(17..19) <= 60
}

/// Builds `POST /v1/sessions` (CON-7): `Authorization: Bearer`, `X-Idempotency-Key =
/// sessionId`, `X-Schema-Version`, `User-Agent`, `Content-Type`, `Accept`; the body is the
/// RFC 8785 form of the payload (sorted keys; whole doubles as `1` like Swift's encoder),
/// so a retry sends identical bytes.
pub fn build_session_request(
    payload: &SessionIngestRequest,
    device_token: &str,
    client: &ClientInfo,
) -> Result<SyncRequest, SyncError> {
    let token = device_token.trim();
    if token.is_empty() {
        return Err(SyncError::NotRegistered);
    }
    if payload.capture_source == CaptureSource::Simulated {
        return Err(SyncError::SimulatedCapture);
    }
    if payload.raw_payload.legacy_provenance == Some(true) {
        return Err(SyncError::LegacyProvenance);
    }
    if payload.schema_version != SCHEMA_VERSION {
        return Err(SyncError::UnsupportedSchemaVersion(
            payload.schema_version.clone(),
        ));
    }
    for (field, value) in [
        ("rawPayload.startedAt", &payload.raw_payload.started_at),
        ("rawPayload.endedAt", &payload.raw_payload.ended_at),
    ] {
        if !is_rfc3339_utc_millis(value) {
            return Err(SyncError::InvalidDate {
                field,
                value: value.clone(),
            });
        }
    }
    let body = canonical_bytes(payload).map_err(|e| SyncError::InvalidPayload(e.to_string()))?;
    Ok(SyncRequest {
        method: "POST".into(),
        path: SESSIONS_PATH.into(),
        headers: vec![
            header("Content-Type", JSON),
            header("Accept", JSON),
            header("Authorization", format!("Bearer {token}")),
            header(IDEMPOTENCY_KEY_HEADER, payload.session_id.clone()),
            header(SCHEMA_VERSION_HEADER, payload.schema_version.clone()),
            header("User-Agent", client.user_agent()),
        ],
        body,
        timeout_secs: SESSION_TIMEOUT_SECS,
    })
}

/// [`build_session_request`] from payload JSON (the FFI entry point): the document is
/// decoded with the contract types first, so a payload the server would reject with
/// `validation-failed` never leaves the device.
pub fn build_session_request_json(
    payload_json: &[u8],
    device_token: &str,
    client: &ClientInfo,
) -> Result<SyncRequest, SyncError> {
    let payload: SessionIngestRequest = serde_json::from_slice(payload_json)
        .map_err(|e| SyncError::InvalidPayload(e.to_string()))?;
    build_session_request(&payload, device_token, client)
}

/// Builds `POST /v1/devices/register`. Non-session calls carry a fresh UUID idempotency key
/// supplied by the caller (ADR-B5); no bearer. `device_id` is sent on re-registration so
/// the server rotates that device's token (CON-11).
pub fn build_register_request(
    enrol_code: &str,
    device_id: Option<&str>,
    idempotency_key: &str,
    client: &ClientInfo,
) -> Result<SyncRequest, SyncError> {
    let code = enrol_code.trim();
    if code.is_empty() {
        return Err(SyncError::EmptyEnrolCode);
    }
    let body = RegisterDeviceRequest {
        enrol_code: code.into(),
        platform: client.os_name.clone(),
        app_version: client.app_version.clone(),
        device_id: device_id.map(str::to_string),
    };
    let body = canonical_bytes(&body).map_err(|e| SyncError::InvalidPayload(e.to_string()))?;
    Ok(SyncRequest {
        method: "POST".into(),
        path: REGISTER_PATH.into(),
        headers: vec![
            header("Content-Type", JSON),
            header("Accept", JSON),
            header("User-Agent", client.user_agent()),
            header(IDEMPOTENCY_KEY_HEADER, idempotency_key),
        ],
        body,
        timeout_secs: REGISTER_TIMEOUT_SECS,
    })
}

#[cfg(test)]
mod tests {
    use super::is_rfc3339_utc_millis;

    #[test]
    fn millisecond_utc_dates() {
        assert!(is_rfc3339_utc_millis("2026-10-26T07:33:20.000Z"));
        assert!(is_rfc3339_utc_millis("2026-12-31T23:59:60.999Z"));
        assert!(!is_rfc3339_utc_millis("2026-00-26T07:33:20.000Z"));
        assert!(!is_rfc3339_utc_millis("2026-10-26T24:00:00.000Z"));
        assert!(!is_rfc3339_utc_millis("2026-10-26T07:33:20.0000"));
        assert!(!is_rfc3339_utc_millis("２026-10-26T07:33:20.000Z"));
    }
}
