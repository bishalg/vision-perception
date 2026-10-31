//! owner: WP-G.1
//!
//! Gaze-tracking types of `docs/spec/modules/asd-gaze-robustness/technical.md` (PIPE-2, REP-1,
//! GEO-1, QS-1, SCH-3) and ADR-0007. Data only; the maths lives in `crate::gaze`.
//!
//! Key casing:
//! - [`RawTrackingSample`] (one line of the raw log, PIPE-2) is **snake_case**, spelled exactly
//!   as PIPE-2 lists it; the bench writer (WP-G.8) writes it field for field.
//! - Everything else (log header, device geometry, processed sample, session
//!   `trackingQuality`, params) is **camelCase**, like the rest of the session document.
//!
//! Optional fields are `Option<T>` with `skip_serializing_if`, so an unmeasured value is an
//! absent key, never `null` and never a default (CON-14). Unknown keys are tolerated
//! (`deny_unknown_fields` is never set), so newer writers stay readable.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// `format` value of the raw log header line (REP-1).
pub const RAW_LOG_FORMAT: &str = "asd-raw-tracking/1";

/// `TrackingQualityParams.schemaVersion`, reported as `trackingQuality.paramsVersion`.
pub const TRACKING_PARAMS_SCHEMA_VERSION: &str = "1.0.0";

// ------------------------------------------------------------------------------------------
// Enums
// ------------------------------------------------------------------------------------------

/// Interface orientation (GEO-3), named as `UIInterfaceOrientation`: `landscapeLeft` has the
/// home side on the left and the camera on the right; `landscapeRight` the other way round.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Orientation {
    Portrait,
    PortraitUpsideDown,
    LandscapeLeft,
    LandscapeRight,
}

impl Orientation {
    /// Every orientation, in declaration order.
    pub const ALL: [Orientation; 4] = [
        Orientation::Portrait,
        Orientation::PortraitUpsideDown,
        Orientation::LandscapeLeft,
        Orientation::LandscapeRight,
    ];
}

/// Per-frame track state (TRK-1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TrackState {
    Tracked,
    Blink,
    Lost,
    Offscreen,
    Artifact,
}

/// Outcome of the raycast (GEO-5). Only `ok` produces a gaze point.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RayStatus {
    Ok,
    /// The ray points away from the screen plane.
    Backward,
    /// The ray is (nearly) parallel to the screen plane, or has no direction.
    Grazing,
    /// The hit parameter is outside the plausible range, or the input is not finite.
    OutOfRange,
}

/// Where the session's device geometry came from (GEO-2, QS-1 `device.geometrySource`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GeometrySource {
    Table,
    IdiomDefault,
}

/// Device idiom, used only to pick the fallback geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DeviceIdiom {
    Phone,
    Pad,
}

/// Provenance of a geometry entry's screen millimetres (GEO-1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ScreenSource {
    /// Derived from Apple's published resolution and ppi.
    #[serde(rename = "spec")]
    Spec,
    /// Measured on the bench (BEN-3).
    #[serde(rename = "measured")]
    Measured,
    /// One of the two hard-coded Swift idiom constants (G-F4).
    #[serde(rename = "legacy-default")]
    LegacyDefault,
}

/// Provenance of a camera offset (GEO-1; `measured` only after BEN-3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CameraOffsetSource {
    Measured,
    Unmeasured,
}

/// Which native front-end produced the raw samples (PLT-3: the axis convention is a
/// documented constant per source; see `gaze::raycast::CAMERA_AXES`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum TrackingSource {
    #[default]
    ArkitFace,
}

/// The kind of on-screen target (calibration, validation and drift checks share the target
/// machinery, PIPE-1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TargetKind {
    Calibration,
    Validation,
    DriftCheck,
}

/// Outcome of one target window (CAL-3, VAL-2, DRF-1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TargetOutcome {
    Hit,
    /// Calibration only: too few samples on the first attempt, show the target again.
    NeedsRetry,
    Missed,
}

/// Calibration model (CAL-4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum CalModelKind {
    #[default]
    ScreenAffine,
    AngularAffine,
    ScreenHomography,
    ScreenPoly2,
}

/// Calibration quality band (CAL-5, CAL-1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CalibrationQuality {
    Good,
    Fair,
    Poor,
    Failed,
    Skipped,
}

/// Session tracking verdict (QS-4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TrackingVerdict {
    Good,
    Fair,
    Poor,
    Unusable,
}

