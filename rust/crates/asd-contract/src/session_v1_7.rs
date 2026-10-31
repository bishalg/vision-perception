//! owner: WP-G.6
//!
//! Session ingest contract v1.7 (SCH-1, QS-1, QS-3, QS-5 of
//! `docs/spec/modules/asd-gaze-robustness/technical.md`; ADR-0007 D10): `POST /v1/sessions`.
//!
//! 1.7 is 1.6 plus, all optional (additive only, so every 1.6 payload decodes here):
//! - typed `rawPayload.trackingQuality`: the asd-core [`TrackingQuality`] (QS-1), re-exported,
//!   never copied;
//! - the QS-3 per-clip fields ([`ClipTracking`]) inside `perVideo[].details` and inside
//!   `rawPayload.perVideo[]`;
//! - `rawPayload.calibratedDistanceCm` becomes optional: the measured calibration distance
//!   ([`calibrated_distance_cm`], QS-5), absent when no calibration ran;
//! - per-frame `track_state` and `gap_filled` in the frames file sample ([`FrameTracking`];
//!   snake_case like the other sample keys, trap 5).
//!
//! No casing change (CON-12 stays deferred), no field gains a default (CON-14). The 1.6
//! structs are reused where the shape is unchanged (`SessionPerVideoDetails`, the response,
//! the headers); the containers that carry a new field are 1.7 types of the same name.
//!
//! Rollout (SCH-2): 1.6 stays the wire version of current clients (`asd-sync` sends and the
//! backend accepts exactly 1.6 until WP-G.11 makes the backend accept {1.6, 1.7}); the iOS
//! writer moves to 1.7 only in WP-G.9. `session-v1.6.json` stays exported and checked.

use std::collections::BTreeMap;

use asd_core::types::{
    AttentionFlag, CaptureSource, ExpectationStatus, GazeQualityVerdict, PerVideoScoreReport,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use asd_core::types::tracking::{TrackState, TrackingQuality};

pub use crate::session_v1_6::{
    DwellUnits, SessionIngestResponse, SessionIngestStatus, IDEMPOTENCY_KEY_HEADER,
    SCHEMA_VERSION_HEADER,
};

use crate::mirror;
use crate::session_v1_6::{self, explicit_null};

/// Wire schema version (`schemaVersion` and `X-Schema-Version`).
pub const SCHEMA_VERSION: &str = "1.7";
/// Record form of the version (`rawPayload.schemaVersion`).
pub const RECORD_SCHEMA_VERSION: &str = "v1.7";
/// `$id` of the exported schema document.
pub const SCHEMA_ID: &str = "https://schemas.curv.org/asd/session-v1.7.json";

/// QS-5: `calibratedDistanceCm` = `trackingQuality.calibration.pose.distanceCm` when a
/// calibration ran in the session; `None` otherwise (skipped, or no pose was measured).
pub fn calibrated_distance_cm(tracking_quality: Option<&TrackingQuality>) -> Option<f64> {
    use asd_core::types::tracking::CalibrationQuality;
    let calibration = &tracking_quality?.calibration;
    if calibration.quality == CalibrationQuality::Skipped {
        return None;
    }
    calibration.pose.map(|p| p.distance_cm)
}

/// QS-3 per-clip tracking fields (camelCase), in `perVideo[].details` and in
/// `rawPayload.perVideo[]`. Each is an absent key when not measured.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ClipTracking {
    /// Mean CNF-1 confidence over the clip's samples.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0.0, max = 1.0))]
    pub mean_sample_confidence: Option<f64>,
    /// Mean confidence of the samples inside fixations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0.0, max = 1.0))]
    pub mean_fixation_confidence: Option<f64>,
    /// Offset of the drift check right after the clip, degrees.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nearest_check_offset_deg: Option<f64>,
    /// Smallest dimension of the clip's role-bearing AOIs, degrees at the session's median
    /// distance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aoi_min_size_deg: Option<f64>,
}

/// Per-frame tracking keys of the frames file sample (SCH-1, TRK-1, GAP-1), snake_case like
/// the other `gaze` sample keys. Absent on frames written by a 1.6 client; the 1.6 frame
/// reader (`asd_core::types::GazeFrame`) ignores them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrameTracking {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub track_state: Option<TrackState>,
    /// `true` for a sample produced by gap filling (GAP-1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gap_filled: Option<bool>,
}

/// `perVideo[].details` in 1.7: the 1.6 details plus [`ClipTracking`] (flattened, camelCase).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "SessionPerVideoDetails")]
pub struct SessionPerVideoDetails {
    #[serde(flatten)]
    pub base: session_v1_6::SessionPerVideoDetails,
    #[serde(flatten)]
    pub tracking: ClipTracking,
}

/// One flattened per-clip row (schema name `SessionPerVideoScore`, as in 1.6). snake_case;
/// identical to the 1.6 row except that `details` carries the QS-3 fields.
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

/// `rawPayload.perVideo[]` in 1.7: the asd-core `PerVideoScoreReport` plus [`ClipTracking`]
/// (both flattened, camelCase).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "PerVideoScoreReport")]
pub struct PerVideoReport {
    #[serde(flatten)]
    #[schemars(with = "mirror::PerVideoScoreReport")]
    pub report: PerVideoScoreReport,
    #[serde(flatten)]
    pub tracking: ClipTracking,
}

/// The full on-device record, carried as `rawPayload` (1.7). Same rules as the 1.6 record
/// (RFC 3339 dates, unknown keys kept in [`GazeSessionRecord::extra`]).
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
    /// QS-5: the calibration pose distance when a calibration ran; absent otherwise
    /// (required in 1.6).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calibrated_distance_cm: Option<f64>,
    #[schemars(with = "mirror::GazeQualityVerdict")]
    pub quality_verdict: GazeQualityVerdict,
    pub social_smile_intensity: f64,
    pub smile_reactivity_index: f64,
    pub flat_affect_score: f64,
    pub surprise_score_mean: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    /// Record form (`"v1.7"`); the wire form drops the `v`.
    pub schema_version: String,
    pub zero_raw_video_enforced: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legacy_provenance: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub per_video_composite: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub per_video: Option<Vec<PerVideoReport>>,
    /// Session tracking-quality record (QS-1); absent when the gaze pipeline did not run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<mirror::TrackingQuality>")]
    pub tracking_quality: Option<TrackingQuality>,
    /// Keys not modelled above, preserved verbatim.
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// `POST /v1/sessions` body, schema 1.7.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(
    title = "SessionIngestRequest",
    description = "Curv Pediatric ASD Gaze Session Ingestion Schema (v1.7)"
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
    /// Explicit `null` when the device did not compute a flag (CON-15).
    #[serde(
        default,
        deserialize_with = "explicit_null",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "Option<mirror::AttentionFlag>")]
    pub attention_flag: Option<Option<AttentionFlag>>,
    /// Catalog protocol id or explicit `null` when unknown.
    #[serde(
        default,
        deserialize_with = "explicit_null",
        skip_serializing_if = "Option::is_none"
    )]
    #[schemars(with = "Option<String>")]
    pub battery: Option<Option<String>>,
    /// Wire form (`"1.7"`), equal to the `X-Schema-Version` header (CON-21).
    pub schema_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub per_video: Option<Vec<SessionPerVideoRow>>,
    pub raw_payload: GazeSessionRecord,
}
