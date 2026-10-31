//! owner: WP-G.7
//!
//! UniFFI surface over the per-session gaze pipeline (`asd_core::gaze::pipeline`, PIPE-1,
//! ADR-0007 decision 2).
//!
//! - **Per frame (60 Hz)** the app calls [`GazePipelineHandle::push`] with a flat
//!   [`RawSampleRecord`] and gets a flat [`ProcessedSampleRecord`]: typed records, no JSON
//!   (the narrow exception to ADR-0006 decision 4). [`GazeTelemetryWriterHandle::write`]
//!   turns a processed record into the frames-file sample, also as a record.
//! - **Per target** (calibration, validation, drift check) the handle takes and returns small
//!   typed records and enums, because the UI branches on them.
//! - **Documents** stay JSON: the `PipelineConfig` in, the session `TrackingQuality` out
//!   ([`GazePipelineHandle::summary_json`]), the calibration model (stored by the app as an
//!   opaque blob bound to the session, CAL-1), the validation record, drift checks and AOIs.
//!
//! # Threads
//! UniFFI objects are shared through `Arc` and must be `Send + Sync`; the core pipeline is
//! single-owner and not `Sync` (trap 10), so the handle wraps it in a `Mutex`. The lock is
//! uncontended when, as intended, one Swift actor (`GazeProcessingActor`) owns the handle and
//! makes every call; calls from several threads are safe but serialise and can reorder frames,
//! which breaks the frame-clock windows (PIPE-5). Call from one actor.
//!
//! # Errors
//! Nothing here panics on caller input: wrong-length matrices are
//! `AsdError::InvalidArgument`, malformed JSON is `AsdError::InvalidJson`, and misuse of the
//! target API (a window already open, no calibration running, a calibration target that is
//! not the pending one) is `AsdError::InvalidArgument` with the reason in `message`. A
//! poisoned lock (a panic inside the core on another call) is recovered, not propagated.
//!
//! This file holds no gaze logic beyond converting records and delegating.

use std::sync::{Arc, Mutex, MutexGuard};

use asd_core::gaze::aoi_space::{self, AoiSpace, ScreenAoi, VideoGravity};
use asd_core::gaze::calibration::{CalModel, TargetStatus};
use asd_core::gaze::pipeline::{GazePipeline, PipelineConfig, PipelineError, TelemetryWriter};
use asd_core::quality::QualityGate;
use asd_core::types::catalog::Area;
use asd_core::types::tracking::{
    CalibrationQuality, Orientation, ProcessedSample, RawTrackingSample, RayStatus, RecalAction,
    RecalAdvice, RecalReason, TargetKind, TargetOutcome, TargetResult, TrackState,
};
use asd_core::types::GazeTelemetrySample;

use crate::{decode, encode, AsdError};

// --- small value records ---------------------------------------------------------------

/// A 2-vector: a screen-normalised point (origin top-left, y down, never clamped) or a
/// `[x, y]` angle pair in degrees.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct GazeVec2 {
    pub x: f64,
    pub y: f64,
}

/// A 3-vector (`look_at` in face space, metres; light direction; eye position in mm).
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct GazeVec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl From<[f64; 2]> for GazeVec2 {
    fn from(v: [f64; 2]) -> Self {
        Self { x: v[0], y: v[1] }
    }
}

impl From<GazeVec2> for [f64; 2] {
    fn from(v: GazeVec2) -> Self {
        [v.x, v.y]
    }
}

impl From<[f64; 3]> for GazeVec3 {
    fn from(v: [f64; 3]) -> Self {
        Self {
            x: v[0],
            y: v[1],
            z: v[2],
        }
    }
}

impl From<GazeVec3> for [f64; 3] {
    fn from(v: GazeVec3) -> Self {
        [v.x, v.y, v.z]
    }
}

// --- enums (mirrors of the core enums, same case names) --------------------------------

/// Interface orientation (GEO-3), named as `UIInterfaceOrientation`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum GazeOrientation {
    Portrait,
    PortraitUpsideDown,
    LandscapeLeft,
    LandscapeRight,
}

