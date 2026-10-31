//! owner: WP-B.6
//!
//! Device-plane care bodies (API-1): `POST /v1/care/events` (idempotent batch),
//! `GET /v1/participants/{token}/summary` and `POST /v1/participants/{token}/erasure`.
//! The semantics are the port of the removed `shims.py` CareKit sync (tasks, medications,
//! diary) plus `milestone`, carried in the `care_events` table. snake_case, timestamps
//! RFC 3339 UTC strings.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Most events accepted in one `POST /v1/care/events` batch.
pub const MAX_EVENTS_PER_BATCH: usize = 500;

/// Header carrying an optional batch-level idempotency key (a replay answers `200` with
/// `duplicate: true` and the stored counts).
pub const IDEMPOTENCY_KEY_HEADER: &str = "X-Idempotency-Key";

/// What a care event records (`care_events.kind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CareEventKind {
    Task,
    Medication,
    Diary,
    Milestone,
}

impl CareEventKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Task => "task",
            Self::Medication => "medication",
            Self::Diary => "diary",
            Self::Milestone => "milestone",
        }
    }
}

/// One care event of a batch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CareEventInput {
    pub kind: CareEventKind,
    /// When it happened (RFC 3339); the server's receive time when absent.
    #[serde(default)]
    pub occurred_at: Option<String>,
    /// Free-form event body. Tasks carry `task_id` and optional `metadata.type`
    /// (`session`, `checkin`); diary entries carry `mood`. Must be a JSON object.
    #[serde(default = "empty_object")]
    pub payload: Value,
    /// Per-event key, unique per org: an event whose key was stored before is counted as a
    /// duplicate and not stored again.
    #[serde(default)]
    pub idempotency_key: Option<String>,
}

fn empty_object() -> Value {
    Value::Object(serde_json::Map::new())
}

/// `POST /v1/care/events` body.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CareEventsRequest {
    pub participant_token: String,
    /// 1 to [`MAX_EVENTS_PER_BATCH`] events.
    pub events: Vec<CareEventInput>,
}

/// `POST /v1/care/events` response (`201` new, `200` replay).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CareEventsResponse {
    pub participant_token: String,
    /// Events stored by this call.
    pub accepted: u32,
    /// Events skipped because their `idempotency_key` was already stored.
    pub duplicates: u32,
    /// True when the whole batch was a replay of an earlier `X-Idempotency-Key`.
    pub duplicate: bool,
}

/// `GET /v1/participants/{token}/summary`: counts over the participant's care events. An
/// unknown token answers all zeros (the old CareKit summary did the same).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CareSummary {
    pub participant_token: String,
    pub total_events: u32,
    pub task_completions: u32,
    pub medication_doses: u32,
    pub diary_entries: u32,
    pub milestones: u32,
    /// Task events whose `task_id` contains `session` or whose `metadata.type` is `session`.
    pub session_count: u32,
    /// Same for `checkin`.
    pub checkin_count: u32,
    /// 100 when any task was completed, else 0 (the CareKit port's rule).
    pub completion_rate_percent: f64,
    /// Distinct diary moods, newest first (at most 10).
    pub recent_moods: Vec<String>,
}

/// `POST /v1/participants/{token}/erasure`: the stored request (execution is a job,
/// WP-B.9). Consent is withdrawn at once, so no new data is accepted for the participant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ErasureRequestResponse {
    pub id: String,
    pub participant_token: String,
    pub scope: String,
    pub requested_at: String,
    /// `null` until the erasure job ran.
    pub completed_at: Option<String>,
    /// True when an open request already existed and is returned unchanged.
    pub duplicate: bool,
}