/// Re-calibration advice level (CAL-9, VAL-4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum RecalAction {
    #[default]
    None,
    /// Operator sees a warning and may continue (VAL-4 `warnAccuracyDeg`).
    Warn,
    /// Re-calibrate before the next clip.
    Recalibrate,
}

/// Why a re-calibration is advised or happened (CAL-9 triggers 1-6, plus an operator request).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RecalReason {
    CalibrationQuality,
    ValidationAccuracy,
    Drift,
    PoseShift,
    OrientationChanged,
    CalibrationAge,
    OperatorRequest,
}

// ------------------------------------------------------------------------------------------
// Device geometry (GEO-1)
// ------------------------------------------------------------------------------------------

/// TrueDepth aperture centre relative to the active area's top-left corner, portrait native
/// frame, millimetres, y down (negative y = above the glass).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraOffsetMm {
    pub x: f64,
    pub y: f64,
    pub source: CameraOffsetSource,
}

/// One entry of `resources/device_geometry.json`. All dimensions are in the portrait native
/// frame; `gaze::geometry` rotates them to the interface orientation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceGeometry {
    /// `utsname.machine` values, e.g. `"iPhone17,3"`. Empty for the idiom defaults.
    #[serde(default)]
    pub model_identifiers: Vec<String>,
    pub display_name: String,
    pub idiom: DeviceIdiom,
    /// Portrait `[width, height]` in pixels, as published. Absent for the idiom defaults.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_pixels: Option<[u32; 2]>,
    /// Published pixels per inch. Absent for the idiom defaults.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ppi: Option<f64>,
    pub screen_width_mm: f64,
    pub screen_height_mm: f64,
    pub camera_offset_mm: CameraOffsetMm,
    pub source: ScreenSource,
    /// Citation of every number in the entry (GEO-1).
    #[serde(rename = "ref")]
    pub reference: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// The two fallback geometries (GEO-2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdiomDefaults {
    pub phone: DeviceGeometry,
    pub pad: DeviceGeometry,
}

/// The whole compiled-in table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceGeometryTable {
    pub schema_version: String,
    pub idiom_defaults: IdiomDefaults,
    #[serde(default)]
    pub devices: Vec<DeviceGeometry>,
}

// ------------------------------------------------------------------------------------------
// Raw tracking log (PIPE-2, REP-1)
// ------------------------------------------------------------------------------------------

/// One ARKit frame as the native front-end saw it (PIPE-2); also one line of the raw log.
///
/// Matrices are 4x4, **column-major** (simd / ARKit memory layout: translation at indices
/// 12, 13, 14). `face_transform` is the face anchor in the camera frame (metres); `look_at`
/// and both eye transforms are in the face-anchor frame. Fields that ARKit only provides with
/// a face anchor are optional: a frame without a tracked face carries `face_tracked = false`
/// and omits them (TRK-1 records such frames; zeros would read as a perfect frontal pose).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RawTrackingSample {
    /// Frame timestamp in seconds (ARKit frame clock).
    pub t_sec: f64,
    pub face_tracked: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub face_transform: Option<[f64; 16]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub look_at: Option<[f64; 3]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub left_eye_transform: Option<[f64; 16]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub right_eye_transform: Option<[f64; 16]>,
    /// ARKit `eyeBlinkLeft` blend shape (0 open, 1 closed).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eye_blink_left: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eye_blink_right: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ambient_intensity: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ambient_color_temp: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub light_direction: Option<[f64; 3]>,
    pub orientation: Orientation,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clip_media_time_sec: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stimulus_id: Option<String>,
}

/// A target shown during a recorded log (bench schedule, REP-1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduledTarget {
    pub kind: TargetKind,
    pub x_norm: f64,
    pub y_norm: f64,
    pub start_sec: f64,
    pub end_sec: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

/// First line of a raw log (REP-1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawLogHeader {
    /// Always [`RAW_LOG_FORMAT`].
    pub format: String,
    #[serde(default)]
    pub source: TrackingSource,
    pub device_model: String,
    pub orientation: Orientation,
    pub geometry_source: GeometrySource,
    /// The geometry the recording device used (portrait native frame).
    pub geometry: DeviceGeometry,
    #[serde(default)]
    pub targets: Vec<ScheduledTarget>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,
    /// Synthetic logs only (TV-1): the generator's ground truth; shape owned by the generator.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ground_truth: Option<Value>,
}