/// Per-frame track state (TRK-1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum GazeTrackState {
    Tracked,
    Blink,
    Lost,
    Offscreen,
    Artifact,
}

/// Raycast outcome (GEO-5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum GazeRayStatus {
    Ok,
    Backward,
    Grazing,
    OutOfRange,
}

/// Target kind; calibration, validation and drift checks share the target API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum GazeTargetKind {
    Calibration,
    Validation,
    DriftCheck,
}

/// Result of one target window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum GazeTargetOutcome {
    Hit,
    /// Calibration only: too few samples on the first attempt; show the target again.
    NeedsRetry,
    Missed,
}

/// Calibration quality band (CAL-5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum GazeCalibrationQuality {
    Good,
    Fair,
    Poor,
    Failed,
    Skipped,
}

/// What the advice asks for (CAL-9, VAL-4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum GazeRecalAction {
    None,
    Warn,
    Recalibrate,
}

/// Why a re-calibration is advised or happened.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum GazeRecalReason {
    CalibrationQuality,
    ValidationAccuracy,
    Drift,
    PoseShift,
    OrientationChanged,
    CalibrationAge,
    OperatorRequest,
}

/// Which space a clip's catalog AOI boxes are in (catalog `aoiSpace`, trap 11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum GazeAoiSpace {
    Video,
    Screen,
}

/// How the video layer fills the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum GazeVideoGravity {
    AspectFill,
    AspectFit,
}

/// One-to-one mappings between a core enum and its FFI mirror.
macro_rules! mirror_enum {
    ($core:ty, $ffi:ty, [$($v:ident),+ $(,)?]) => {
        impl From<$core> for $ffi {
            fn from(v: $core) -> Self {
                match v { $(<$core>::$v => Self::$v),+ }
            }
        }
        impl From<$ffi> for $core {
            fn from(v: $ffi) -> Self {
                match v { $(<$ffi>::$v => Self::$v),+ }
            }
        }
    };
}

mirror_enum!(
    Orientation,
    GazeOrientation,
    [Portrait, PortraitUpsideDown, LandscapeLeft, LandscapeRight]
);
mirror_enum!(
    TrackState,
    GazeTrackState,
    [Tracked, Blink, Lost, Offscreen, Artifact]
);
mirror_enum!(
    RayStatus,
    GazeRayStatus,
    [Ok, Backward, Grazing, OutOfRange]
);
mirror_enum!(
    TargetKind,
    GazeTargetKind,
    [Calibration, Validation, DriftCheck]
);
mirror_enum!(TargetOutcome, GazeTargetOutcome, [Hit, NeedsRetry, Missed]);
mirror_enum!(
    CalibrationQuality,
    GazeCalibrationQuality,
    [Good, Fair, Poor, Failed, Skipped]
);
mirror_enum!(RecalAction, GazeRecalAction, [None, Warn, Recalibrate]);
mirror_enum!(
    RecalReason,
    GazeRecalReason,
    [
        CalibrationQuality,
        ValidationAccuracy,
        Drift,
        PoseShift,
        OrientationChanged,
        CalibrationAge,
        OperatorRequest
    ]
);
mirror_enum!(AoiSpace, GazeAoiSpace, [Video, Screen]);
mirror_enum!(VideoGravity, GazeVideoGravity, [AspectFill, AspectFit]);

// --- per-frame records -----------------------------------------------------------------

/// One ARKit frame (PIPE-2, the replay log line REP-1). Matrices are column-major 4x4
/// (`simd_float4x4` columns in order), exactly 16 numbers, in ARKit's own spaces: the face
/// anchor in the camera frame, the eyes in the face frame. `None` when ARKit gave none.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct RawSampleRecord {
    /// ARKit frame clock, seconds.
    pub t_sec: f64,
    pub face_tracked: bool,
    pub face_transform: Option<Vec<f64>>,
    /// `ARFaceAnchor.lookAtPoint`, face space.
    pub look_at: Option<GazeVec3>,
    pub left_eye_transform: Option<Vec<f64>>,
    pub right_eye_transform: Option<Vec<f64>>,
    /// Blend shape `eyeBlinkLeft` (0 open, 1 closed).
    pub eye_blink_left: Option<f64>,
    pub eye_blink_right: Option<f64>,
    pub ambient_intensity: Option<f64>,
    pub ambient_color_temp: Option<f64>,
    pub light_direction: Option<GazeVec3>,
    pub orientation: GazeOrientation,
    pub clip_media_time_sec: Option<f64>,
    pub stimulus_id: Option<String>,
}

