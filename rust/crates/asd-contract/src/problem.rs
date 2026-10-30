//! owner: WP-R.3
//!
//! RFC 7807 `application/problem+json` bodies for every `/v1` error. `type` is
//! `https://schemas.curv.org/asd/problems/<slug>`; the slugs used by the spec are constants
//! here so the backend and the clients share one spelling.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Media type of every error body.
pub const CONTENT_TYPE: &str = "application/problem+json";
/// Prefix of every problem `type` URI.
pub const TYPE_PREFIX: &str = "https://schemas.curv.org/asd/problems/";

/// Missing or malformed field (CON-14); carries `invalid_params`.
pub const VALIDATION_FAILED: &str = "validation-failed";
/// `X-Schema-Version` and `schemaVersion` differ or are not `1.6` (CON-21).
pub const UNSUPPORTED_SCHEMA_VERSION: &str = "unsupported-schema-version";
/// Same idempotency key with a different body (CON-17).
pub const IDEMPOTENCY_KEY_REUSED: &str = "idempotency-key-reused";
/// Enrol code used up or expired, no `device_id` given (CON-11).
pub const ENROL_CODE_EXHAUSTED: &str = "enrol-code-exhausted";
/// `captureSource` refused: `simulated` always, `synthetic` unless allowed (CON-13).
pub const CAPTURE_SOURCE_REJECTED: &str = "capture-source-rejected";
/// Missing, invalid or revoked credentials.
pub const UNAUTHORIZED: &str = "unauthorized";
/// Authenticated but the role does not allow the action.
pub const FORBIDDEN: &str = "forbidden";
/// Unknown resource, or another org's resource (AUTH-4).
pub const NOT_FOUND: &str = "not-found";
/// Rate limit hit; the response carries `Retry-After`.
pub const RATE_LIMITED: &str = "rate-limited";
/// State conflict other than idempotency (for example a lifecycle transition).
pub const CONFLICT: &str = "conflict";
/// 409: the participant withdrew consent; ingest and blob uploads for them are refused.
pub const PARTICIPANT_WITHDRAWN: &str = "participant-withdrawn";
/// Unexpected server error (no details leaked).
pub const INTERNAL: &str = "internal";

/// Every slug above, published as `$defs/ProblemSlug` in `api-v1.json` so clients can
/// check their spelling. Route-specific slugs (e.g. `email-taken`, `last-owner`) are
/// documented with their route and are not listed here.
pub const SLUGS: &[&str] = &[
    VALIDATION_FAILED,
    UNSUPPORTED_SCHEMA_VERSION,
    IDEMPOTENCY_KEY_REUSED,
    ENROL_CODE_EXHAUSTED,
    CAPTURE_SOURCE_REJECTED,
    UNAUTHORIZED,
    FORBIDDEN,
    NOT_FOUND,
    RATE_LIMITED,
    CONFLICT,
    PARTICIPANT_WITHDRAWN,
    INTERNAL,
];

/// Full `type` URI for a slug.
pub fn type_uri(slug: &str) -> String {
    format!("{TYPE_PREFIX}{slug}")
}

/// One rejected request parameter (`invalid_params[]`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct InvalidParam {
    /// JSON pointer or field name, e.g. `sessionId` or `/perVideo/0/dwell_social`.
    pub name: String,
    pub reason: String,
}

/// RFC 7807 problem details.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(example = example_problem())]
pub struct Problem {
    #[serde(rename = "type")]
    pub type_uri: String,
    pub title: String,
    pub status: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub invalid_params: Vec<InvalidParam>,
}

impl Problem {
    /// A problem with the `type` URI for `slug`.
    pub fn new(slug: &str, status: u16, title: impl Into<String>) -> Self {
        Self {
            type_uri: type_uri(slug),
            title: title.into(),
            status,
            detail: None,
            instance: None,
            invalid_params: Vec::new(),
        }
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn with_instance(mut self, instance: impl Into<String>) -> Self {
        self.instance = Some(instance.into());
        self
    }

    pub fn with_invalid_param(
        mut self,
        name: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        self.invalid_params.push(InvalidParam {
            name: name.into(),
            reason: reason.into(),
        });
        self
    }
}

fn example_problem() -> Problem {
    Problem::new(VALIDATION_FAILED, 400, "Validation failed")
        .with_detail("sessionId is required")
        .with_invalid_param("sessionId", "missing")
}
