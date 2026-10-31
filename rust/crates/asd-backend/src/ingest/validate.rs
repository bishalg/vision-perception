//! owner: WP-B.3
//!
//! Request validation for `POST /v1/sessions` (ING-1, CON-13, CON-14, CON-21). Order:
//! JSON syntax, schema version (header and body), the exported session JSON Schema, typed
//! decode, then the checks the schema cannot express (capture source policy, identifier
//! charset, `rawPayload` consistency, unique clip ids, RFC 3339 dates, idempotency key).
//! Nothing here fills in a missing value (CON-14).
//!
//! Schema versions (SCH-2, WP-G.11): `X-Schema-Version` is `1.6` or `1.7` and the body's
//! `schemaVersion` must equal it. Each version is checked against its own exported schema
//! and decoded as its own type, so a 1.6 body is accepted or refused exactly as before
//! (1.6 allows unknown keys that 1.7 types, such as `rawPayload.trackingQuality`). A 1.6
//! request is then carried as the 1.7 type ([`upgrade_v1_6`], lossless: 1.7 is 1.6 plus
//! optional fields), with no tracking-quality record.

use asd_contract::problem as slugs;
use asd_contract::schema;
use asd_contract::session_v1_6;
use asd_contract::session_v1_7::{self, SessionIngestRequest};
use asd_core::types::CaptureSource;
use chrono::{DateTime, Utc};
use serde_json::Value;

use super::idempotency;
use crate::error::AppError;
use crate::problem::InvalidParam;

/// Longest accepted `sessionId` / `participantToken`.
pub const MAX_ID_LEN: usize = 128;

/// Session schema versions `POST /v1/sessions` accepts (SCH-2; CON-21 becomes a set).
pub const SUPPORTED_SCHEMA_VERSIONS: [&str; 2] =
    [session_v1_6::SCHEMA_VERSION, session_v1_7::SCHEMA_VERSION];

/// A request that passed every check.
#[derive(Debug, Clone)]
pub struct ValidatedSession {
    /// The request as the 1.7 type; `schema_version` says which version was sent (a 1.6
    /// request has no tracking-quality record and no QS-3 fields).
    pub request: SessionIngestRequest,
    /// SHA-256 of the JCS canonical body (key order and whitespace do not matter).
    pub request_hash: [u8; 32],
    /// `X-Idempotency-Key`, which must equal `sessionId` (ADR-B5); `sessionId` when absent.
    pub idempotency_key: String,
    pub started_at: DateTime<Utc>,
    pub ended_at: DateTime<Utc>,
    /// `rawPayload` exactly as received (stored as `sessions.summary`, ING-2).
    pub raw_payload: Value,
    /// `perVideo[].details` exactly as received, by index.
    pub per_video_details: Vec<Value>,
}

fn param(name: impl Into<String>, reason: impl Into<String>) -> InvalidParam {
    InvalidParam {
        name: name.into(),
        reason: reason.into(),
    }
}

fn unsupported(detail: String) -> AppError {
    AppError::BadRequest {
        slug: slugs::UNSUPPORTED_SCHEMA_VERSION,
        detail,
    }
}

/// Identifiers end up in blob keys and URL paths: `[A-Za-z0-9._~-]`, 1..=128 characters,
/// not only dots.
pub fn is_safe_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_ID_LEN
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'~' | b'-'))
        && !id.bytes().all(|b| b == b'.')
}

/// Maps one `jsonschema` message (`"<pointer>: <message>"`) to an invalid parameter. A
/// missing property is named by its own pointer (`/sessionId`), not by its parent.
fn schema_param(error: &str) -> InvalidParam {
    let (pointer, message) = error.split_once(": ").unwrap_or(("/", error));
    if let Some(prop) = message
        .strip_suffix(" is a required property")
        .and_then(|p| p.strip_prefix('"'))
        .and_then(|p| p.strip_suffix('"'))
    {
        let base = pointer.trim_end_matches('/');
        return param(format!("{base}/{prop}"), "required");
    }
    param(pointer, message)
}