/// One processed frame (`ProcessedSample`). Points are screen-normalised and never clamped
/// (clamp only for drawing).
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct ProcessedSampleRecord {
    pub t_sec: f64,
    pub track_state: GazeTrackState,
    /// `None` when there was no face to raycast.
    pub ray_status: Option<GazeRayStatus>,
    pub raw_point_norm: Option<GazeVec2>,
    /// After calibration, before filtering (validity is decided here).
    pub calibrated_point_norm: Option<GazeVec2>,
    /// After the 1-Euro filter (tracked frames only). The point to draw and to store.
    pub filtered_point_norm: Option<GazeVec2>,
    /// Eye origin in the oriented screen frame, mm (z = distance in front of the glass).
    pub eye_pos_mm: Option<GazeVec3>,
    pub distance_cm: Option<f64>,
    pub head_yaw_rad: Option<f64>,
    pub head_pitch_rad: Option<f64>,
    pub head_roll_rad: Option<f64>,
    pub head_speed_deg_per_sec: Option<f64>,
    pub eye_speed_deg_per_sec: Option<f64>,
    pub left_eye_openness: Option<f64>,
    pub right_eye_openness: Option<f64>,
    pub ambient_intensity: Option<f64>,
    /// CNF-1 in [0, 1]; 0 for any state other than `tracked`.
    pub confidence: f64,
    /// `tracked` and confidence at or above `qualityGate.minConfidence`.
    pub valid: bool,
    /// Always `false` from `push`; set only by [`GazePipelineHandle::gap_fill`].
    pub gap_filled: bool,
    pub active_aoi_names: Vec<String>,
    pub stimulus_id: Option<String>,
    pub clip_media_time_sec: Option<f64>,
}

fn matrix(v: Option<Vec<f64>>, what: &str) -> Result<Option<[f64; 16]>, AsdError> {
    v.map(|m| {
        <[f64; 16]>::try_from(m.as_slice()).map_err(|_| AsdError::InvalidArgument {
            message: format!("{what}: expected 16 numbers, got {}", m.len()),
        })
    })
    .transpose()
}

impl TryFrom<RawSampleRecord> for RawTrackingSample {
    type Error = AsdError;

    fn try_from(r: RawSampleRecord) -> Result<Self, AsdError> {
        Ok(Self {
            t_sec: r.t_sec,
            face_tracked: r.face_tracked,
            face_transform: matrix(r.face_transform, "face_transform")?,
            look_at: r.look_at.map(Into::into),
            left_eye_transform: matrix(r.left_eye_transform, "left_eye_transform")?,
            right_eye_transform: matrix(r.right_eye_transform, "right_eye_transform")?,
            eye_blink_left: r.eye_blink_left,
            eye_blink_right: r.eye_blink_right,
            ambient_intensity: r.ambient_intensity,
            ambient_color_temp: r.ambient_color_temp,
            light_direction: r.light_direction.map(Into::into),
            orientation: r.orientation.into(),
            clip_media_time_sec: r.clip_media_time_sec,
            stimulus_id: r.stimulus_id,
        })
    }
}

