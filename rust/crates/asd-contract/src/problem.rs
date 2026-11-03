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
/// 401 from `POST /v1/auth/login`: email and password are right, and the user has TOTP
/// enrolled; send the request again with `totp_code` (AUTH-1).
pub const TOTP_REQUIRED: &str = "totp-required";
/// 401 from `POST /v1/auth/login`: the `totp_code` is wrong, outside the accepted window or
/// already used (AUTH-1).
pub const TOTP_INVALID: &str = "totp-invalid";
/// 401 from the TOTP enrolment routes (`/v1/users/me/totp`): the password is not the
/// caller's (AUTH-6, AUTH-8). Distinct from `unauthorized`, which means "sign in again".
pub const INVALID_CREDENTIALS: &str = "invalid-credentials";
/// 409 from `POST /v1/users/me/totp/enrol`: the caller already has TOTP; disable it first
/// (AUTH-6).
pub const TOTP_ALREADY_ENROLLED: &str = "totp-already-enrolled";
/// 410 from `POST /v1/users/me/totp/confirm`: no pending enrolment, or it is older than
/// 10 minutes; start again with `enrol` (AUTH-7).
pub const TOTP_ENROLMENT_EXPIRED: &str = "totp-enrolment-expired";
/// 404 from `DELETE /v1/users/me/totp`: the caller has no TOTP to disable (AUTH-8).
pub const TOTP_NOT_ENROLLED: &str = "totp-not-enrolled";
/// 403 from any user-plane route except `GET /v1/users/me`, `POST
/// /v1/users/me/totp/enrol`, `POST /v1/users/me/totp/confirm` and `GET /v1/orgs/{id}`:
/// the caller's role requires TOTP and the enrolment deadline has passed; enrol first
/// (AUTH-13). `detail` names the deadline.
pub const TOTP_ENROLMENT_REQUIRED: &str = "totp-enrolment-required";

// ---- clinician workspace and interop (CTR-2, WP-C.9) ------------------------------------

/// 503: `ASD_DATA_KEK` is not configured, so clinical content cannot be read or written
/// (KEY-2). The rest of the service keeps working.
pub const CLINICAL_CRYPTO_UNAVAILABLE: &str = "clinical-crypto-unavailable";
/// 409: the org's `identity.patient_ref_mode` is `none` (MRN-2).
pub const PATIENT_REF_DISABLED: &str = "patient-ref-disabled";
/// 412: `If-Match` does not name the current `row_version` (SIGN-3).
pub const STALE_VERSION: &str = "stale-version";
/// 409: the signed `content_hash` is not the current content's hash (SIGN-4).
pub const CONTENT_CHANGED: &str = "content-changed";
/// 401: step-up password or TOTP check failed (SIGN-5).
pub const REAUTHENTICATION_FAILED: &str = "reauthentication-failed";
/// 409: the cosigner is the first signer (SIGN-6).
pub const COSIGNER_MUST_DIFFER: &str = "cosigner-must-differ";
/// 409: the signer has no `display_name` (NOTE-6).
pub const SIGNER_PROFILE_INCOMPLETE: &str = "signer-profile-incomplete";
/// 410: the export artifact expired (EXJ-5).
pub const EXPORT_EXPIRED: &str = "export-expired";
/// 400: the export exceeds the row cap (XLSX-3, EXJ-9).
pub const EXPORT_TOO_LARGE: &str = "export-too-large";
/// 400: the export mode is not allowed for the purpose (XC-5).
pub const CONSENT_PURPOSE_MISMATCH: &str = "consent-purpose-mismatch";
/// Delivery status only: the SFTP host key does not match the pinned one (INTG-5).
pub const HOST_KEY_MISMATCH: &str = "host-key-mismatch";
/// 403: an actor that may not read clinical content (superadmin, org admin) cannot create a
/// user, or promote one, into a role holding clinical-content actions it lacks: it would
/// choose or know that user's password (RBAC-3, ADR-0008 D2). An owner provisions it.
pub const CLINICAL_ROLE_NOT_ASSIGNABLE: &str = "clinical-role-not-assignable";
/// 403: an org-admin TOTP reset (TRS-1) above the caller's rank (an admin on an owner), or
/// on the caller itself (`DELETE /v1/users/me/totp` is the self-service path).
pub const TOTP_RESET_FORBIDDEN: &str = "totp-reset-forbidden";