fn parse_date(value: &str, name: &str, errors: &mut Vec<InvalidParam>) -> Option<DateTime<Utc>> {
    match DateTime::parse_from_rfc3339(value) {
        Ok(d) => Some(d.with_timezone(&Utc)),
        Err(_) => {
            errors.push(param(name, "must be an RFC 3339 date-time"));
            None
        }
    }
}

/// Validates the raw body and headers. `allow_synthetic` is `ASD_ALLOW_SYNTHETIC`.
pub fn validate(
    body: &[u8],
    schema_version_header: Option<&str>,
    idempotency_key_header: Option<&str>,
    allow_synthetic: bool,
) -> Result<ValidatedSession, AppError> {
    let value: Value = serde_json::from_slice(body).map_err(|e| {
        AppError::Validation(vec![param(
            "/",
            format!("body is not valid JSON: {}", e.classify_label()),
        )])
    })?;

    // CON-21 / SCH-2: the header is mandatory, supported, and equal to the body's version.
    let version = match schema_version_header {
        None => return Err(unsupported("X-Schema-Version header is required".into())),
        Some(v) if !SUPPORTED_SCHEMA_VERSIONS.contains(&v) => {
            return Err(unsupported(format!(
                "X-Schema-Version {v} is not supported (expected one of {})",
                SUPPORTED_SCHEMA_VERSIONS.join(", ")
            )))
        }
        Some(v) => v,
    };
    if let Some(body_version) = value.get("schemaVersion").and_then(Value::as_str) {
        if body_version != version {
            return Err(unsupported(format!(
                "schemaVersion {body_version} does not match X-Schema-Version {version}"
            )));
        }
    }

    let is_v1_7 = version == session_v1_7::SCHEMA_VERSION;
    let checked = if is_v1_7 {
        schema::validate_session_v1_7(&value)
    } else {
        schema::validate_session(&value)
    };
    if let Err(errors) = checked {
        return Err(AppError::Validation(
            errors.iter().map(|e| schema_param(e)).collect(),
        ));
    }
    let decode_err = |e: serde_json::Error| AppError::Validation(vec![param("/", e.to_string())]);
    let request: SessionIngestRequest = if is_v1_7 {
        serde_json::from_value(value.clone()).map_err(decode_err)?
    } else {
        upgrade_v1_6(serde_json::from_value(value.clone()).map_err(decode_err)?)
    };

    // CON-13: capture-source policy.
    match request.capture_source {
        CaptureSource::Simulated => {
            return Err(AppError::BadRequest {
                slug: slugs::CAPTURE_SOURCE_REJECTED,
                detail: "simulated sessions are never accepted".into(),
            })
        }
        CaptureSource::Synthetic if !allow_synthetic => {
            return Err(AppError::BadRequest {
                slug: slugs::CAPTURE_SOURCE_REJECTED,
                detail: "synthetic sessions are not accepted by this server".into(),
            })
        }
        _ => {}
    }

    let mut errors = Vec::new();
    for (name, id) in [
        ("/sessionId", &request.session_id),
        ("/participantToken", &request.participant_token),
    ] {
        if !is_safe_id(id) {
            errors.push(param(
                name,
                format!("must be 1-{MAX_ID_LEN} characters of [A-Za-z0-9._~-]"),
            ));
        }
    }
    let raw = &request.raw_payload;
    if raw.session_id != request.session_id {
        errors.push(param("/rawPayload/sessionId", "must equal sessionId"));
    }
    if raw.participant_id != request.participant_token {
        errors.push(param(
            "/rawPayload/participantId",
            "must equal participantToken",
        ));
    }
    if raw.capture_source != request.capture_source {
        errors.push(param(
            "/rawPayload/captureSource",
            "must equal captureSource",
        ));
    }
    if let Some(rows) = &request.per_video {
        let mut seen = std::collections::BTreeSet::new();
        for (i, row) in rows.iter().enumerate() {
            if !seen.insert(row.clip_id.as_str()) {
                errors.push(param(format!("/perVideo/{i}/clip_id"), "duplicate clip_id"));
            }
            if i32::try_from(row.order).is_err() {
                errors.push(param(format!("/perVideo/{i}/order"), "out of range"));
            }
            if row
                .fixation_count
                .is_some_and(|n| i32::try_from(n).is_err())
            {
                errors.push(param(
                    format!("/perVideo/{i}/fixation_count"),
                    "out of range",
                ));
            }
        }
    }
    let started_at = parse_date(&raw.started_at, "/rawPayload/startedAt", &mut errors);
    let ended_at = parse_date(&raw.ended_at, "/rawPayload/endedAt", &mut errors);

    let idempotency_key = match idempotency_key_header {
        Some(k) if k != request.session_id => {
            errors.push(param("X-Idempotency-Key", "must equal sessionId"));
            String::new()
        }
        Some(k) => k.to_owned(),
        None => request.session_id.clone(),
    };

    match (started_at, ended_at) {
        (Some(started_at), Some(ended_at)) if errors.is_empty() => Ok(ValidatedSession {
            request_hash: idempotency::request_hash(&value)?,
            request,
            idempotency_key,
            started_at,
            ended_at,
            per_video_details: value
                .get("perVideo")
                .and_then(Value::as_array)
                .map(|rows| rows.iter().map(|r| r["details"].clone()).collect())
                .unwrap_or_default(),
            raw_payload: value.get("rawPayload").cloned().unwrap_or(Value::Null),
        }),
        _ => Err(AppError::Validation(errors)),
    }
}

