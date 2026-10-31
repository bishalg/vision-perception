//! owner: WP-R.3
//!
//! Schema-only mirrors of the `asd-core::types` that appear inside contract documents.
//!
//! `asd-core` stays free of `schemars` (WS-2 keeps it minimal), so the JSON Schema of each
//! embedded core type is described here and attached with `#[schemars(with = "...")]`. These
//! types are never constructed or serialised; serde attributes are present only so
//! `schemars` derives the same key names, optionality and enum values as the real types.
//!
//! Drift guard: `tests/config_defaults.rs`, `tests/session_v1_6.rs` and
//! `tests/session_v1_7.rs` validate real documents (bundled catalog and baselines, the iOS
//! golden payloads, the default scoring profile) against the schema with unknown keys
//! forbidden, and decode-then-validate checks fail when a mirror requires a key the real
//! type omits. When `asd-core` gains an optional `schemars` feature these mirrors can be
//! replaced by derives on the real types.
//!
//! The `TrackingQualityParams` mirrors (SCH-3) are the one place where a mirror also
//! derives `Serialize` and implements `Default`: their `Default` is the asd-core default
//! passed through JSON, so the per-field `default` keywords of the exported schema come from
//! the single asd-core source and cannot drift from it.

#![allow(dead_code)]

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, JsonSchema)]
pub(crate) enum CaptureSource {
    #[serde(rename = "truedepth")]
    TrueDepth,
    #[serde(rename = "simulated")]
    Simulated,
    #[serde(rename = "synthetic")]
    Synthetic,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum GazeQualityVerdict {
    Optimal,
    Acceptable,
    Degraded,
    Rejected,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum AttentionFlag {
    Green,
    Yellow,
    Red,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ExpectationStatus {
    OnTarget,
    Monitor,
    Atypical,
}

#[derive(Deserialize, JsonSchema)]
pub(crate) enum NormativeDirection {
    #[serde(rename = "higherBetter")]
    HigherBetter,
    #[serde(rename = "lowerBetter")]
    LowerBetter,
    #[serde(rename = "band")]
    Band,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PerVideoScoreRow {
    segment_id: String,
    segment_description: String,
    biomarker: String,
    #[serde(default, rename = "targetAOI")]
    target_aoi: Option<String>,
    metric_key: String,
    direction: NormativeDirection,
    observed: f64,
    #[serde(default)]
    expected: Option<f64>,
    #[serde(default)]
    acceptable_low: Option<f64>,
    #[serde(default)]
    acceptable_high: Option<f64>,
    status: ExpectationStatus,
    score: f64,
    imputed: bool,
    citation: String,
    source: String,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PerVideoScoreReport {
    stimulus_id: String,
    title: String,
    category: String,
    rows: Vec<PerVideoScoreRow>,
    subscore: f64,
    #[schemars(range(min = 0.0, max = 1.0))]
    valid_sample_ratio: f64,
    low_confidence: bool,
    has_baseline: bool,
    weight: f64,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NormativeRange {
    #[serde(default)]
    expected: Option<f64>,
    #[serde(default)]
    acceptable_low: Option<f64>,
    #[serde(default)]
    acceptable_high: Option<f64>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NormativeExpectation {
    biomarker: String,
    direction: NormativeDirection,
    #[serde(default, rename = "targetAOI")]
    target_aoi: Option<String>,
    /// Keyed by age-bracket id (`"18-23"`) or `"default"`.
    by_age: BTreeMap<String, NormativeRange>,
    citation: String,
    /// `"literature"` or `"interpolated"`.
    source: String,
    #[serde(default)]
    note: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NormativeSegment {
    id: String,
    description: String,
    start_sec: f64,
    end_sec: f64,
    expectations: Vec<NormativeExpectation>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NormativeVideo {
    title: String,
    category: String,
    weight: f64,
    segments: Vec<NormativeSegment>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AgeBracket {
    id: String,
    min_months: i64,
    max_months: i64,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NormativeBaselineSet {
    schema_version: String,
    status: String,
    #[serde(default)]
    provenance: Option<String>,
    age_brackets: Vec<AgeBracket>,
    videos: BTreeMap<String, NormativeVideo>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AgePack {
    id: String,
    min_months: i64,
    max_months: i64,
}

#[derive(Deserialize, JsonSchema)]
pub(crate) struct Membership {
    id: String,
    order: i64,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Area {
    name: String,
    start_sec: f64,
    end_sec: f64,
    x_min: f64,
    y_min: f64,
    x_max: f64,
    y_max: f64,
    roles: Vec<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Clip {
    id: String,
    title: String,
    category: String,
    duration_sec: f64,
    min_months: i64,
    max_months: i64,
    protocols: Vec<Membership>,
    areas: Vec<Area>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StimulusCatalog {
    schema_version: String,
    #[serde(default)]
    comment: Option<String>,
    languages: Vec<String>,
    fallback_language: String,
    age_packs: Vec<AgePack>,
    clips: Vec<Clip>,
}

// ------------------------------------------------------------------------------------------
// Gaze tracking (WP-G.6): `asd_core::types::tracking`
// ------------------------------------------------------------------------------------------

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Orientation {
    Portrait,
    PortraitUpsideDown,
    LandscapeLeft,
    LandscapeRight,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum GeometrySource {
    Table,
    IdiomDefault,
}

#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum CalModelKind {
    ScreenAffine,
    AngularAffine,
    ScreenHomography,
    ScreenPoly2,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum CalibrationQuality {
    Good,
    Fair,
    Poor,
    Failed,
    Skipped,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum TrackingVerdict {
    Good,
    Fair,
    Poor,
    Unusable,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum RecalReason {
    CalibrationQuality,
    ValidationAccuracy,
    Drift,
    PoseShift,
    OrientationChanged,
    CalibrationAge,
    OperatorRequest,
}

/// `rawPayload.trackingQuality` (QS-1, schema 1.7).
#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TrackingQuality {
    /// asd-core gaze maths version (`asd-gaze/1.0.0`).
    pipeline_version: String,
    /// `TrackingQualityParams.schemaVersion` the session ran with.
    params_version: String,
    device: TrackingDevice,
    calibration: CalibrationRecord,
    #[serde(default)]
    validation: Option<ValidationRecord>,
    #[serde(default)]
    drift_checks: Vec<DriftCheckRecord>,
    #[serde(default)]
    max_drift_deg: Option<f64>,
    #[serde(default)]
    recalibrations: Vec<RecalibrationRecord>,
    conditions: TrackingConditions,
    loss: LossSummary,
    confidence: ConfidenceSummary,
    /// QS-2; absent when not measured.
    #[serde(default)]
    estimated_accuracy_deg: Option<f64>,
    /// QS-4.
    verdict: TrackingVerdict,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TrackingDevice {
    model_identifier: String,
    geometry_source: GeometrySource,
    orientation: Orientation,
    screen_width_mm: f64,
    screen_height_mm: f64,
    #[serde(default)]
    sample_rate_hz: Option<f64>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CalibrationPose {
    distance_cm: f64,
    yaw_deg: f64,
    pitch_deg: f64,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CalibrationRecord {
    #[serde(default)]
    model: Option<CalModelKind>,
    layout_points: u32,
    usable_points: u32,
    missing_points: u32,
    #[serde(default)]
    residual_deg: Option<f64>,
    #[serde(default)]
    loo_cv_deg: Option<f64>,
    #[serde(default)]
    condition_number: Option<f64>,
    quality: CalibrationQuality,
    #[serde(default)]
    pose: Option<CalibrationPose>,
    #[serde(default)]
    age_sec_at_start: Option<f64>,
    attempts: u32,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ValidationTargetRecord {
    x_norm: f64,
    y_norm: f64,
    #[serde(default)]
    accuracy_deg: Option<f64>,
    #[serde(default)]
    precision_rms_deg: Option<f64>,
    #[serde(default)]
    sd_deg: Option<f64>,
    #[schemars(range(min = 0.0, max = 1.0))]
    data_loss: f64,
    valid_samples: u32,
    missed: bool,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ValidationRecord {
    complete: bool,
    targets: Vec<ValidationTargetRecord>,
    #[serde(default)]
    accuracy_deg: Option<f64>,
    #[serde(default)]
    precision_rms_deg: Option<f64>,
    #[serde(default)]
    precision_rms_raw_deg: Option<f64>,
    #[schemars(range(min = 0.0, max = 1.0))]
    data_loss: f64,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DriftCheckRecord {
    at_sec: f64,
    x_norm: f64,
    y_norm: f64,
    #[serde(default)]
    offset_deg: Option<f64>,
    #[serde(default)]
    drift_deg: Option<f64>,
    missed: bool,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RecalibrationRecord {
    at_sec: f64,
    reason: RecalReason,
    new_quality: CalibrationQuality,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TrackingConditions {
    #[serde(default)]
    distance_cm_median: Option<f64>,
    #[serde(default)]
    distance_cm_iqr: Option<f64>,
    #[serde(default)]
    head_speed_deg_per_sec_p90: Option<f64>,
    #[serde(default)]
    ambient_intensity_median: Option<f64>,
    #[serde(default)]
    light_asymmetry: Option<f64>,
    #[serde(default)]
    glasses: Option<bool>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LossSummary {
    track_loss_sec: f64,
    blink_sec: f64,
    offscreen_sec: f64,
    artifact_sec: f64,
    gap_filled_sec: f64,
    longest_gap_sec: f64,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ConfidenceSummary {
    #[schemars(range(min = 0.0, max = 1.0))]
    mean_sample: f64,
    #[serde(default)]
    #[schemars(range(min = 0.0, max = 1.0))]
    mean_fixation: Option<f64>,
    #[schemars(range(min = 0.0, max = 1.0))]
    low_confidence_sample_ratio: f64,
}

// ---- `ScoringProfile.trackingQuality` (SCH-3) ----------------------------------------------

/// `Default` for a params mirror = the asd-core default through JSON (single source).
macro_rules! default_from_core {
    ($($mirror:ident => $core:ty),* $(,)?) => {$(
        impl Default for $mirror {
            fn default() -> Self {
                let value = serde_json::to_value(<$core>::default())
                    .expect("asd-core tracking defaults serialise");
                serde_json::from_value(value).expect("params mirror matches the asd-core type")
            }
        }
    )*};
}

use asd_core::types::tracking as core_tracking;

default_from_core!(
    TrackingQualityParams => core_tracking::TrackingQualityParams,
    AccuracyBands => core_tracking::AccuracyBands,
    CalibrationParams => core_tracking::CalibrationParams,
    RecalibrationParams => core_tracking::RecalibrationParams,
    ValidationParams => core_tracking::ValidationParams,
    DriftParams => core_tracking::DriftParams,
    TrackStateParams => core_tracking::TrackStateParams,
    ConfidenceParams => core_tracking::ConfidenceParams,
    GapFillParams => core_tracking::GapFillParams,
    FilterParams => core_tracking::FilterParams,
    VerdictParams => core_tracking::VerdictParams,
    ScoringCouplingParams => core_tracking::ScoringCouplingParams,
);

// `#[schemars(default)]`, not `#[serde(default)]`: serde's container default would call
// `Default::default()` while deserialising, and that `Default` itself deserialises.

/// Every threshold of the gaze-robustness spec (SCH-3). Each member is optional; a missing
/// member takes the default shown.
#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(default)]
pub(crate) struct TrackingQualityParams {
    schema_version: String,
    /// `true` until the bench replaces the literature defaults (BEN-6).
    provisional: bool,
    calibration: CalibrationParams,
    recalibration: RecalibrationParams,
    validation: ValidationParams,
    drift: DriftParams,
    track_state: TrackStateParams,
    confidence: ConfidenceParams,
    gap_fill: GapFillParams,
    filter: FilterParams,
    verdict: VerdictParams,
    scoring: ScoringCouplingParams,
}

#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(default)]
pub(crate) struct AccuracyBands {
    good_max_deg: f64,
    fair_max_deg: f64,
    poor_max_deg: f64,
}

#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(default)]
pub(crate) struct CalibrationParams {
    settle_ms: f64,
    collect_ms: f64,
    max_target_wait_ms: f64,
    min_samples_per_target: u32,
    max_retries_per_target: u32,
    mad_reject_k: f64,
    default_model: CalModelKind,
    auto_select: bool,
    min_improvement_deg: f64,
    eligibility_margin_points: u32,
    bands: AccuracyBands,
    min_usable_points: u32,
    max_condition_number: f64,
    layout5: Vec<[f64; 2]>,
    layout9: Vec<[f64; 2]>,
}

#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(default)]
pub(crate) struct RecalibrationParams {
    pose_shift_distance_pct: f64,
    pose_shift_angle_deg: f64,
    pose_window_sec: f64,
    max_calibration_age_sec: f64,
    max_recalibrations_per_session: u32,
    max_recalibrations_in_battery: u32,
}

#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(default)]
pub(crate) struct ValidationParams {
    layout: Vec<[f64; 2]>,
    settle_ms: f64,
    collect_ms: f64,
    min_samples_per_target: u32,
    min_targets_hit: u32,
    warn_accuracy_deg: f64,
    max_accuracy_deg: f64,
    bench_layout13: Vec<[f64; 2]>,
}

#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(default)]
pub(crate) struct DriftParams {
    check_layout: Vec<[f64; 2]>,
    settle_ms: f64,
    collect_ms: f64,
    min_samples: u32,
    warn_deg: f64,
    warn_consecutive: u32,
    hard_deg: f64,
}

#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(default)]
pub(crate) struct TrackStateParams {
    blink_recovery_ms: f64,
    max_eye_speed_deg_per_sec: f64,
    offscreen_margin_norm: f64,
}

#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(default)]
pub(crate) struct ConfidenceParams {
    eye_openness_comfort: f64,
    pose_comfort_fraction: f64,
    distance_comfort_min_cm: f64,
    distance_comfort_max_cm: f64,
    head_speed_comfort_deg_per_sec: f64,
    max_head_speed_deg_per_sec: f64,
    pose_shift_comfort_pct: f64,
}

#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(default)]
pub(crate) struct GapFillParams {
    max_ms: f64,
    max_deg: f64,
    loss_interval_cap_ms: f64,
}

#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(default)]
pub(crate) struct FilterParams {
    min_cutoff_hz: f64,
    beta: f64,
    d_cutoff_hz: f64,
}

#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(default)]
pub(crate) struct VerdictParams {
    accuracy_bands: AccuracyBands,
    unusable_below_valid_ratio: f64,
    poor_above_data_loss: f64,
    fair_above_data_loss: f64,
}

#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(default)]
pub(crate) struct ScoringCouplingParams {
    max_accuracy_deg_for_scoring: f64,
    aoi_resolution_factor: f64,
    ivt_velocity_deg_per_sec: f64,
}
