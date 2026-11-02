//! owner: WP-R.3
//!
//! What an HTTP answer means for the upload queue (`SessionSyncClient.postSession`,
//! `ResearchKitSyncService.processSession`, `retryAfterSeconds`).

use asd_contract::api_v1::RegisterDeviceResponse;
use asd_contract::problem;
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

/// Classifies the answer to `POST /v1/sessions` (and a blob `PUT`) reading the `409` body
/// (SYNC-7). Every other status is classified exactly as [`classify_response`] does.
///
/// A `409` is classified on the problem slug first (`type` ends in `/<slug>`, RFC 7807):
/// `idempotency-key-reused` or `participant-withdrawn` -> `Fatal` (quarantine: the item is
/// kept for inspection, never deleted). Otherwise a body with `duplicate: true` and, by
/// default, any other body (empty, not JSON, another slug, as older servers send) ->
/// `Duplicate` (a replay of a globally unique session id: delivered, CON-17).
pub fn classify_response_with_body(
    status: u16,
    headers: &[(String, String)],
    body: &[u8],
) -> SyncOutcome {
    if status != 409 {
        return classify_response(status, headers, body);
    }
    let slug = serde_json::from_slice::<serde_json::Value>(body)
        .ok()
        .and_then(|v| v.get("type")?.as_str().map(|t| problem_slug(t).to_owned()));
    match slug.as_deref() {
        Some(problem::IDEMPOTENCY_KEY_REUSED | problem::PARTICIPANT_WITHDRAWN) => {
            SyncOutcome::Fatal { status }
        }
        // `{duplicate: true}` and the default (older servers) are the same answer.
        _ => SyncOutcome::Duplicate,
    }
}

/// The slug of a problem `type` URI: the text after the last `/` (the whole value when it
/// has none), so both `https://schemas.curv.org/asd/problems/<slug>` and a bare slug match.
fn problem_slug(type_uri: &str) -> &str {
    type_uri.trim().rsplit('/').next().unwrap_or_default()
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn problem_body(slug: &str) -> Vec<u8> {
        serde_json::to_vec(&json!({
            "type": format!("{}{slug}", problem::TYPE_PREFIX),
            "title": "Conflict",
            "status": 409,
        }))
        .unwrap()
    }

    #[test]
    fn sync7_409_slugs_are_fatal_and_replays_are_duplicates() {
        for slug in [
            problem::IDEMPOTENCY_KEY_REUSED,
            problem::PARTICIPANT_WITHDRAWN,
        ] {
            assert_eq!(
                classify_response_with_body(409, &[], &problem_body(slug)),
                SyncOutcome::Fatal { status: 409 },
                "{slug}"
            );
        }
        let replay = serde_json::to_vec(&json!({"duplicate": true, "sessionId": "S-1"})).unwrap();
        assert_eq!(
            classify_response_with_body(409, &[], &replay),
            SyncOutcome::Duplicate
        );
        // Older servers: no body, or a body that is not a problem.
        for body in [
            &b""[..],
            b"{}",
            b"conflict",
            b"[]",
            b"null",
            b"{\"duplicate\":false}",
        ] {
            assert_eq!(
                classify_response_with_body(409, &[], body),
                SyncOutcome::Duplicate
            );
        }
        // Another slug (or `about:blank`) is the default: a replay.
        assert_eq!(
            classify_response_with_body(409, &[], &problem_body(problem::CONFLICT)),
            SyncOutcome::Duplicate
        );
        let blank = serde_json::to_vec(&json!({"type": "about:blank", "status": 409})).unwrap();
        assert_eq!(
            classify_response_with_body(409, &[], &blank),
            SyncOutcome::Duplicate
        );
        // The slug wins over `duplicate: true`.
        let both = serde_json::to_vec(&json!({
            "type": format!("{}{}", problem::TYPE_PREFIX, problem::PARTICIPANT_WITHDRAWN),
            "duplicate": true,
        }))
        .unwrap();
        assert_eq!(
            classify_response_with_body(409, &[], &both),
            SyncOutcome::Fatal { status: 409 }
        );
        // A bare slug in `type` is read the same way.
        let bare = serde_json::to_vec(&json!({"type": "participant-withdrawn"})).unwrap();
        assert_eq!(
            classify_response_with_body(409, &[], &bare),
            SyncOutcome::Fatal { status: 409 }
        );
        // The old function is unchanged: every 409 is a duplicate.
        assert_eq!(
            classify_response(409, &[], &problem_body(problem::PARTICIPANT_WITHDRAWN)),
            SyncOutcome::Duplicate
        );
    }

    /// Deterministic xorshift so the property test needs no extra crate.
    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
        fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
            &items[(self.next() % items.len() as u64) as usize]
        }
    }

    /// Property: for any status other than 409 the new classifier equals the old one; for
    /// 409 the outcome is only ever `Duplicate` or `Fatal { 409 }`, and it is `Fatal`
    /// exactly when the body is a JSON object whose string `type` ends in a fatal slug,
    /// whatever `duplicate` says.
    #[test]
    fn sync7_property_matches_the_rule_for_generated_bodies() {
        let slugs = [
            problem::IDEMPOTENCY_KEY_REUSED,
            problem::PARTICIPANT_WITHDRAWN,
            problem::CONFLICT,
            problem::VALIDATION_FAILED,
            "",
        ];
        let prefixes = [problem::TYPE_PREFIX, "", "urn:x/", " "];
        let headers = vec![("Retry-After".to_owned(), "7".to_owned())];
        let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
        for _ in 0..5000 {
            let mut map = serde_json::Map::new();
            let slug = *rng.pick(&slugs);
            let has_type = !rng.next().is_multiple_of(3);
            if has_type {
                map.insert(
                    "type".into(),
                    json!(format!("{}{slug}", rng.pick(&prefixes))),
                );
            }
            let duplicate = match rng.next() % 4 {
                0 => Some(json!(true)),
                1 => Some(json!(false)),
                2 => Some(json!("true")),
                _ => None,
            };
            if let Some(d) = &duplicate {
                map.insert("duplicate".into(), d.clone());
            }
            if rng.next().is_multiple_of(2) {
                map.insert("detail".into(), json!("x"));
            }
            let body = match rng.next() % 6 {
                0 => Vec::new(),
                1 => b"not json".to_vec(),
                _ => serde_json::to_vec(&serde_json::Value::Object(map.clone())).unwrap(),
            };
            let status = *rng.pick(&[
                200u16, 201, 400, 401, 403, 404, 408, 409, 409, 409, 429, 500, 503,
            ]);
            let got = classify_response_with_body(status, &headers, &body);
            if status != 409 {
                assert_eq!(got, classify_response(status, &headers, &body));
                continue;
            }
            let parsed: Option<serde_json::Value> = serde_json::from_slice(&body).ok();
            let expected_fatal = match parsed.as_ref().and_then(|v| v.as_object()) {
                Some(obj) => match obj.get("type").and_then(|t| t.as_str()) {
                    Some(t) => {
                        let s = t.trim().rsplit('/').next().unwrap_or("");
                        s == problem::IDEMPOTENCY_KEY_REUSED || s == problem::PARTICIPANT_WITHDRAWN
                    }
                    None => false,
                },
                None => false,
            };
            let expected = if expected_fatal {
                SyncOutcome::Fatal { status: 409 }
            } else {
                SyncOutcome::Duplicate
            };
            assert_eq!(got, expected, "body {}", String::from_utf8_lossy(&body));
        }
    }
}
