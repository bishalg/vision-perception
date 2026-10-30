//! owner: WP-R.3
//!
//! Session ingest contract v1.6 (CON-2, CON-12, CON-13, CON-14): `POST /v1/sessions`.
//!
//! The types match what `SessionSyncClient.swift` encodes, key for key:
//! - top level and `rawPayload` are camelCase; `perVideo[]` rows are snake_case and their
//!   `details.rows[]` are camelCase `PerVideoScoreRow`s (trap 4: two different structs);
//! - `attentionFlag` and `battery` are always written, as explicit `null` when unknown;
//!   absent on decode is tolerated (`Option<Option<T>>`, trap 3);
//! - unmeasured metrics (`eyeDwellRatio`, `dwell_mouth`, ...) are absent keys, never `null`
//!   on encode and never defaulted on decode;
//! - dwell values are ratios in `[0, 1]` (`details.dwellUnits == "ratio"`);
//! - no field has a default value (CON-14): a missing required field is a decode error.
//!
//! 1.6 keeps the mixed casing; normalising it is a 1.7 change (CON-12).

use std::collections::BTreeMap;

use asd_core::types::{
    AttentionFlag, CaptureSource, ExpectationStatus, GazeQualityVerdict, PerVideoScoreReport,
    PerVideoScoreRow,
};
use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use crate::mirror;

/// Wire schema version (`schemaVersion` and `X-Schema-Version`).
pub const SCHEMA_VERSION: &str = "1.6";
/// `$id` of the exported schema document.
pub const SCHEMA_ID: &str = "https://schemas.curv.org/asd/session-v1.6.json";
/// Header carrying the wire schema version (CON-21).
pub const SCHEMA_VERSION_HEADER: &str = "X-Schema-Version";
/// Header carrying the idempotency key (= `sessionId` for sessions, ADR-B5).
pub const IDEMPOTENCY_KEY_HEADER: &str = "X-Idempotency-Key";

/// Deserialises a present value (including `null`) as `Some(..)`; combined with
/// `#[serde(default)]` an absent key stays `None`. Serialising `Some(None)` writes `null`.
pub(crate) fn explicit_null<'de, D, T>(d: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(d).map(Some)
}

/// Units of the `dwell_*` columns. Ratios of valid clip samples, never seconds (CON-13).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum DwellUnits {
    Ratio,
}

/// `SessionPerVideoRow.Details` (camelCase).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(rename = "SessionPerVideoDetails")]
pub struct SessionPerVideoDetails {
    pub title: String,
    pub category: String,
    pub subscore: f64,
    #[schemars(range(min = 0.0, max = 1.0))]
    pub valid_sample_ratio: f64,
    pub low_confidence: bool,
    pub has_baseline: bool,
    pub dwell_units: DwellUnits,
    #[schemars(with = "Vec<mirror::PerVideoScoreRow>")]
    pub rows: Vec<PerVideoScoreRow>,
}

/// One flattened per-clip row (`SessionPerVideoRow` in Swift; schema name
/// `SessionPerVideoScore`, which the Swift schema test looks up by name). snake_case.
///
/// A metric the clip did not measure, or measured only by imputation, is an absent key.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "SessionPerVideoScore")]
pub struct SessionPerVideoRow {
    pub clip_id: String,
    /// 1-based play order.
    pub order: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0.0, max = 1.0))]
    pub dwell_social: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0.0, max = 1.0))]
    pub dwell_geometric: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0.0, max = 1.0))]
    pub dwell_eye: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0.0, max = 1.0))]
    pub dwell_mouth: Option<f64>,
    /// Measured first-entry latency; an imputed latency is not sent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub orienting_latency_ms: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fixation_count: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mean_fixation_ms: Option<f64>,
    /// Worst row status of the clip; absent when the clip has no scored rows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<mirror::ExpectationStatus>")]
    pub status: Option<ExpectationStatus>,
    pub score: f64,
    pub weight: f64,
    pub details: SessionPerVideoDetails,
}