// ------------------------------------------------------------------------------------------
// Pipeline output (PIPE-1)
// ------------------------------------------------------------------------------------------

/// One processed frame (`GazePipeline::push` output). Points are screen-normalised (x, y in
/// [0,1] over the full display in the current orientation, origin top-left, y down) and
/// **never clamped**.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessedSample {
    pub t_sec: f64,
    pub track_state: TrackState,
    /// Absent when there was no face to raycast.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ray_status: Option<RayStatus>,
    /// Raycast hit before calibration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_point_norm: Option<[f64; 2]>,
    /// After calibration, before filtering (validity is decided here, TRK-2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calibrated_point_norm: Option<[f64; 2]>,
    /// After the 1-Euro filter (tracked samples only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filtered_point_norm: Option<[f64; 2]>,
    /// Eye origin in the oriented screen frame, mm (z = distance in front of the glass).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eye_pos_mm: Option<[f64; 3]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub distance_cm: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head_yaw_rad: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head_pitch_rad: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head_roll_rad: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head_speed_deg_per_sec: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eye_speed_deg_per_sec: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub left_eye_openness: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub right_eye_openness: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ambient_intensity: Option<f64>,
    /// CNF-1, in [0,1]; 0 for any state other than `tracked`.
    pub confidence: f64,
    /// `tracked` and confidence at or above `minConfidence`.
    pub valid: bool,
    /// GAP-1. Always `false` from `push`; set only by gap filling.
    #[serde(default)]
    pub gap_filled: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub active_aoi_names: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stimulus_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clip_media_time_sec: Option<f64>,
}

/// Result of one target window (`GazePipeline::end_target`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetResult {
    pub kind: TargetKind,
    pub x_norm: f64,
    pub y_norm: f64,
    pub started_at_sec: f64,
    pub ended_at_sec: f64,
    /// 1 for the first showing, 2 for the CAL-3 re-show.
    pub attempt: u32,
    pub outcome: TargetOutcome,
    pub valid_samples: u32,
    pub total_samples: u32,
    /// Share of frame time in the window that was not `tracked`.
    pub data_loss: f64,
    /// Robust median of the uncalibrated points (calibration input).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_median_norm: Option<[f64; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calibrated_median_norm: Option<[f64; 2]>,
    /// Median eye position during the window (angles are measured from here).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eye_pos_mm_median: Option<[f64; 3]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accuracy_deg: Option<f64>,
    /// Signed offset (gaze minus target) in degrees, x right and y down (DRF-2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset_deg: Option<[f64; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub precision_rms_deg: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub precision_rms_raw_deg: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sd_deg: Option<f64>,
}

/// Re-calibration advice (`GazePipeline::advice`, CAL-9).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RecalAdvice {
    pub action: RecalAction,
    #[serde(default)]
    pub reasons: Vec<RecalReason>,
    /// A trigger fired but `maxRecalibrationsPerSession` (or the in-battery limit) is spent.
    #[serde(default)]
    pub limit_reached: bool,
}

// ------------------------------------------------------------------------------------------
// Session record (QS-1)
// ------------------------------------------------------------------------------------------

/// `rawPayload.trackingQuality` (QS-1, schema 1.7).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackingQuality {
    /// `asd-core` gaze maths version (`gaze::PIPELINE_VERSION`).
    pub pipeline_version: String,
    /// `TrackingQualityParams.schemaVersion`.
    pub params_version: String,
    pub device: TrackingDevice,
    pub calibration: CalibrationRecord,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validation: Option<ValidationRecord>,
    #[serde(default)]
    pub drift_checks: Vec<DriftCheckRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_drift_deg: Option<f64>,
    #[serde(default)]
    pub recalibrations: Vec<RecalibrationRecord>,
    pub conditions: TrackingConditions,
    pub loss: LossSummary,
    pub confidence: ConfidenceSummary,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimated_accuracy_deg: Option<f64>,
    pub verdict: TrackingVerdict,
}

/// `trackingQuality.device`. Screen millimetres are as laid out in `orientation` (the frame
/// every `xNorm`/`yNorm` of the record uses).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackingDevice {
    pub model_identifier: String,
    pub geometry_source: GeometrySource,
    pub orientation: Orientation,
    pub screen_width_mm: f64,
    pub screen_height_mm: f64,
    /// 1 / median frame interval. Absent with fewer than two frames.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample_rate_hz: Option<f64>,
}

