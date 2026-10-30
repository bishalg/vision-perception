//! owner: WP-B.3
//!
//! Request validation for `POST /v1/sessions` (ING-1, CON-13, CON-14, CON-21). Order:
//! JSON syntax, schema version (header and body), the exported session JSON Schema, typed
//! decode, then the checks the schema cannot express (capture source policy, identifier
//! charset, `rawPayload` consistency, unique clip ids, RFC 3339 dates, idempotency key).
//! Nothing here fills in a missing value (CON-14).

use asd_contract::problem as slugs;
use asd_contract::schema;
use asd_contract::session_v1_6::{SessionIngestRequest, SCHEMA_VERSION};
use asd_core::types::CaptureSource;
use chrono::{DateTime, Utc};
use serde_json::Value;

use super::idempotency;
use crate::error::AppError;
use crate::problem::InvalidParam;

/// Longest accepted `sessionId` / `participantToken`.
pub const MAX_ID_LEN: usize = 128;

/// A request that passed every check.
#[derive(Debug, Clone)]
pub struct ValidatedSession {
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

    // CON-21: the header is mandatory, supported, and equal to the body's version.
    match schema_version_header {
        None => return Err(unsupported("X-Schema-Version header is required".into())),
        Some(v) if v != SCHEMA_VERSION => {
            return Err(unsupported(format!(
                "X-Schema-Version {v} is not supported (expected {SCHEMA_VERSION})"
            )))
        }
        Some(_) => {}
    }
    if let Some(body_version) = value.get("schemaVersion").and_then(Value::as_str) {
        if body_version != SCHEMA_VERSION {
            return Err(unsupported(format!(
                "schemaVersion {body_version} does not match X-Schema-Version {SCHEMA_VERSION}"
            )));
        }
    }

    if let Err(errors) = schema::validate_session(&value) {
        return Err(AppError::Validation(
            errors.iter().map(|e| schema_param(e)).collect(),
        ));
    }
    let request: SessionIngestRequest = serde_json::from_value(value.clone())
        .map_err(|e| AppError::Validation(vec![param("/", e.to_string())]))?;

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
