//! owner: WP-G.12 (session schema 1.8, owner decision 2026-11-01)
//!
//! Session ingest contract v1.8 (SCH-5 of `docs/spec/modules/asd-gaze-robustness/technical.md`):
//! `POST /v1/sessions`.
//!
//! 1.8 is 1.7 plus an explicit scoring outcome, so a session whose on-device scoring failed is
//! sent without invented numbers:
//! - a required [`ScoringStatus`] (`scoringStatus`, top level and in `rawPayload`, equal);
//! - an optional [`UnscorableReason`] (`unscorableReason`, top level and in `rawPayload`),
//!   only on an `unscorable` session;
//! - the five score fields [`SCORE_FIELDS`] (`socialPreferenceScore` at the top level and in
//!   `rawPayload`; `socialSmileIntensity`, `smileReactivityIndex`, `flatAffectScore`,
//!   `surpriseScoreMean` in `rawPayload`) become optional: required numbers when `scored`,
//!   absent or `null` when `unscorable` (CON-14: never a placeholder);
//! - an `unscorable` session carries no device screening flag (`attentionFlag` absent or
//!   `null`); the backend stores it as `unreliable`.
//!
//! The conditional rules are in the exported schema (`allOf` / `if` / `then` / `else`) and in
//! [`scoring_violations`], which the backend and `asd-sync` run on the typed request.
//!
//! Not additive: 1.8 requires `scoringStatus`, so a 1.7 body is not a 1.8 body. A 1.7
//! request maps onto 1.8 losslessly as `scored` ([`upgrade_v1_7`]). No casing change, no
//! field gains a default. Every other shape (per-clip rows, tracking quality, frames file)
//! is the 1.7 one, reused.

use std::collections::BTreeMap;

use asd_core::types::{AttentionFlag, CaptureSource, GazeQualityVerdict};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use crate::session_v1_6::{
    DwellUnits, SessionIngestResponse, SessionIngestStatus, IDEMPOTENCY_KEY_HEADER,
    SCHEMA_VERSION_HEADER,
};
pub use crate::session_v1_7::{
    calibrated_distance_cm, ClipTracking, FrameTracking, PerVideoReport, SessionPerVideoDetails,
    SessionPerVideoRow, TrackState, TrackingQuality,
};

use crate::mirror;
use crate::session_v1_6::explicit_null;
use crate::session_v1_7;

/// Wire schema version (`schemaVersion` and `X-Schema-Version`).
pub const SCHEMA_VERSION: &str = "1.8";
/// Record form of the version (`rawPayload.schemaVersion`).
pub const RECORD_SCHEMA_VERSION: &str = "v1.8";
/// `$id` of the exported schema document.
pub const SCHEMA_ID: &str = "https://schemas.curv.org/asd/session-v1.8.json";

/// The `rawPayload` keys that are required numbers when `scored` and absent or `null` when
/// `unscorable` (the top-level `socialPreferenceScore` follows the same rule).
pub const SCORE_FIELDS: [&str; 5] = [
    "socialPreferenceScore",
    "socialSmileIntensity",
    "smileReactivityIndex",
    "flatAffectScore",
    "surpriseScoreMean",
];

/// Whether on-device scoring produced the session's scores.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ScoringStatus {
    // Every score field is a measured number. (Plain comments: a doc comment on a variant
    // turns the exported enum into a `oneOf`.)
    Scored,
    // Scoring did not run or was refused; no score field carries a value.
    Unscorable,
}

impl ScoringStatus {
    /// Wire spelling (`scored`, `unscorable`), also the stored column value.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Scored => "scored",
            Self::Unscorable => "unscorable",
        }
    }
}

/// Why a session is `unscorable` (optional; absent when the device does not know).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum UnscorableReason {
    // The core refused the session aggregation (e.g. a non-finite input).
    AggregationFailed,
    // Too few valid gaze samples to compute a score.
    InsufficientValidGaze,
    // Tracking quality verdict `unusable`.
    TrackingUnusable,
    // Calibration failed and no score may be computed without it.
    CalibrationFailed,
    // No clip of the protocol could be scored.
    NoScorableClips,
    // Anything else.
    Other,
}

impl UnscorableReason {
    /// Every value, in declaration order.
    pub const ALL: [UnscorableReason; 6] = [
        Self::AggregationFailed,
        Self::InsufficientValidGaze,
        Self::TrackingUnusable,
        Self::CalibrationFailed,
        Self::NoScorableClips,
        Self::Other,
    ];