/// Head pose at calibration (median over the calibration).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalibrationPose {
    pub distance_cm: f64,
    pub yaw_deg: f64,
    pub pitch_deg: f64,
}

/// `trackingQuality.calibration`. Fitted numbers are absent when the calibration was skipped
/// or could not be fitted.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalibrationRecord {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<CalModelKind>,
    pub layout_points: u32,
    pub usable_points: u32,
    pub missing_points: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub residual_deg: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loo_cv_deg: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition_number: Option<f64>,
    pub quality: CalibrationQuality,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pose: Option<CalibrationPose>,
    /// Calibration age when the first clip started.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub age_sec_at_start: Option<f64>,
    pub attempts: u32,
}

/// One validation target in the record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationTargetRecord {
    pub x_norm: f64,
    pub y_norm: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accuracy_deg: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub precision_rms_deg: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sd_deg: Option<f64>,
    pub data_loss: f64,
    pub valid_samples: u32,
    pub missed: bool,
}

/// `trackingQuality.validation` (VAL-2, VAL-3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationRecord {
    pub complete: bool,
    pub targets: Vec<ValidationTargetRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accuracy_deg: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub precision_rms_deg: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub precision_rms_raw_deg: Option<f64>,
    pub data_loss: f64,
}

/// One drift check (DRF-2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DriftCheckRecord {
    pub at_sec: f64,
    pub x_norm: f64,
    pub y_norm: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset_deg: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drift_deg: Option<f64>,
    pub missed: bool,
}

/// One re-calibration that happened (CAL-9).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecalibrationRecord {
    pub at_sec: f64,
    pub reason: RecalReason,
    pub new_quality: CalibrationQuality,
}

/// `trackingQuality.conditions`. Every value is absent when it was not measured (for example
/// a session without a single tracked frame).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TrackingConditions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub distance_cm_median: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub distance_cm_iqr: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head_speed_deg_per_sec_p90: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ambient_intensity_median: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub light_asymmetry: Option<f64>,
    /// Operator answer at the positioning step; absent when not asked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub glasses: Option<bool>,
}

/// `trackingQuality.loss` (GAP-3), seconds.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LossSummary {
    pub track_loss_sec: f64,
    pub blink_sec: f64,
    pub offscreen_sec: f64,
    pub artifact_sec: f64,
    pub gap_filled_sec: f64,
    pub longest_gap_sec: f64,
}

/// `trackingQuality.confidence` (CNF-3).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfidenceSummary {
    pub mean_sample: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mean_fixation: Option<f64>,
    pub low_confidence_sample_ratio: f64,
}

// ------------------------------------------------------------------------------------------
// Parameters (SCH-3: `ScoringProfile.trackingQuality`)
// ------------------------------------------------------------------------------------------

/// Every threshold of the gaze-robustness spec, with its provisional default. Delivered in
/// the scoring profile (SCH-3); each struct is `serde(default)` so an older or partial bundle
/// decodes to these defaults.
///
/// The six CORE-7 gate limits (`minDistanceCm`, `maxDistanceCm`, `maxYawRad`, `maxPitchRad`,
/// `minConfidence`, `blinkThreshold`) are deliberately **not** repeated here: they stay in
/// `ScoringProfile.qualityGate` (`crate::quality::QualityGate`) and the confidence ramps
/// (CNF-1) read them from there, so the 0.5 crossing cannot drift from the gate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TrackingQualityParams {
    pub schema_version: String,
    /// `true` until the WP-G.14 bench replaces the literature defaults (BEN-6).
    pub provisional: bool,
    pub calibration: CalibrationParams,
    pub recalibration: RecalibrationParams,
    pub validation: ValidationParams,
    pub drift: DriftParams,
    pub track_state: TrackStateParams,
    pub confidence: ConfidenceParams,
    pub gap_fill: GapFillParams,
    pub filter: FilterParams,
    pub verdict: VerdictParams,
    pub scoring: ScoringCouplingParams,
}

impl Default for TrackingQualityParams {
    fn default() -> Self {
        Self {
            schema_version: TRACKING_PARAMS_SCHEMA_VERSION.to_string(),
            provisional: true,
            calibration: CalibrationParams::default(),
            recalibration: RecalibrationParams::default(),
            validation: ValidationParams::default(),
            drift: DriftParams::default(),
            track_state: TrackStateParams::default(),
            confidence: ConfidenceParams::default(),
            gap_fill: GapFillParams::default(),
            filter: FilterParams::default(),
            verdict: VerdictParams::default(),
            scoring: ScoringCouplingParams::default(),
        }
    }
}