// ---- config documents (WP-B.4, CFG-1) ----------------------------------------------------

/// 403: the four-eyes rule: the author cannot approve or publish their own document.
pub const FOUR_EYES_REQUIRED: &str = "four-eyes-required";
/// 409: the lifecycle step does not apply to the document's status.
pub const INVALID_TRANSITION: &str = "invalid-transition";
/// 409: that `(kind, version)` already exists.
pub const CONFIG_VERSION_EXISTS: &str = "config-version-exists";
/// 409: a stored document body or a bundle member fails its checks.
pub const CONFIG_INVALID: &str = "config-invalid";
/// 409: a bundle needs a published document of each kind and one is missing.
pub const CONFIG_NOT_PUBLISHED: &str = "config-not-published";
/// 503: no bundle signing key is configured.
pub const BUNDLE_SIGNING_UNAVAILABLE: &str = "bundle-signing-unavailable";

// ---- EMR-lite record and share a case (REC-9, WP-R.7) ------------------------------------

/// 409: the encounter is `closed`; only `follow_up_done_at` or a reopen by `owner` or
/// `clinician` is accepted (REC-4).
pub const ENCOUNTER_CLOSED: &str = "encounter-closed";
/// 400: an encounter `summary` without `ClinicalNoteWrite` (REC-4).
pub const ENCOUNTER_SUMMARY_FORBIDDEN: &str = "encounter-summary-forbidden";
/// 400: attachment larger than 10 MiB, or empty (REC-6, REC-7).
pub const ATTACHMENT_TOO_LARGE: &str = "attachment-too-large";
/// 400: attachment media type other than PDF, JPEG or PNG (REC-6).
pub const ATTACHMENT_TYPE_NOT_ALLOWED: &str = "attachment-type-not-allowed";
/// 400: `Content-Digest` (RFC 9530) does not match the uploaded bytes (REC-7, SYNC).
pub const DIGEST_MISMATCH: &str = "digest-mismatch";
/// The share link existed and expired (SHR-9); unknown and revoked links answer a plain
/// `404 not-found` so the endpoint does not confirm ids.
pub const SHARE_LINK_EXPIRED: &str = "share-link-expired";
/// 410: the share link's `max_downloads` are used up (SHR-9).
pub const SHARE_LINK_EXHAUSTED: &str = "share-link-exhausted";
/// 401: wrong share-link passcode (SHR-9).
pub const SHARE_PASSCODE_INVALID: &str = "share-passcode-invalid";
/// 400: an identified case package without `consent_reference` and
/// `consent_recorded_at` (SHR-7).
pub const CASE_CONSENT_MISSING: &str = "case-consent-missing";
/// Participant identity fields are not enabled before wave 3 (REC-9).
pub const IDENTITY_DISABLED: &str = "identity-disabled";

// ---- researcher toolkit (ADR-0012, WP-X.0) -----------------------------------------------