impl From<RawTrackingSample> for RawSampleRecord {
    fn from(s: RawTrackingSample) -> Self {
        Self {
            t_sec: s.t_sec,
            face_tracked: s.face_tracked,
            face_transform: s.face_transform.map(|m| m.to_vec()),
            look_at: s.look_at.map(Into::into),
            left_eye_transform: s.left_eye_transform.map(|m| m.to_vec()),
            right_eye_transform: s.right_eye_transform.map(|m| m.to_vec()),
            eye_blink_left: s.eye_blink_left,
            eye_blink_right: s.eye_blink_right,
            ambient_intensity: s.ambient_intensity,
            ambient_color_temp: s.ambient_color_temp,
            light_direction: s.light_direction.map(Into::into),
            orientation: s.orientation.into(),
            clip_media_time_sec: s.clip_media_time_sec,
            stimulus_id: s.stimulus_id,
        }
    }
}

impl From<ProcessedSample> for ProcessedSampleRecord {
    fn from(s: ProcessedSample) -> Self {
        Self {
            t_sec: s.t_sec,
            track_state: s.track_state.into(),
            ray_status: s.ray_status.map(Into::into),
            raw_point_norm: s.raw_point_norm.map(Into::into),
            calibrated_point_norm: s.calibrated_point_norm.map(Into::into),
            filtered_point_norm: s.filtered_point_norm.map(Into::into),
            eye_pos_mm: s.eye_pos_mm.map(Into::into),
            distance_cm: s.distance_cm,
            head_yaw_rad: s.head_yaw_rad,
            head_pitch_rad: s.head_pitch_rad,
            head_roll_rad: s.head_roll_rad,
            head_speed_deg_per_sec: s.head_speed_deg_per_sec,
            eye_speed_deg_per_sec: s.eye_speed_deg_per_sec,
            left_eye_openness: s.left_eye_openness,
            right_eye_openness: s.right_eye_openness,
            ambient_intensity: s.ambient_intensity,
            confidence: s.confidence,
            valid: s.valid,
            gap_filled: s.gap_filled,
            active_aoi_names: s.active_aoi_names,
            stimulus_id: s.stimulus_id,
            clip_media_time_sec: s.clip_media_time_sec,
        }
    }
}

impl From<ProcessedSampleRecord> for ProcessedSample {
    fn from(r: ProcessedSampleRecord) -> Self {
        Self {
            t_sec: r.t_sec,
            track_state: r.track_state.into(),
            ray_status: r.ray_status.map(Into::into),
            raw_point_norm: r.raw_point_norm.map(Into::into),
            calibrated_point_norm: r.calibrated_point_norm.map(Into::into),
            filtered_point_norm: r.filtered_point_norm.map(Into::into),
            eye_pos_mm: r.eye_pos_mm.map(Into::into),
            distance_cm: r.distance_cm,
            head_yaw_rad: r.head_yaw_rad,
            head_pitch_rad: r.head_pitch_rad,
            head_roll_rad: r.head_roll_rad,
            head_speed_deg_per_sec: r.head_speed_deg_per_sec,
            eye_speed_deg_per_sec: r.eye_speed_deg_per_sec,
            left_eye_openness: r.left_eye_openness,
            right_eye_openness: r.right_eye_openness,
            ambient_intensity: r.ambient_intensity,
            confidence: r.confidence,
            valid: r.valid,
            gap_filled: r.gap_filled,
            active_aoi_names: r.active_aoi_names,
            stimulus_id: r.stimulus_id,
            clip_media_time_sec: r.clip_media_time_sec,
        }
    }
}

// --- target records --------------------------------------------------------------------

/// What the calibration UI should show next.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Enum)]
pub enum GazeCalibrationStatus {
    /// Show `target`, then call `begin_target(Calibration, target)` when it appears.
    Pending {
        index: u32,
        target: GazeVec2,
        attempt: u32,
    },
    /// A target window is open; keep pushing frames until `target_complete()`.
    Running {
        index: u32,
        target: GazeVec2,
        attempt: u32,
        started_at_sec: f64,
    },
    /// Every target was shown; call `finish_calibration()`.
    Complete,
}

fn index_u32(i: usize) -> u32 {
    u32::try_from(i).unwrap_or(u32::MAX)
}