/// The full on-device record (`GazeSessionRecord.swift`), carried as `rawPayload`.
///
/// Dates are RFC 3339 UTC strings with milliseconds (`2026-10-26T07:33:20.000Z`), as
/// `SessionPayloadCoding` writes them. Keys this type does not know (for example a future
/// `bundleVersion` or `normCollection`) are kept in [`GazeSessionRecord::extra`] and written
/// back unchanged, so `sessions.summary` stores the record without loss.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GazeSessionRecord {
    pub id: String,
    pub session_id: String,
    /// Pseudonymous participant token, never a name (CNS-7).
    pub participant_id: String,
    #[schemars(with = "mirror::CaptureSource")]
    pub capture_source: CaptureSource,
    #[schemars(extend("format" = "date-time"))]
    pub started_at: String,
    #[schemars(extend("format" = "date-time"))]
    pub ended_at: String,
    pub duration_sec: f64,
    pub social_preference_score: f64,
    #[schemars(range(min = 0.0, max = 1.0))]
    pub valid_frame_ratio: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0.0, max = 1.0))]
    pub eye_dwell_ratio: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0.0, max = 1.0))]
    pub mouth_dwell_ratio: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_eye_latency_ms: Option<f64>,
    pub calibrated_distance_cm: f64,
    #[schemars(with = "mirror::GazeQualityVerdict")]
    pub quality_verdict: GazeQualityVerdict,
    pub social_smile_intensity: f64,
    pub smile_reactivity_index: f64,
    pub flat_affect_score: f64,
    pub surprise_score_mean: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    /// Record form (`"v1.6"`); the wire form drops the `v`.
    pub schema_version: String,
    pub zero_raw_video_enforced: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legacy_provenance: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub per_video_composite: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<Vec<mirror::PerVideoScoreReport>>")]
    pub per_video: Option<Vec<PerVideoScoreReport>>,
    /// Keys not modelled above, preserved verbatim.
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// `POST /v1/sessions` body (`SessionIngestPayload` in Swift).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(
    title = "SessionIngestRequest",
    description = "Curv Pediatric ASD Gaze Session Ingestion Schema (v1.6)"
)]
pub struct SessionIngestRequest {
    pub session_id: String,
    /// Pseudonymous token (CNS-7), never a name.
    pub participant_token: String,
    #[schemars(with = "mirror::CaptureSource")]
    pub capture_source: CaptureSource,
    pub duration_sec: f64,
    #[schemars(range(min = 0.0, max = 1.0))]
    pub valid_ratio: f64,
    pub social_preference_score: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0.0, max = 1.0))]
    pub eye_dwell_ratio: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0.0, max = 1.0))]
    pub mouth_dwell_ratio: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_eye_latency_ms: Option<f64>,
    #[schemars(with = "mirror::GazeQualityVerdict")]
    pub quality_verdict: GazeQualityVerdict,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calibrated_distance_cm: Option<f64>,
    /// Explicit `null` when the device did not compute a flag; the server then derives it
    /// (CON-15). `None` = key absent, `Some(None)` = `null`.
    #[serde(
        default,
        deserialize_with = "explicit_null",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "Option<mirror::AttentionFlag>")]
    pub attention_flag: Option<Option<AttentionFlag>>,
    /// Catalog protocol id (`express_30s`, CON-16) or explicit `null` when unknown.
    #[serde(
        default,
        deserialize_with = "explicit_null",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "Option<String>")]
    pub battery: Option<Option<String>>,
    /// Wire form (`"1.6"`), equal to the `X-Schema-Version` header (CON-21).
    pub schema_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub per_video: Option<Vec<SessionPerVideoRow>>,
    pub raw_payload: GazeSessionRecord,
}

/// Outcome recorded for an accepted session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SessionIngestStatus {
    Accepted,
}

/// `POST /v1/sessions` success body. Key casing follows the iOS decoder
/// (`sessionId`, `frames_upload_url`). A replay returns the original body with
/// `duplicate: true` (CON-17). Upload URLs are absolute pre-signed URLs or relative
/// `/v1/sessions/{id}/blobs/{frames|report}` paths (CON-18); `null` when no upload is wanted.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(example = example_ingest_response())]
pub struct SessionIngestResponse {
    #[serde(rename = "sessionId")]
    pub session_id: String,
    pub status: SessionIngestStatus,
    pub duplicate: bool,
    pub frames_upload_url: Option<String>,
    pub report_upload_url: Option<String>,
}

fn example_ingest_response() -> SessionIngestResponse {
    SessionIngestResponse {
        session_id: "SESS-GOLDEN-0001".into(),
        status: SessionIngestStatus::Accepted,
        duplicate: false,
        frames_upload_url: Some("/v1/sessions/SESS-GOLDEN-0001/blobs/frames".into()),
        report_upload_url: None,
    }
}