    /// Wire spelling, also the stored column value.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AggregationFailed => "aggregation_failed",
            Self::InsufficientValidGaze => "insufficient_valid_gaze",
            Self::TrackingUnusable => "tracking_unusable",
            Self::CalibrationFailed => "calibration_failed",
            Self::NoScorableClips => "no_scorable_clips",
            Self::Other => "other",
        }
    }
}

/// The full on-device record, carried as `rawPayload` (1.8). The 1.7 record with the five
/// score fields optional and the scoring outcome added.
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
    /// Equal to the top-level `scoringStatus`.
    pub scoring_status: ScoringStatus,
    /// Equal to the top-level `unscorableReason`; only on an `unscorable` record.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unscorable_reason: Option<UnscorableReason>,
    /// Required when `scored`; absent or `null` when `unscorable`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub social_preference_score: Option<f64>,
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
    /// QS-5: the calibration pose distance when a calibration ran; absent otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calibrated_distance_cm: Option<f64>,
    #[schemars(with = "mirror::GazeQualityVerdict")]
    pub quality_verdict: GazeQualityVerdict,
    /// Required when `scored`; absent or `null` when `unscorable`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub social_smile_intensity: Option<f64>,
    /// Required when `scored`; absent or `null` when `unscorable`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub smile_reactivity_index: Option<f64>,
    /// Required when `scored`; absent or `null` when `unscorable`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flat_affect_score: Option<f64>,
    /// Required when `scored`; absent or `null` when `unscorable`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surprise_score_mean: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    /// Record form (`"v1.8"`); the wire form drops the `v`.
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

/// `POST /v1/sessions` body, schema 1.8.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(
    title = "SessionIngestRequest",
    description = "Curv Pediatric ASD Gaze Session Ingestion Schema (v1.8)"
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
    /// `scored`: every score field is a number. `unscorable`: none is (absent or `null`),
    /// and `attentionFlag` is absent or `null`.
    pub scoring_status: ScoringStatus,
    /// Only on an `unscorable` session; optional there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unscorable_reason: Option<UnscorableReason>,
    /// Required when `scored`; absent or `null` when `unscorable`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub social_preference_score: Option<f64>,
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
    /// Wire form (`"1.8"`), equal to the `X-Schema-Version` header (CON-21).
    pub schema_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub per_video: Option<Vec<SessionPerVideoRow>>,
    pub raw_payload: GazeSessionRecord,
}

impl SessionIngestRequest {
    /// `true` for an `unscorable` session.
    pub fn is_unscorable(&self) -> bool {
        self.scoring_status == ScoringStatus::Unscorable
    }
}

/// One broken scoring rule: the JSON pointer of the offending field and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScoringViolation {
    pub pointer: String,
    pub reason: &'static str,
}

/// The 1.8 scoring rules the typed request must satisfy (the schema states the same rules
/// for the JSON form; this is the one implementation for typed values):
/// - `rawPayload.scoringStatus` / `unscorableReason` equal the top-level ones;
/// - `scored`: no `unscorableReason`, every score field present and finite;
/// - `unscorable`: no score field and no device `attentionFlag`.
///
/// Empty when the request is consistent.
pub fn scoring_violations(req: &SessionIngestRequest) -> Vec<ScoringViolation> {
    let mut out = Vec::new();
    let mut push = |pointer: &str, reason: &'static str| {
        out.push(ScoringViolation {
            pointer: pointer.into(),
            reason,
        })
    };
    let raw = &req.raw_payload;
    if raw.scoring_status != req.scoring_status {
        push("/rawPayload/scoringStatus", "must equal scoringStatus");
    }
    if raw.unscorable_reason != req.unscorable_reason {
        push(
            "/rawPayload/unscorableReason",
            "must equal unscorableReason",
        );
    }
    let scores: [(&str, Option<f64>); 6] = [
        ("/socialPreferenceScore", req.social_preference_score),
        (
            "/rawPayload/socialPreferenceScore",
            raw.social_preference_score,
        ),
        (
            "/rawPayload/socialSmileIntensity",
            raw.social_smile_intensity,
        ),
        (
            "/rawPayload/smileReactivityIndex",
            raw.smile_reactivity_index,
        ),
        ("/rawPayload/flatAffectScore", raw.flat_affect_score),
        ("/rawPayload/surpriseScoreMean", raw.surprise_score_mean),
    ];
    match req.scoring_status {
        ScoringStatus::Scored => {
            if req.unscorable_reason.is_some() {
                push(
                    "/unscorableReason",
                    "only allowed when scoringStatus is unscorable",
                );
            }
            for (pointer, value) in scores {
                match value {
                    None => push(pointer, "required when scoringStatus is scored"),
                    Some(v) if !v.is_finite() => push(pointer, "must be a finite number"),
                    Some(_) => {}
                }
            }
        }
        ScoringStatus::Unscorable => {
            for (pointer, value) in scores {
                if value.is_some() {
                    push(
                        pointer,
                        "must be absent or null when scoringStatus is unscorable",
                    );
                }
            }
            if matches!(req.attention_flag, Some(Some(_))) {
                push(
                    "/attentionFlag",
                    "must be absent or null when scoringStatus is unscorable",
                );
            }
        }
    }
    out
}