/// A decoded 1.6 request as the 1.7 type, without loss: every 1.6 field maps to the
/// same 1.7 field, the 1.7-only fields stay absent (no tracking record, no QS-3 fields), and
/// unknown `rawPayload` keys stay in `extra`. Exhaustive destructuring: a field added to
/// either version fails to compile here.
pub fn upgrade_v1_6(req: session_v1_6::SessionIngestRequest) -> SessionIngestRequest {
    let session_v1_6::SessionIngestRequest {
        session_id,
        participant_token,
        capture_source,
        duration_sec,
        valid_ratio,
        social_preference_score,
        eye_dwell_ratio,
        mouth_dwell_ratio,
        first_eye_latency_ms,
        quality_verdict,
        calibrated_distance_cm,
        attention_flag,
        battery,
        schema_version,
        per_video,
        raw_payload,
    } = req;
    SessionIngestRequest {
        session_id,
        participant_token,
        capture_source,
        duration_sec,
        valid_ratio,
        social_preference_score,
        eye_dwell_ratio,
        mouth_dwell_ratio,
        first_eye_latency_ms,
        quality_verdict,
        calibrated_distance_cm,
        attention_flag,
        battery,
        schema_version,
        per_video: per_video.map(|rows| rows.into_iter().map(upgrade_row).collect()),
        raw_payload: upgrade_record(raw_payload),
    }
}

fn upgrade_row(row: session_v1_6::SessionPerVideoRow) -> session_v1_7::SessionPerVideoRow {
    let session_v1_6::SessionPerVideoRow {
        clip_id,
        order,
        dwell_social,
        dwell_geometric,
        dwell_eye,
        dwell_mouth,
        orienting_latency_ms,
        fixation_count,
        mean_fixation_ms,
        status,
        score,
        weight,
        details,
    } = row;
    session_v1_7::SessionPerVideoRow {
        clip_id,
        order,
        dwell_social,
        dwell_geometric,
        dwell_eye,
        dwell_mouth,
        orienting_latency_ms,
        fixation_count,
        mean_fixation_ms,
        status,
        score,
        weight,
        details: session_v1_7::SessionPerVideoDetails {
            base: details,
            tracking: session_v1_7::ClipTracking::default(),
        },
    }
}

