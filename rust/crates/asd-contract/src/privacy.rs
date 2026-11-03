//! owner: WP-B.9
//!
//! Erasure execution and the retention sweep (ADR-B8; 02 §5 "retention sweep (nightly:
//! delete frames blobs beyond retention, keep aggregate counts), erasure execution"):
//!
//! - `GET /v1/orgs/{id}/erasure-requests` ([`ErasureRequestPage`]) and
//!   `POST /v1/orgs/{id}/participants/{participant_id}/erasure` (an admin files a request for
//!   a participant; [`ErasureRequest`]). The device route `POST /v1/participants/{token}/erasure`
//!   (WP-B.6, `care::ErasureRequestResponse`) files the same request.
//! - `POST /v1/orgs/{id}/retention-sweep` ([`RetentionSweepRequest`] -> [`RetentionSweepJob`]);
//!   the job's `result` is a [`RetentionSweepResult`] (`GET /v1/jobs/{id}`).
//!
//! Conventions as `api_v1`: snake_case, UUID strings, RFC 3339 UTC timestamps.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// `erasure_requests.status`.
///
/// - `pending`: filed, the erasure job has not finished;
/// - `completed`: everything about the participant was erased; the participant row stays as
///   a tombstone (`erased_at`, withdrawn, demographics cleared) so replayed uploads stay
///   refused;
/// - `held`: the participant has signed clinical records (signed or superseded review
///   versions, clinical notes, finalised letters). Everything else was erased; the held
///   records, the sessions they document and the MRN mapping stay until the owner decides
///   ADR-0008 Q2 (retention versus erasure of clinical records).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ErasureStatus {
    Pending,
    Completed,
    Held,
}

impl ErasureStatus {
    pub const ALL: &'static [ErasureStatus] = &[Self::Pending, Self::Completed, Self::Held];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Completed => "completed",
            Self::Held => "held",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|v| v.as_str() == s)
    }
}

/// `erasure_requests.hold_reason` when `status = held`.
pub const HOLD_CLINICAL_RECORD: &str = "clinical_record";

/// What one erasure removed (`erasure_requests.outcome`). Counts only, never identifiers.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ErasureOutcome {
    /// Sessions deleted with their clips, review-queue items and labels.
    pub sessions_deleted: u64,
    /// Sessions kept because a held clinical record documents them.
    pub sessions_held: u64,
    pub care_events_deleted: u64,
    /// Frames and report blobs deleted (absent keys count too: the store cannot tell).
    pub session_blobs_deleted: u64,
    /// Unexpired export artifacts that included the participant, deleted (EXJ-8).
    pub export_artifacts_deleted: u64,
    /// Unsigned clinical drafts (review drafts, draft letters) deleted.
    pub clinical_drafts_deleted: u64,
    /// The MRN mapping was deleted.
    pub patient_ref_deleted: bool,
    /// Attachments deleted with their blobs (REC-7). Absent in outcomes recorded before
    /// EMR-lite.
    #[serde(default)]
    pub attachments_deleted: u64,
}

/// One erasure request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ErasureRequest {
    pub id: String,
    pub org_id: String,
    pub participant_id: String,
    pub participant_token: String,
    pub status: ErasureStatus,
    /// Always `all` today.
    pub scope: String,
    /// `device | user | system`.
    pub requested_by_type: String,
    #[schemars(extend("format" = "date-time"))]
    pub requested_at: String,
    /// When the erasure job last ran for this request.
    #[schemars(extend("format" = "date-time"))]
    pub executed_at: Option<String>,
    #[schemars(extend("format" = "date-time"))]
    pub completed_at: Option<String>,
    /// [`HOLD_CLINICAL_RECORD`] when `held`.
    pub hold_reason: Option<String>,
    pub outcome: Option<ErasureOutcome>,
    /// True when an open request already existed and is returned unchanged.
    #[serde(default)]
    pub duplicate: bool,
}

/// `GET /v1/orgs/{id}/erasure-requests` (`ParticipantErase`), newest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ErasureRequestPage {
    pub items: Vec<ErasureRequest>,
    pub next_cursor: Option<String>,
}

/// `POST /v1/orgs/{id}/retention-sweep` (`ParticipantErase`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RetentionSweepRequest {
    /// Count what the sweep would delete without deleting anything (default `true`).
    #[serde(default = "default_true")]
    pub dry_run: bool,
}

fn default_true() -> bool {
    true
}

/// `202` of `POST /v1/orgs/{id}/retention-sweep`: the queued job (`GET /v1/jobs/{id}`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RetentionSweepJob {
    pub job_id: String,
    pub org_id: String,
    pub dry_run: bool,
    /// `queued` when created.
    pub status: String,
    #[schemars(extend("format" = "date-time"))]
    pub created_at: String,
}

/// One org in a [`RetentionSweepResult`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RetentionOrgResult {
    pub org_id: String,
    /// `organizations.retention_months`.
    pub retention_months: i32,
    /// Sessions received before this instant are past retention.
    #[schemars(extend("format" = "date-time"))]
    pub cutoff: String,
    /// Sessions whose frames blob was (or, in a dry run, would be) deleted.
    pub frames_purged: u64,
    /// Sessions whose research frame log was (or, in a dry run, would be) deleted (RFL-5).
    #[serde(default)]
    pub research_frames_purged: u64,
}

/// `jobs.result` of a `retention_sweep` job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RetentionSweepResult {
    pub dry_run: bool,
    pub orgs: Vec<RetentionOrgResult>,
}