/// Visual-angle bands, upper bounds inclusive (CAL-5, QS-4).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AccuracyBands {
    pub good_max_deg: f64,
    pub fair_max_deg: f64,
    pub poor_max_deg: f64,
}

impl Default for AccuracyBands {
    fn default() -> Self {
        Self {
            good_max_deg: 2.5,
            fair_max_deg: 3.5,
            poor_max_deg: 5.0,
        }
    }
}

/// CAL-3 .. CAL-8.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CalibrationParams {
    pub settle_ms: f64,
    pub collect_ms: f64,
    pub max_target_wait_ms: f64,
    pub min_samples_per_target: u32,
    /// Times a starved target is shown again before it is `missed` (CAL-3: once).
    pub max_retries_per_target: u32,
    /// Per-axis rejection beyond `k x MAD` from the median.
    pub mad_reject_k: f64,
    pub default_model: CalModelKind,
    pub auto_select: bool,
    pub min_improvement_deg: f64,
    /// A model is eligible with at least `minPoints + eligibilityMarginPoints` usable points.
    pub eligibility_margin_points: u32,
    pub bands: AccuracyBands,
    pub min_usable_points: u32,
    pub max_condition_number: f64,
    pub layout5: Vec<[f64; 2]>,
    pub layout9: Vec<[f64; 2]>,
}

impl Default for CalibrationParams {
    fn default() -> Self {
        let layout5 = vec![
            [0.5, 0.5],
            [0.12, 0.12],
            [0.88, 0.12],
            [0.12, 0.88],
            [0.88, 0.88],
        ];
        let mut layout9 = layout5.clone();
        layout9.extend([[0.5, 0.12], [0.12, 0.5], [0.88, 0.5], [0.5, 0.88]]);
        Self {
            settle_ms: 400.0,
            collect_ms: 1200.0,
            max_target_wait_ms: 4000.0,
            min_samples_per_target: 20,
            max_retries_per_target: 1,
            mad_reject_k: 3.0,
            default_model: CalModelKind::ScreenAffine,
            auto_select: false,
            min_improvement_deg: 0.3,
            eligibility_margin_points: 2,
            bands: AccuracyBands::default(),
            min_usable_points: 4,
            max_condition_number: 1e6,
            layout5,
            layout9,
        }
    }
}

/// CAL-9 triggers and limits.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RecalibrationParams {
    /// Distance change from the calibration pose, percent (also the CNF-1 limit).
    pub pose_shift_distance_pct: f64,
    pub pose_shift_angle_deg: f64,
    /// Window of the median head pose compared with the calibration pose.
    pub pose_window_sec: f64,
    pub max_calibration_age_sec: f64,
    pub max_recalibrations_per_session: u32,
    pub max_recalibrations_in_battery: u32,
}

impl Default for RecalibrationParams {
    fn default() -> Self {
        Self {
            pose_shift_distance_pct: 20.0,
            pose_shift_angle_deg: 15.0,
            pose_window_sec: 3.0,
            max_calibration_age_sec: 600.0,
            max_recalibrations_per_session: 2,
            max_recalibrations_in_battery: 1,
        }
    }
}

/// VAL-1 .. VAL-5.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ValidationParams {
    pub layout: Vec<[f64; 2]>,
    pub settle_ms: f64,
    pub collect_ms: f64,
    pub min_samples_per_target: u32,
    /// Validation is `complete` with at least this many hit targets.
    pub min_targets_hit: u32,
    pub warn_accuracy_deg: f64,
    pub max_accuracy_deg: f64,
    /// Bench-only 13-point grid (VAL-5).
    pub bench_layout13: Vec<[f64; 2]>,
}

impl Default for ValidationParams {
    fn default() -> Self {
        let mut bench = Vec::with_capacity(13);
        for y in [0.1, 0.5, 0.9] {
            for x in [0.1, 0.5, 0.9] {
                bench.push([x, y]);
            }
        }
        bench.extend([[0.3, 0.3], [0.7, 0.3], [0.3, 0.7], [0.7, 0.7]]);
        Self {
            layout: vec![[0.30, 0.30], [0.70, 0.30], [0.30, 0.70], [0.70, 0.70]],
            settle_ms: 400.0,
            collect_ms: 1000.0,
            min_samples_per_target: 12,
            min_targets_hit: 3,
            warn_accuracy_deg: 3.5,
            max_accuracy_deg: 5.0,
            bench_layout13: bench,
        }
    }
}