impl From<TargetStatus> for GazeCalibrationStatus {
    fn from(s: TargetStatus) -> Self {
        match s {
            TargetStatus::Pending {
                index,
                target_norm,
                attempt,
            } => Self::Pending {
                index: index_u32(index),
                target: target_norm.into(),
                attempt,
            },
            TargetStatus::Running {
                index,
                target_norm,
                attempt,
                started_at_sec,
            } => Self::Running {
                index: index_u32(index),
                target: target_norm.into(),
                attempt,
                started_at_sec,
            },
            TargetStatus::Complete => Self::Complete,
        }
    }
}

/// Result of one target window (`TargetResult`); angles in degrees, x right and y down.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct GazeTargetResultRecord {
    pub kind: GazeTargetKind,
    pub target: GazeVec2,
    pub started_at_sec: f64,
    pub ended_at_sec: f64,
    /// 1 for the first showing, 2 for the re-show.
    pub attempt: u32,
    pub outcome: GazeTargetOutcome,
    pub valid_samples: u32,
    pub total_samples: u32,
    pub data_loss: f64,
    pub raw_median_norm: Option<GazeVec2>,
    pub calibrated_median_norm: Option<GazeVec2>,
    pub eye_pos_mm_median: Option<GazeVec3>,
    pub accuracy_deg: Option<f64>,
    pub offset_deg: Option<GazeVec2>,
    pub precision_rms_deg: Option<f64>,
    pub precision_rms_raw_deg: Option<f64>,
    pub sd_deg: Option<f64>,
}

impl From<TargetResult> for GazeTargetResultRecord {
    fn from(r: TargetResult) -> Self {
        Self {
            kind: r.kind.into(),
            target: GazeVec2 {
                x: r.x_norm,
                y: r.y_norm,
            },
            started_at_sec: r.started_at_sec,
            ended_at_sec: r.ended_at_sec,
            attempt: r.attempt,
            outcome: r.outcome.into(),
            valid_samples: r.valid_samples,
            total_samples: r.total_samples,
            data_loss: r.data_loss,
            raw_median_norm: r.raw_median_norm.map(Into::into),
            calibrated_median_norm: r.calibrated_median_norm.map(Into::into),
            eye_pos_mm_median: r.eye_pos_mm_median.map(Into::into),
            accuracy_deg: r.accuracy_deg,
            offset_deg: r.offset_deg.map(Into::into),
            precision_rms_deg: r.precision_rms_deg,
            precision_rms_raw_deg: r.precision_rms_raw_deg,
            sd_deg: r.sd_deg,
        }
    }
}

/// Result of `finish_calibration`.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct GazeCalibrationOutcomeRecord {
    pub quality: GazeCalibrationQuality,
    /// The fitted model is now in effect (quality `good`, `fair` or `poor`). When `false`
    /// the previous model (or none) stays in effect and `advice()` asks to re-calibrate.
    pub applied: bool,
    /// The fitted model as JSON (`CalModel`), absent when no model could be fitted. Store it
    /// as an opaque blob bound to this session and child (CAL-1); never reuse it elsewhere.
    pub model_json: Option<String>,
    /// Why no model could be fitted (log text), absent when one was.
    pub fit_error: Option<String>,
    /// `trackingQuality.calibration` of this attempt, JSON (QS-1).
    pub record_json: String,
    /// Every target window in order.
    pub targets: Vec<GazeTargetResultRecord>,
}

/// Re-calibration advice (CAL-9); act on it only at a clip boundary.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct GazeRecalAdviceRecord {
    pub action: GazeRecalAction,
    pub reasons: Vec<GazeRecalReason>,
    /// A trigger fired but the re-calibration limit is spent.
    pub limit_reached: bool,
}

impl From<RecalAdvice> for GazeRecalAdviceRecord {
    fn from(a: RecalAdvice) -> Self {
        Self {
            action: a.action.into(),
            reasons: a.reasons.into_iter().map(Into::into).collect(),
            limit_reached: a.limit_reached,
        }
    }
}

fn pipeline_error(e: PipelineError) -> AsdError {
    AsdError::InvalidArgument {
        message: e.to_string(),
    }
}