fn upgrade_record(r: session_v1_6::GazeSessionRecord) -> session_v1_7::GazeSessionRecord {
    let session_v1_6::GazeSessionRecord {
        id,
        session_id,
        participant_id,
        capture_source,
        started_at,
        ended_at,
        duration_sec,
        social_preference_score,
        valid_frame_ratio,
        eye_dwell_ratio,
        mouth_dwell_ratio,
        first_eye_latency_ms,
        calibrated_distance_cm,
        quality_verdict,
        social_smile_intensity,
        smile_reactivity_index,
        flat_affect_score,
        surprise_score_mean,
        notes,
        schema_version,
        zero_raw_video_enforced,
        legacy_provenance,
        per_video_composite,
        per_video,
        extra,
    } = r;
    session_v1_7::GazeSessionRecord {
        id,
        session_id,
        participant_id,
        capture_source,
        started_at,
        ended_at,
        duration_sec,
        social_preference_score,
        valid_frame_ratio,
        eye_dwell_ratio,
        mouth_dwell_ratio,
        first_eye_latency_ms,
        calibrated_distance_cm: Some(calibrated_distance_cm),
        quality_verdict,
        social_smile_intensity,
        smile_reactivity_index,
        flat_affect_score,
        surprise_score_mean,
        notes,
        schema_version,
        zero_raw_video_enforced,
        legacy_provenance,
        per_video_composite,
        per_video: per_video.map(|videos| {
            videos
                .into_iter()
                .map(|report| session_v1_7::PerVideoReport {
                    report,
                    tracking: session_v1_7::ClipTracking::default(),
                })
                .collect()
        }),
        tracking_quality: None,
        extra,
    }
}

/// Short, PHI-free label of a JSON syntax error (serde's message can quote input).
trait ClassifyLabel {
    fn classify_label(&self) -> &'static str;
}

impl ClassifyLabel for serde_json::Error {
    fn classify_label(&self) -> &'static str {
        match self.classify() {
            serde_json::error::Category::Io => "io",
            serde_json::error::Category::Syntax => "syntax error",
            serde_json::error::Category::Data => "unexpected data",
            serde_json::error::Category::Eof => "unexpected end of input",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_ids() {
        for ok in ["SESS-GOLDEN-0001", "child-SESS-GOL", "a.b_c~d", "x"] {
            assert!(is_safe_id(ok), "{ok}");
        }
        for bad in [
            "",
            ".",
            "..",
            "a/b",
            "a b",
            "a?b",
            "a%2F",
            "é",
            &"x".repeat(129),
        ] {
            assert!(!is_safe_id(bad), "{bad:?}");
        }
    }

    #[test]
    fn upgrading_a_1_6_request_is_lossless() {
        let text = include_str!(
            "../../../../../apps/ios_asd_gaze_app/Tests/Fixtures/expected-session-v1.6.json"
        );
        let v16: session_v1_6::SessionIngestRequest = serde_json::from_str(text).unwrap();
        // The golden has no 1.7-only key, so the 1.7 decode of the same JSON is the target.
        let v17: SessionIngestRequest = serde_json::from_str(text).unwrap();
        let up = upgrade_v1_6(v16);
        assert_eq!(up, v17);
        assert!(up.raw_payload.tracking_quality.is_none());
        assert_eq!(up.raw_payload.calibrated_distance_cm, Some(38.0));
    }

    #[test]
    fn upgrading_keeps_unknown_record_keys_in_extra() {
        let mut v: Value = serde_json::from_str(include_str!(
            "../../../../../apps/ios_asd_gaze_app/Tests/Fixtures/expected-session-v1.6.json"
        ))
        .unwrap();
        v["rawPayload"]["trackingQuality"] = serde_json::json!({"verdict": "excellent"});
        let v16: session_v1_6::SessionIngestRequest = serde_json::from_value(v).unwrap();
        let up = upgrade_v1_6(v16);
        assert!(up.raw_payload.tracking_quality.is_none());
        assert_eq!(
            up.raw_payload.extra["trackingQuality"],
            serde_json::json!({"verdict": "excellent"})
        );
    }

    #[test]
    fn schema_messages_map_to_pointers() {
        assert_eq!(
            schema_param("/: \"sessionId\" is a required property").name,
            "/sessionId"
        );
        assert_eq!(
            schema_param("/rawPayload: \"startedAt\" is a required property").name,
            "/rawPayload/startedAt"
        );
        let p = schema_param("/validRatio: -0.1 is less than the minimum of 0");
        assert_eq!(p.name, "/validRatio");
        assert_eq!(p.reason, "-0.1 is less than the minimum of 0");
    }
}
