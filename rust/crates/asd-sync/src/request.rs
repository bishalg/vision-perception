//! owner: WP-R.3
//!
//! Request building for the device plane (`SessionSyncClient.makeSessionRequest`,
//! `DeviceRegistrationService.register`).

use asd_contract::api_v1::RegisterDeviceRequest;
use asd_contract::session_v1_6::{
    SessionIngestRequest, IDEMPOTENCY_KEY_HEADER, SCHEMA_VERSION, SCHEMA_VERSION_HEADER,
};
use asd_contract::session_v1_7;
use asd_contract::session_v1_8;
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
    session_request(
        SessionParts {
            capture_source: payload.capture_source,
            legacy_provenance: payload.raw_payload.legacy_provenance,
            schema_version: &payload.schema_version,
            supported: SCHEMA_VERSION,
            started_at: &payload.raw_payload.started_at,
            ended_at: &payload.raw_payload.ended_at,
            session_id: &payload.session_id,
        },
        || canonical_bytes(payload),
        device_token,
        client,
    )
}

/// [`build_session_request`] for a schema 1.7 body (SCH-2: the app writes 1.7 from the
/// build that runs the Rust gaze pipeline, WP-G.9). Same rules; the body is the RFC 8785
/// form of the 1.7 contract types, so `trackingQuality` and the per-clip tracking keys are
/// carried, not dropped.
pub fn build_session_request_v1_7(
    payload: &session_v1_7::SessionIngestRequest,
    device_token: &str,
    client: &ClientInfo,
) -> Result<SyncRequest, SyncError> {
    session_request(
        SessionParts {
            capture_source: payload.capture_source,
            legacy_provenance: payload.raw_payload.legacy_provenance,
            schema_version: &payload.schema_version,
            supported: session_v1_7::SCHEMA_VERSION,
            started_at: &payload.raw_payload.started_at,
            ended_at: &payload.raw_payload.ended_at,
            session_id: &payload.session_id,
        },
        || canonical_bytes(payload),
        device_token,
        client,
    )
}

/// [`build_session_request`] for a schema 1.8 body (SCH-5: explicit `scoringStatus`; an
/// `unscorable` session carries no score numbers). Same rules, plus the 1.8 scoring rules
/// (`session_v1_8::scoring_violations`): a body the server would refuse with
/// `validation-failed` is [`SyncError::InvalidPayload`] here and never leaves the device.
pub fn build_session_request_v1_8(
    payload: &session_v1_8::SessionIngestRequest,
    device_token: &str,
    client: &ClientInfo,
) -> Result<SyncRequest, SyncError> {
    session_request(
        SessionParts {
            capture_source: payload.capture_source,
            legacy_provenance: payload.raw_payload.legacy_provenance,
            schema_version: &payload.schema_version,
            supported: session_v1_8::SCHEMA_VERSION,
            started_at: &payload.raw_payload.started_at,
            ended_at: &payload.raw_payload.ended_at,
            session_id: &payload.session_id,
        },
        || {
            if let Some(v) = session_v1_8::scoring_violations(payload).first() {
                return Err(format!("{}: {}", v.pointer, v.reason));
            }
            canonical_bytes(payload).map_err(|e| e.to_string())
        },
        device_token,
        client,
    )
}

/// The fields the request rules read, from either contract version.
struct SessionParts<'a> {
    capture_source: CaptureSource,
    legacy_provenance: Option<bool>,
    schema_version: &'a str,
    supported: &'a str,
    started_at: &'a str,
    ended_at: &'a str,
    session_id: &'a str,
}

fn session_request<E: std::fmt::Display>(
    parts: SessionParts<'_>,
    body: impl FnOnce() -> Result<Vec<u8>, E>,
    device_token: &str,
    client: &ClientInfo,
) -> Result<SyncRequest, SyncError> {
    let token = device_token.trim();
    if token.is_empty() {
        return Err(SyncError::NotRegistered);
    }
    if parts.capture_source == CaptureSource::Simulated {
        return Err(SyncError::SimulatedCapture);
    }
    if parts.legacy_provenance == Some(true) {
        return Err(SyncError::LegacyProvenance);
    }
    if parts.schema_version != parts.supported {
        return Err(SyncError::UnsupportedSchemaVersion(
            parts.schema_version.to_string(),
        ));
    }
    for (field, value) in [
        ("rawPayload.startedAt", parts.started_at),
        ("rawPayload.endedAt", parts.ended_at),
    ] {
        if !is_rfc3339_utc_millis(value) {
            return Err(SyncError::InvalidDate {
                field,
                value: value.to_string(),
            });
        }
    }
    let body = body().map_err(|e| SyncError::InvalidPayload(e.to_string()))?;
    Ok(SyncRequest {
        method: "POST".into(),
        path: SESSIONS_PATH.into(),
        headers: vec![
            header("Content-Type", JSON),
            header("Accept", JSON),
            header("Authorization", format!("Bearer {token}")),
            header(IDEMPOTENCY_KEY_HEADER, parts.session_id.to_string()),
            header(SCHEMA_VERSION_HEADER, parts.schema_version.to_string()),
            header("User-Agent", client.user_agent()),
        ],
        body,
        timeout_secs: SESSION_TIMEOUT_SECS,
    })
}

/// [`build_session_request`] from payload JSON (the FFI entry point): the document is
/// decoded with the contract types of its `schemaVersion` (`1.6`, `1.7` or `1.8`) first, so
/// a payload the server would reject with `validation-failed` never leaves the device.
pub fn build_session_request_json(
    payload_json: &[u8],
    device_token: &str,
    client: &ClientInfo,
) -> Result<SyncRequest, SyncError> {
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Version {
        schema_version: Option<String>,
    }
    let invalid = |e: serde_json::Error| SyncError::InvalidPayload(e.to_string());
    let version: Version = serde_json::from_slice(payload_json).map_err(invalid)?;
    if version.schema_version.as_deref() == Some(session_v1_8::SCHEMA_VERSION) {
        let payload: session_v1_8::SessionIngestRequest =
            serde_json::from_slice(payload_json).map_err(invalid)?;
        return build_session_request_v1_8(&payload, device_token, client);
    }
    if version.schema_version.as_deref() == Some(session_v1_7::SCHEMA_VERSION) {
        let payload: session_v1_7::SessionIngestRequest =
            serde_json::from_slice(payload_json).map_err(invalid)?;
        return build_session_request_v1_7(&payload, device_token, client);
    }
    let payload: SessionIngestRequest = serde_json::from_slice(payload_json).map_err(invalid)?;
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