/// 409: another experiment of the org has that name (EXP-2).
pub const EXPERIMENT_NAME_EXISTS: &str = "experiment-name-exists";
/// 409: the experiment was already promoted (GOV-2).
pub const EXPERIMENT_PROMOTED: &str = "experiment-promoted";
/// 409: the experiment has never been run on a cohort, so it cannot be promoted (GOV-2).
pub const EXPERIMENT_NO_RUN: &str = "experiment-no-run";
/// 400: the cohort selects no session.
pub const COHORT_EMPTY: &str = "cohort-empty";
/// 400: the cohort exceeds `ASD_ANALYSIS_MAX_SESSIONS` / `_MAX_FRAME_SESSIONS` (RUN-9).
pub const COHORT_TOO_LARGE: &str = "cohort-too-large";
/// 400: the sweep exceeds its cell or evaluation budget (SWP-2).
pub const SWEEP_TOO_LARGE: &str = "sweep-too-large";
/// 429: the org has as many queued or running analysis runs as allowed (RUN-9).
pub const ANALYSIS_QUOTA_EXCEEDED: &str = "analysis-quota-exceeded";
/// 400: `level: frames` was requested and no cohort session has a research log (RPX-3).
pub const ANALYSIS_LEVEL_UNAVAILABLE: &str = "analysis-level-unavailable";
/// 404: the session has no research frame log (RFL-4).
pub const FRAMES_NOT_AVAILABLE: &str = "frames-not-available";
/// 410: the retention sweep deleted the research frame log (RFL-4, RFL-5).
pub const FRAMES_PURGED: &str = "frames-purged";
/// 409: the session has no research consent; its frame log is refused (RFL-3).
pub const RESEARCH_CONSENT_REQUIRED: &str = "research-consent-required";
/// 400: the upload needs a `Content-Digest` header (RFL-3).
pub const DIGEST_REQUIRED: &str = "digest-required";
/// 409: no succeeded analysis run covers the candidate's body; it cannot be published (GOV-3).
pub const CONFIG_UNANALYSED: &str = "config-unanalysed";
/// 409: the job already finished; there is nothing to cancel (RUN-8).
pub const JOB_FINISHED: &str = "job-finished";
/// Run failure reason: the sealed per-session result exceeds 64 MiB (RUN-3).
pub const RESULT_TOO_LARGE: &str = "result-too-large";
/// 501: the route exists but its package has not shipped yet (phase 8 step 0 stubs).
pub const NOT_IMPLEMENTED: &str = "not-implemented";

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
    TOTP_REQUIRED,
    TOTP_INVALID,
    INVALID_CREDENTIALS,
    TOTP_ALREADY_ENROLLED,
    TOTP_ENROLMENT_EXPIRED,
    TOTP_NOT_ENROLLED,
    TOTP_ENROLMENT_REQUIRED,
    CLINICAL_CRYPTO_UNAVAILABLE,
    PATIENT_REF_DISABLED,
    STALE_VERSION,
    CONTENT_CHANGED,
    REAUTHENTICATION_FAILED,
    COSIGNER_MUST_DIFFER,
    SIGNER_PROFILE_INCOMPLETE,
    EXPORT_EXPIRED,
    EXPORT_TOO_LARGE,
    CONSENT_PURPOSE_MISMATCH,
    HOST_KEY_MISMATCH,
    CLINICAL_ROLE_NOT_ASSIGNABLE,
    TOTP_RESET_FORBIDDEN,
    FOUR_EYES_REQUIRED,
    INVALID_TRANSITION,
    CONFIG_VERSION_EXISTS,
    CONFIG_INVALID,
    CONFIG_NOT_PUBLISHED,
    BUNDLE_SIGNING_UNAVAILABLE,
    ENCOUNTER_CLOSED,
    ENCOUNTER_SUMMARY_FORBIDDEN,
    ATTACHMENT_TOO_LARGE,
    ATTACHMENT_TYPE_NOT_ALLOWED,
    DIGEST_MISMATCH,
    SHARE_LINK_EXPIRED,
    SHARE_LINK_EXHAUSTED,
    SHARE_PASSCODE_INVALID,
    CASE_CONSENT_MISSING,
    IDENTITY_DISABLED,
    EXPERIMENT_NAME_EXISTS,
    EXPERIMENT_PROMOTED,
    EXPERIMENT_NO_RUN,
    COHORT_EMPTY,
    COHORT_TOO_LARGE,
    SWEEP_TOO_LARGE,
    ANALYSIS_QUOTA_EXCEEDED,
    ANALYSIS_LEVEL_UNAVAILABLE,
    FRAMES_NOT_AVAILABLE,
    FRAMES_PURGED,
    RESEARCH_CONSENT_REQUIRED,
    DIGEST_REQUIRED,
    CONFIG_UNANALYSED,
    JOB_FINISHED,
    RESULT_TOO_LARGE,
    NOT_IMPLEMENTED,
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