// --- the pipeline handle ---------------------------------------------------------------

/// The per-session gaze pipeline (PIPE-1). Create one per session (CAL-1), own it from one
/// actor, push every ARKit frame (tracked or not, TRK-1), and read the summary at the end.
#[derive(Debug, uniffi::Object)]
pub struct GazePipelineHandle {
    inner: Mutex<GazePipeline>,
}

impl GazePipelineHandle {
    fn lock(&self) -> MutexGuard<'_, GazePipeline> {
        // A poisoned lock only means a previous call panicked; the pipeline state is still a
        // valid value, so keep going rather than panic across the boundary.
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Wraps an existing core pipeline (tests, replay tools).
    pub fn from_pipeline(pipeline: GazePipeline) -> Self {
        Self {
            inner: Mutex::new(pipeline),
        }
    }
}

#[uniffi::export]
impl GazePipelineHandle {
    /// A pipeline for `PipelineConfig` JSON (camelCase; start from
    /// [`gaze_default_config_json`] and set `qualityGate` / `params` from the delivered
    /// `ScoringProfile`).
    #[uniffi::constructor]
    pub fn new(config_json: String) -> Result<Arc<Self>, AsdError> {
        let config: PipelineConfig = decode(&config_json, "pipeline config")?;
        Ok(Arc::new(Self::from_pipeline(GazePipeline::new(config))))
    }

    /// The config this pipeline was created with, JSON.
    pub fn config_json(&self) -> Result<String, AsdError> {
        encode(self.lock().config(), "pipeline config")
    }

    /// Processes one ARKit frame. Every frame yields a sample, with or without a face.
    /// `Err(InvalidArgument)` only for a matrix that is not 16 numbers.
    pub fn push(&self, raw: RawSampleRecord) -> Result<ProcessedSampleRecord, AsdError> {
        let raw = RawTrackingSample::try_from(raw)?;
        Ok(self.lock().push(&raw).into())
    }

    /// Frames pushed while recording count in the summary; turn it on for the stimulus clips
    /// only (off during positioning, calibration and validation).
    pub fn set_recording(&self, on: bool) {
        self.lock().set_recording(on);
    }

    /// Switches the interface orientation (a frame with another orientation also does).
    pub fn set_orientation(&self, orientation: GazeOrientation) {
        self.lock().set_orientation(orientation.into());
    }

    /// Replaces the calibration in effect with a `CalModel` JSON (replay, tests), or removes
    /// it with `None`. Does not count as an in-session calibration.
    pub fn set_calibration_json(&self, model_json: Option<String>) -> Result<(), AsdError> {
        let model: Option<CalModel> = model_json
            .map(|j| decode(&j, "calibration model"))
            .transpose()?;
        self.lock().set_calibration(model);
        Ok(())
    }

    /// The calibration in effect as `CalModel` JSON; `None` without one.
    pub fn calibration_json(&self) -> Result<Option<String>, AsdError> {
        self.lock()
            .calibration()
            .map(|m| encode(m, "calibration model"))
            .transpose()
    }

    /// AOIs of the clip now playing, `[ScreenAoi]` JSON in screen space (from
    /// [`gaze_clip_aois_json`]); `"[]"` stops hit testing.
    pub fn set_clip_aois_json(&self, aois_json: String) -> Result<(), AsdError> {
        let aois: Vec<ScreenAoi> = decode(&aois_json, "screen AOIs")?;
        self.lock().set_clip_aois(aois);
        Ok(())
    }

    /// Starts a (re-)calibration. `layout` defaults to the configured 5-point layout;
    /// `reason` is recorded for a re-calibration (`operatorRequest` when absent).
    pub fn start_calibration(
        &self,
        layout: Option<Vec<GazeVec2>>,
        reason: Option<GazeRecalReason>,
    ) -> Result<(), AsdError> {
        let layout = layout.map(|l| l.into_iter().map(Into::into).collect());
        self.lock()
            .start_calibration(layout, reason.map(Into::into))
            .map_err(pipeline_error)
    }