/// DRF-1 .. DRF-3.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DriftParams {
    pub check_layout: Vec<[f64; 2]>,
    pub settle_ms: f64,
    pub collect_ms: f64,
    pub min_samples: u32,
    pub warn_deg: f64,
    /// Consecutive checks above `warnDeg` that trigger (DRF-3: two).
    pub warn_consecutive: u32,
    pub hard_deg: f64,
}

impl Default for DriftParams {
    fn default() -> Self {
        Self {
            check_layout: vec![[0.5, 0.5], [0.3, 0.3], [0.7, 0.3], [0.3, 0.7], [0.7, 0.7]],
            settle_ms: 300.0,
            collect_ms: 900.0,
            min_samples: 10,
            warn_deg: 1.5,
            warn_consecutive: 2,
            hard_deg: 3.0,
        }
    }
}

/// TRK-1.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TrackStateParams {
    pub blink_recovery_ms: f64,
    pub max_eye_speed_deg_per_sec: f64,
    pub offscreen_margin_norm: f64,
}

impl Default for TrackStateParams {
    fn default() -> Self {
        Self {
            blink_recovery_ms: 50.0,
            max_eye_speed_deg_per_sec: 1000.0,
            offscreen_margin_norm: 0.02,
        }
    }
}

/// CNF-1 comfort zones. Hard limits come from `QualityGate` (distance, pose, blink),
/// `maxHeadSpeedDegPerSec` here, and `recalibration.poseShiftDistancePct`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ConfidenceParams {
    pub eye_openness_comfort: f64,
    /// Comfort zone for |yaw| and |pitch| as a fraction of the gate limit.
    pub pose_comfort_fraction: f64,
    pub distance_comfort_min_cm: f64,
    pub distance_comfort_max_cm: f64,
    pub head_speed_comfort_deg_per_sec: f64,
    /// Also the CAL-2 sample gate.
    pub max_head_speed_deg_per_sec: f64,
    pub pose_shift_comfort_pct: f64,
}

impl Default for ConfidenceParams {
    fn default() -> Self {
        Self {
            eye_openness_comfort: 0.70,
            pose_comfort_fraction: 0.6,
            distance_comfort_min_cm: 40.0,
            distance_comfort_max_cm: 60.0,
            head_speed_comfort_deg_per_sec: 30.0,
            max_head_speed_deg_per_sec: 120.0,
            pose_shift_comfort_pct: 10.0,
        }
    }
}

/// GAP-1 .. GAP-3.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GapFillParams {
    pub max_ms: f64,
    pub max_deg: f64,
    /// Each frame interval counts at most this long in the loss sums (PLC-5).
    pub loss_interval_cap_ms: f64,
}

impl Default for GapFillParams {
    fn default() -> Self {
        Self {
            max_ms: 75.0,
            max_deg: 1.0,
            loss_interval_cap_ms: 100.0,
        }
    }
}

/// PIPE-3 1-Euro filter (defaults of `OneEuroFilter.swift`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FilterParams {
    pub min_cutoff_hz: f64,
    pub beta: f64,
    pub d_cutoff_hz: f64,
}

impl Default for FilterParams {
    fn default() -> Self {
        Self {
            min_cutoff_hz: 1.2,
            beta: 0.015,
            d_cutoff_hz: 1.0,
        }
    }
}

/// QS-4.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct VerdictParams {
    pub accuracy_bands: AccuracyBands,
    pub unusable_below_valid_ratio: f64,
    pub poor_above_data_loss: f64,
    pub fair_above_data_loss: f64,
}

impl Default for VerdictParams {
    fn default() -> Self {
        Self {
            accuracy_bands: AccuracyBands::default(),
            unusable_below_valid_ratio: 0.50,
            poor_above_data_loss: 0.40,
            fair_above_data_loss: 0.20,
        }
    }
}

/// SCR-1 .. SCR-3 (used only when the owner enables WP-G.13).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ScoringCouplingParams {
    pub max_accuracy_deg_for_scoring: f64,
    pub aoi_resolution_factor: f64,
    pub ivt_velocity_deg_per_sec: f64,
}

impl Default for ScoringCouplingParams {
    fn default() -> Self {
        Self {
            max_accuracy_deg_for_scoring: 5.0,
            aoi_resolution_factor: 2.0,
            ivt_velocity_deg_per_sec: 30.0,
        }
    }
}