/// The JSON Schema form of the scoring rules for one object: `if scoringStatus = scored then
/// the fields are numbers and required (and no unscorableReason) else they are null`. Added
/// to the exported schema by `schema::session_v1_8`.
pub(crate) fn scoring_rule_schema(score_fields: &[&str]) -> Value {
    let numbers: serde_json::Map<String, Value> = score_fields
        .iter()
        .map(|f| ((*f).to_string(), serde_json::json!({"type": "number"})))
        .chain(std::iter::once((
            "unscorableReason".to_string(),
            serde_json::json!({"type": "null"}),
        )))
        .collect();
    let nulls: serde_json::Map<String, Value> = score_fields
        .iter()
        .map(|f| ((*f).to_string(), serde_json::json!({"type": "null"})))
        .collect();
    serde_json::json!({
        "if": {
            "properties": {"scoringStatus": {"const": "scored"}},
            "required": ["scoringStatus"]
        },
        "then": {"required": score_fields, "properties": numbers},
        "else": {"properties": nulls}
    })
}

/// A 1.7 request as the 1.8 type, without loss: `scored`, every 1.7 score number carried as
/// `Some`, no reason, the record version unchanged (`v1.7`; `schemaVersion` still says which
/// version was sent). Exhaustive destructuring: a field added to either version fails to
/// compile here.
pub fn upgrade_v1_7(req: session_v1_7::SessionIngestRequest) -> SessionIngestRequest {
    let session_v1_7::SessionIngestRequest {
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
    let session_v1_7::GazeSessionRecord {
        id,
        session_id: record_session_id,
        participant_id,
        capture_source: record_capture_source,
        started_at,
        ended_at,
        duration_sec: record_duration_sec,
        social_preference_score: record_social_preference_score,
        valid_frame_ratio,
        eye_dwell_ratio: record_eye_dwell_ratio,
        mouth_dwell_ratio: record_mouth_dwell_ratio,
        first_eye_latency_ms: record_first_eye_latency_ms,
        calibrated_distance_cm: record_calibrated_distance_cm,
        quality_verdict: record_quality_verdict,
        social_smile_intensity,
        smile_reactivity_index,
        flat_affect_score,
        surprise_score_mean,
        notes,
        schema_version: record_schema_version,
        zero_raw_video_enforced,
        legacy_provenance,
        per_video_composite,
        per_video: record_per_video,
        tracking_quality,
        extra,
    } = raw_payload;
    SessionIngestRequest {
        session_id,
        participant_token,
        capture_source,
        duration_sec,
        valid_ratio,
        scoring_status: ScoringStatus::Scored,
        unscorable_reason: None,
        social_preference_score: Some(social_preference_score),
        eye_dwell_ratio,
        mouth_dwell_ratio,
        first_eye_latency_ms,
        quality_verdict,
        calibrated_distance_cm,
        attention_flag,
        battery,
        schema_version,
        per_video,
        raw_payload: GazeSessionRecord {
            id,
            session_id: record_session_id,
            participant_id,
            capture_source: record_capture_source,
            started_at,
            ended_at,
            duration_sec: record_duration_sec,
            scoring_status: ScoringStatus::Scored,
            unscorable_reason: None,
            social_preference_score: Some(record_social_preference_score),
            valid_frame_ratio,
            eye_dwell_ratio: record_eye_dwell_ratio,
            mouth_dwell_ratio: record_mouth_dwell_ratio,
            first_eye_latency_ms: record_first_eye_latency_ms,
            calibrated_distance_cm: record_calibrated_distance_cm,
            quality_verdict: record_quality_verdict,
            social_smile_intensity: Some(social_smile_intensity),
            smile_reactivity_index: Some(smile_reactivity_index),
            flat_affect_score: Some(flat_affect_score),
            surprise_score_mean: Some(surprise_score_mean),
            notes,
            schema_version: record_schema_version,
            zero_raw_video_enforced,
            legacy_provenance,
            per_video_composite,
            per_video: record_per_video,
            tracking_quality,
            extra,
        },
    }
}