    /// What the calibration UI should show next; `None` when no calibration is running.
    pub fn calibration_status(&self) -> Option<GazeCalibrationStatus> {
        self.lock().calibration_status().map(Into::into)
    }

    /// Opens a target window at the next pushed frame. A calibration target must be the
    /// pending one of `calibration_status()`.
    pub fn begin_target(&self, kind: GazeTargetKind, target: GazeVec2) -> Result<(), AsdError> {
        self.lock()
            .begin_target(kind.into(), target.into())
            .map_err(pipeline_error)
    }

    /// Whether the open window has collected its frame time (then call `end_target`).
    pub fn target_complete(&self) -> bool {
        self.lock().target_complete()
    }

    /// Closes the open window; `None` without one (or a calibration window with no frame).
    pub fn end_target(&self) -> Option<GazeTargetResultRecord> {
        self.lock().end_target().map(Into::into)
    }

    /// Fits and scores the running calibration and applies it unless it failed.
    pub fn finish_calibration(&self) -> Result<GazeCalibrationOutcomeRecord, AsdError> {
        let mut p = self.lock();
        let outcome = p.finish_calibration().map_err(pipeline_error)?;
        let (model_json, fit_error, applied) = match &outcome.fit {
            Ok(fit) => (
                Some(encode(&fit.selected.model, "calibration model")?),
                None,
                p.calibration() == Some(&fit.selected.model),
            ),
            Err(e) => (None, Some(e.to_string()), false),
        };
        Ok(GazeCalibrationOutcomeRecord {
            quality: outcome.record.quality.into(),
            applied,
            model_json,
            fit_error,
            record_json: encode(&outcome.record, "calibration record")?,
            targets: outcome.targets.into_iter().map(Into::into).collect(),
        })
    }

    /// Validation of the current calibration (`ValidationRecord` JSON); `None` before any
    /// validation target.
    pub fn validation_json(&self) -> Result<Option<String>, AsdError> {
        self.lock()
            .validation()
            .map(|v| encode(&v, "validation record"))
            .transpose()
    }

    /// Every drift check of the session, `[DriftCheckRecord]` JSON.
    pub fn drift_checks_json(&self) -> Result<String, AsdError> {
        encode(&self.lock().drift_checks(), "drift checks")
    }

    /// Where to show the next drift check target.
    pub fn next_check_target(&self) -> GazeVec2 {
        self.lock().next_check_target().into()
    }

    /// Re-calibration advice (CAL-9, VAL-4).
    pub fn advice(&self) -> GazeRecalAdviceRecord {
        self.lock().advice().into()
    }

    /// GAP-1 gap filling over processed frames (for the frames file), with the session
    /// geometry. Filled frames have `gap_filled = true` and stay invalid.
    pub fn gap_fill(&self, samples: Vec<ProcessedSampleRecord>) -> Vec<ProcessedSampleRecord> {
        let samples: Vec<ProcessedSample> = samples.into_iter().map(Into::into).collect();
        self.lock()
            .gap_fill(&samples)
            .into_iter()
            .map(Into::into)
            .collect()
    }

    /// `validRatio` of the recorded frames.
    pub fn valid_ratio(&self) -> f64 {
        self.lock().valid_ratio()
    }

    /// Share of recorded frame time not `tracked`.
    pub fn data_loss(&self) -> f64 {
        self.lock().data_loss()
    }

    /// The session `trackingQuality` (QS-1), JSON, camelCase.
    pub fn summary_json(&self) -> Result<String, AsdError> {
        encode(&self.lock().summary(), "tracking quality")
    }
}

// --- frames-file writer ----------------------------------------------------------------

/// One frames-file sample (`GazeTelemetrySample`) with its CORE-7 gate result.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct GazeTelemetryRecord {
    pub timestamp_ms: f64,
    pub video_time_sec: f64,
    pub x_norm: f64,
    pub y_norm: f64,
    pub confidence: f64,
    pub left_eye_openness: f64,
    pub right_eye_openness: f64,
    pub head_pitch_rad: f64,
    pub head_yaw_rad: f64,
    pub head_roll_rad: f64,
    pub distance_to_screen_cm: f64,
    pub is_valid: bool,
    pub stimulus_id: Option<String>,
    pub clip_media_time_sec: Option<f64>,
    /// `GazeQualityVerdict` raw value (`optimal`, `acceptable`, `degraded`, `rejected`).
    pub quality_verdict: String,
    pub failure_reasons: Vec<String>,
    pub is_blink: bool,
}

/// Converts processed frames to frames-file samples, holding the last known values for
/// frames without a face (trap 6: never zeros). One per session, from the same actor.
#[derive(Debug, uniffi::Object)]
pub struct GazeTelemetryWriterHandle {
    inner: Mutex<TelemetryWriter>,
}

#[uniffi::export]
impl GazeTelemetryWriterHandle {
    /// `origin_sec`: the session clock origin (frame clock). `quality_gate_json`: the
    /// `ScoringProfile.qualityGate` object; `None` for the default gate.
    #[uniffi::constructor]
    pub fn new(origin_sec: f64, quality_gate_json: Option<String>) -> Result<Arc<Self>, AsdError> {
        let gate: QualityGate = match quality_gate_json {
            Some(j) => decode(&j, "quality gate")?,
            None => QualityGate::default(),
        };
        Ok(Arc::new(Self {
            inner: Mutex::new(TelemetryWriter::new(origin_sec, gate)),
        }))
    }

    /// The frames-file sample of one processed frame, gated.
    pub fn write(&self, sample: ProcessedSampleRecord) -> GazeTelemetryRecord {
        let mut w = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let s: GazeTelemetrySample = w.sample(&sample.into());
        let e = w.evaluate(&s);
        let verdict = serde_json::to_value(e.verdict)
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_default();
        GazeTelemetryRecord {
            timestamp_ms: s.timestamp_ms,
            video_time_sec: s.video_time_sec,
            x_norm: s.x_norm,
            y_norm: s.y_norm,
            confidence: s.confidence,
            left_eye_openness: s.left_eye_openness,
            right_eye_openness: s.right_eye_openness,
            head_pitch_rad: s.head_pitch_rad,
            head_yaw_rad: s.head_yaw_rad,
            head_roll_rad: s.head_roll_rad,
            distance_to_screen_cm: s.distance_to_screen_cm,
            is_valid: s.is_valid,
            stimulus_id: s.stimulus_id,
            clip_media_time_sec: s.clip_media_time_sec,
            quality_verdict: verdict,
            failure_reasons: e.failure_reasons,
            is_blink: e.is_blink,
        }
    }
}

// --- free functions --------------------------------------------------------------------

/// `PipelineConfig` JSON for a hardware model (`utsname.machine`, e.g. `iPhone15,2`): the
/// table geometry or the idiom default (GEO-2), default gate and params.
#[uniffi::export]
pub fn gaze_default_config_json(
    model_identifier: String,
    orientation: GazeOrientation,
) -> Result<String, AsdError> {
    encode(
        &PipelineConfig::for_model(&model_identifier, orientation.into()),
        "pipeline config",
    )
}

/// A clip's catalog areas (`[StimulusCatalog.Area]` JSON) as screen-space AOIs
/// (`[ScreenAoi]` JSON) for [`GazePipelineHandle::set_clip_aois_json`]. `Screen` passes boxes
/// through (today's catalog, trap 11); `Video` maps them and drops the ones cropped away.
#[uniffi::export]
pub fn gaze_clip_aois_json(
    areas_json: String,
    aoi_space: GazeAoiSpace,
    video_aspect: f64,
    screen_aspect: f64,
    gravity: GazeVideoGravity,
) -> Result<String, AsdError> {
    let areas: Vec<Area> = decode(&areas_json, "catalog areas")?;
    let aois = aoi_space::clip_aois(
        &areas,
        aoi_space.into(),
        video_aspect,
        screen_aspect,
        gravity.into(),
    );
    encode(&aois, "screen AOIs")
}
