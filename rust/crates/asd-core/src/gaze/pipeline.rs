//! owner: WP-G.3
//!
//! The stateful, single-owner per-frame gaze pipeline (PIPE-1, PIPE-5), tying together the
//! raycast (G.1), calibration (G.2), validation, drift and the session summary (G.4).
//!
//! # Order inside [`GazePipeline::push`]
//! 1. **Ocular track state** (`lost`, `blink` with the time-based recovery) — needs no ray;
//! 2. **Raycast** (GEO-5) of the raw sample, unclamped;
//! 3. **Calibration apply** (`CalModel::apply_sample`; identity without a calibration);
//! 4. **Geometric track state** on the *calibrated* ray and point (`artifact`, `offscreen`,
//!    TRK-2), completing TRK-1;
//! 5. **Confidence** (CNF-1), written into the sample (CNF-4); `valid` = `tracked` and
//!    confidence ≥ `qualityGate.minConfidence`, which equals the CORE-7 gate verdict;
//! 6. **1-Euro filter** on `tracked` samples only, in sample time (PIPE-3);
//! 7. **AOI hit test** of the filtered point on `tracked` samples, in screen space (GEO-7).
//!
//! Steps 1 and 4 are the two halves of `track_state::TrackStateMachine`; splitting them lets
//! the ray be cast once and keeps the stated order (state, raycast, calibration, confidence,
//! filter, hit test) for everything that consumes the state.
//!
//! # Decisions recorded here
//! - **Calibration samples are judged on the raw ray.** Before calibration the raw point of
//!   an edge target is often off the glass (raw error 3–6 cm, G-F1), so the frame's own state
//!   is `offscreen`. The [`CalibrationSession`] gets a copy of the frame whose state and
//!   confidence are recomputed from the raw ray without the off-screen rule; otherwise the
//!   calibration would lose exactly the samples it exists to correct.
//! - **Target windows start at the next pushed frame and end at the last pushed frame** (frame
//!   clock only, PIPE-5). Calibration windows close themselves (`CalibrationSession`);
//!   validation and drift-check windows are complete after `settleMs + collectMs` of frame
//!   time ([`GazePipeline::target_complete`]) and are judged by `validation::target_result`.
//! - **A calibration is applied only when its quality is `good`, `fair` or `poor`.** A
//!   `failed` attempt (or one that could not be fitted) is not applied. When an in-session
//!   calibration is in effect, it stays in effect *with everything that describes it*: the
//!   summary's calibration record (model, quality, pose, so QS-5 `calibratedDistanceCm`),
//!   its orientation and age, its validation and the drift reference; the failed attempt
//!   appears only as a `recalibrations[]` entry (`newQuality: failed`). Without one (first
//!   attempt), the failed attempt becomes the calibration record and advice asks for a
//!   re-calibration (CAL-9.1). `attempts` counts every attempt of the session; each attempt
//!   after the first is a `recalibrations[]` entry with the reason given to
//!   [`GazePipeline::start_calibration`] (`operatorRequest` when none). An applied new
//!   calibration clears the validation of the previous one, calls
//!   `DriftMonitor::on_recalibrated`, and resets the 1-Euro filter (the calibrated space
//!   jumps); the validation that follows sets the drift reference.
//! - **Calibration samples carry no pose-shift factor.** Their confidence (CAL-2 gate) is
//!   CNF-1 without the distance-shift-from-calibration factor, because a new calibration
//!   defines a new pose; judging it against the pose it replaces would reject every sample
//!   after a move of more than `poseShiftDistancePct`.
//! - **An orientation change while a calibration runs rejects it** (GEO-3, CAL-9.5): the
//!   `CalibrationSession` holds the geometry of the orientation it started in, so the attempt
//!   is scored `failed` at [`GazePipeline::finish_calibration`] and is not applied.
//! - **The summary covers recorded frames only** ([`GazePipeline::set_recording`]): loss,
//!   `validRatio`, data loss, confidence, conditions and the sample rate. Recording is off
//!   until the native app turns it on for the stimulus clips; frames during positioning,
//!   calibration and validation do not count. `confidence.meanSample` and
//!   `lowConfidenceSampleRatio` are over `tracked` recorded frames (loss is reported in
//!   `loss`, not double-counted as zero confidence); with no tracked frame they are 0 and 1.
//!   `meanFixation` is the mean of the fixation confidences of [`gapfill::fixations`] (I-VT in
//!   degrees per second at `scoring.ivtVelocityDegPerSec`, gap-filled samples joining runs,
//!   80 ms minimum as `metrics`); scoring's own I-VT is untouched (SCR-3 is WP-G.13).
//! - **The raw sample's orientation wins.** A frame whose `orientation` differs from the
//!   current one switches the pipeline to it ([`GazePipeline::set_orientation`]), so replayed
//!   logs and live capture behave the same; the calibration's orientation is kept for the
//!   CAL-9 `orientationChanged` trigger.
//! - **Distance** is the perpendicular distance of the face anchor from the screen plane, as
//!   the Swift actor computed it (`-faceTransform.z`), without its 20 cm floor; head yaw, pitch
//!   and roll use the Swift actor's formulas. Head speed is the rotation angle between
//!   consecutive face orientations per second of frame time; eye speed the angle between
//!   consecutive eye-in-head directions (`calibration::gaze_angles_deg`). Both are absent on
//!   the first face frame after a frame without a face.
//! - A missing blink blend shape counts as an open eye (Swift: `?? 0.0`).
//! - Memory: one compact record per recorded frame (about 120 bytes; 10 minutes at 60 Hz is
//!   about 4 MB), the buffered frames of an open validation or check window, and the last
//!   `recalibration.poseWindowSec` of tracked poses.

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::aoi_space::{active_aoi_names, ScreenAoi};
use super::calibration::{
    gaze_angles_deg, CalModel, CalSessionError, CalibrationOutcome, CalibrationSession, SampleGate,
    TargetStatus,
};
use super::confidence::{self, ConfidenceFeatures, ConfidenceLimits};
use super::drift::{self, AdviceState, DriftMonitor};
use super::filter::OneEuro2D;
use super::gapfill::{self, GapSample, LossFrame};
use super::geometry::{self, angle_deg, OrientedGeometry};
use super::raycast::{raycast, RayHit, CAMERA_AXES};
use super::summary::{self, SummaryInput};
use super::track_state::{geometric_state, TrackStateMachine};
use super::validation::{self, median, quantile, TargetWindow};
use crate::metrics::MIN_FIXATION_DURATION_SEC;
use crate::quality::{self, Evaluation, QualityGate};
use crate::types::tracking::{
    CalModelKind, CalibrationQuality, CalibrationRecord, ConfidenceSummary, DeviceGeometry,
    DriftCheckRecord, GeometrySource, LossSummary, Orientation, ProcessedSample, RawTrackingSample,
    RecalAdvice, RecalReason, RecalibrationRecord, TargetKind, TargetOutcome, TargetResult,
    TrackState, TrackingConditions, TrackingDevice, TrackingQuality, TrackingQualityParams,
    ValidationRecord,
};
use crate::types::GazeTelemetrySample;

/// Tolerance on time-window edges, ms.
const EPS_MS: f64 = 1e-6;
/// Two targets closer than this (screen-normalised) are the same target.
const TARGET_EPS: f64 = 1e-9;

/// Everything a session's pipeline needs (trap 12: one config per session, passed
/// everywhere). JSON across the FFI (ADR-0007 D2), camelCase; the gate and the params default
/// when absent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PipelineConfig {
    pub model_identifier: String,
    pub geometry_source: GeometrySource,
    /// Portrait native geometry (the pipeline rotates it, GEO-3).
    pub geometry: DeviceGeometry,
    pub orientation: Orientation,
    /// `ScoringProfile.qualityGate` (distance, pose, blink and `minConfidence` limits).
    #[serde(default)]
    pub quality_gate: QualityGate,
    /// `ScoringProfile.trackingQuality`.
    #[serde(default)]
    pub params: TrackingQualityParams,
    /// A calibration to apply from the start (replay, tests). Normal sessions calibrate in
    /// the pipeline (CAL-1: no reuse across sessions).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calibration: Option<CalModel>,
    /// Operator answer at positioning (CNF-2); absent when not asked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub glasses: Option<bool>,
}

impl PipelineConfig {
    /// Config for a hardware model with the table geometry (or the idiom default, GEO-2) and
    /// default gate and params.
    pub fn for_model(model_identifier: &str, orientation: Orientation) -> Self {
        let found = geometry::lookup(model_identifier);
        Self {
            model_identifier: found.model_identifier,
            geometry_source: found.source,
            geometry: found.geometry,
            orientation,
            quality_gate: QualityGate::default(),
            params: TrackingQualityParams::default(),
            calibration: None,
            glasses: None,
        }
    }
}

/// Misuse of the target API.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum PipelineError {
    #[error("a target window is already open")]
    TargetOpen,
    #[error("no calibration is running; call start_calibration first")]
    NoCalibrationSession,
    #[error("every calibration target has been shown")]
    CalibrationComplete,
    #[error("calibration expects target {expected:?}, got {got:?}")]
    TargetMismatch { expected: [f64; 2], got: [f64; 2] },
}

/// An open target window.
#[derive(Debug, Clone)]
struct OpenTarget {
    kind: TargetKind,
    target: [f64; 2],
    started_at_sec: Option<f64>,
    /// Validation and drift-check windows: every frame since the window opened.
    samples: Vec<ProcessedSample>,
    /// Calibration windows: the result once the session closed the window.
    result: Option<TargetResult>,
}

/// One recorded frame, compact (see the module docs).
#[derive(Debug, Clone, Copy)]
struct Rec {
    t_sec: f64,
    state: TrackState,
    valid: bool,
    confidence: f64,
    /// Filtered point (tracked frames only).
    point: Option<[f64; 2]>,
    eye: Option<[f64; 3]>,
    distance_cm: Option<f64>,
    head_speed: Option<f64>,
    ambient: Option<f64>,
}

/// The latest calibration attempt.
#[derive(Debug, Clone)]
struct CalState {
    record: CalibrationRecord,
    at_sec: f64,
    orientation: Orientation,
}

/// The per-session gaze pipeline (PIPE-1). Single owner, `Send`, not `Sync` by design (trap
/// 10: the FFI handle wraps it in a `Mutex`).
#[derive(Debug, Clone)]
pub struct GazePipeline {
    config: PipelineConfig,
    orientation: Orientation,
    geo: OrientedGeometry,
    limits: ConfidenceLimits,
    track: TrackStateMachine,
    filter: OneEuro2D,
    calibration: Option<CalModel>,
    cal_session: Option<CalibrationSession>,
    cal_reason: Option<RecalReason>,
    /// The orientation changed while the running calibration collected (L3, GEO-3).
    cal_orientation_changed: bool,
    cal_state: Option<CalState>,
    cal_attempts: u32,
    recalibrations: Vec<RecalibrationRecord>,
    validation_targets: Vec<TargetResult>,
    drift: DriftMonitor,
    target: Option<OpenTarget>,
    aois: Vec<ScreenAoi>,
    clip_origin_sec: Option<f64>,
    recording: bool,
    first_recorded_sec: Option<f64>,
    age_at_start_sec: Option<f64>,
    recorded: Vec<Rec>,
    recent_poses: VecDeque<ProcessedSample>,
    last_t_sec: Option<f64>,
    prev_face: Option<(f64, [f64; 16])>,
    prev_gaze_dir: Option<(f64, [f64; 3])>,
}

impl GazePipeline {
    pub fn new(config: PipelineConfig) -> Self {
        let geo = config.geometry.rotate(config.orientation);
        Self {
            orientation: config.orientation,
            geo,
            limits: ConfidenceLimits::new(&config.quality_gate, &config.params),
            track: TrackStateMachine::new(
                config.params.track_state,
                config.quality_gate.blink_threshold,
            ),
            filter: OneEuro2D::new(config.params.filter),
            calibration: config.calibration.clone(),
            cal_session: None,
            cal_reason: None,
            cal_orientation_changed: false,
            cal_state: None,
            cal_attempts: 0,
            recalibrations: Vec::new(),
            validation_targets: Vec::new(),
            drift: DriftMonitor::new(config.params.drift.clone()),
            target: None,
            aois: Vec::new(),
            clip_origin_sec: None,
            recording: false,
            first_recorded_sec: None,
            age_at_start_sec: None,
            recorded: Vec::new(),
            recent_poses: VecDeque::new(),
            last_t_sec: None,
            prev_face: None,
            prev_gaze_dir: None,
            config,
        }
    }

    pub fn config(&self) -> &PipelineConfig {
        &self.config
    }

    /// The session geometry in the current orientation (use it for every angle, trap 12).
    pub fn geometry(&self) -> &OrientedGeometry {
        &self.geo
    }

    /// The calibration in effect.
    pub fn calibration(&self) -> Option<&CalModel> {
        self.calibration.as_ref()
    }

    /// Replaces the calibration in effect (replay, FFI). The session's calibration record is
    /// not changed: without an in-session calibration it stays `skipped`. Resets the filter.
    pub fn set_calibration(&mut self, model: Option<CalModel>) {
        self.calibration = model;
        self.filter.reset();
    }

    /// Switches the interface orientation (GEO-3): rotates the geometry and resets the filter.
    /// The calibration stays in effect; `advice()` reports `orientationChanged` (CAL-9).
    pub fn set_orientation(&mut self, orientation: Orientation) {
        if orientation == self.orientation {
            return;
        }
        self.orientation = orientation;
        self.geo = self.config.geometry.rotate(orientation);
        self.filter.reset();
        if self.cal_session.is_some() {
            self.cal_orientation_changed = true;
        }
    }

    /// Frames pushed while recording count in the session summary.
    pub fn set_recording(&mut self, on: bool) {
        self.recording = on;
    }

    /// AOIs of the clip now playing, in screen space (`aoi_space::clip_aois`). Their times are
    /// clip media time; frames without `clip_media_time_sec` use the time since the first
    /// frame after this call (Swift fallback). An empty list stops hit testing.
    pub fn set_clip_aois(&mut self, aois: Vec<ScreenAoi>) {
        self.aois = aois;
        self.clip_origin_sec = None;
    }

    // --------------------------------------------------------------------------------------
    // Per frame
    // --------------------------------------------------------------------------------------

    /// Processes one ARKit frame (module docs for the order). Every frame yields a sample,
    /// with or without a face (TRK-1).
    pub fn push(&mut self, raw: &RawTrackingSample) -> ProcessedSample {
        if raw.orientation != self.orientation {
            self.set_orientation(raw.orientation);
        }
        let t = raw.t_sec;
        let face = raw.face_transform.filter(|_| raw.face_tracked);
        let openness = |b: Option<f64>| (1.0 - b.unwrap_or(0.0)).clamp(0.0, 1.0);
        let (lo, ro) = (openness(raw.eye_blink_left), openness(raw.eye_blink_right));
        let ocular = self.track.ocular_state(t, face.is_some(), lo, ro);

        let mut out = ProcessedSample {
            t_sec: t,
            track_state: TrackState::Lost,
            ray_status: None,
            raw_point_norm: None,
            calibrated_point_norm: None,
            filtered_point_norm: None,
            eye_pos_mm: None,
            distance_cm: None,
            head_yaw_rad: None,
            head_pitch_rad: None,
            head_roll_rad: None,
            head_speed_deg_per_sec: None,
            eye_speed_deg_per_sec: None,
            left_eye_openness: None,
            right_eye_openness: None,
            ambient_intensity: raw.ambient_intensity,
            confidence: 0.0,
            valid: false,
            gap_filled: false,
            active_aoi_names: Vec::new(),
            stimulus_id: raw.stimulus_id.clone(),
            clip_media_time_sec: raw.clip_media_time_sec,
        };

        let Some(face) = face else {
            self.prev_face = None;
            self.prev_gaze_dir = None;
            self.finish_frame(&out, raw, None);
            return out;
        };

        // Pose and speeds.
        out.left_eye_openness = Some(lo);
        out.right_eye_openness = Some(ro);
        // Swift formulas; `+ 0.0` turns a negative zero into 0 so records never say "-0.0".
        out.head_pitch_rad = Some((-face[9]).clamp(-1.0, 1.0).asin() + 0.0);
        out.head_yaw_rad = Some(face[8].atan2(face[10]) + 0.0);
        out.head_roll_rad = Some(face[1].atan2(face[5]) + 0.0);
        out.distance_cm = Some(CAMERA_AXES.z_viewer_side * face[14] * 100.0);
        out.head_speed_deg_per_sec = self
            .prev_face
            .and_then(|(pt, pf)| per_sec(rotation_angle_deg(&pf, &face), t - pt));
        self.prev_face = Some((t, face));
        let gaze_dir = gaze_angles_deg(raw).map(dir_of_angles);
        out.eye_speed_deg_per_sec = match (self.prev_gaze_dir, gaze_dir) {
            (Some((pt, pd)), Some(d)) => per_sec(angle_deg(pd, d, [0.0; 3]), t - pt),
            _ => None,
        };
        self.prev_gaze_dir = gaze_dir.map(|d| (t, d));

        // Raycast, then calibration.
        let raw_hit = raycast(raw, &self.geo);
        let cal_hit = match &self.calibration {
            Some(m) => m.apply_sample(raw, &self.geo),
            None => raw_hit,
        };
        out.ray_status = cal_hit.map(|h| h.status);
        out.raw_point_norm = raw_hit.and_then(|h| h.point_norm);
        out.calibrated_point_norm = cal_hit.and_then(|h| h.point_norm);
        out.eye_pos_mm = raw_hit.and_then(|h| h.eye_pos_mm);

        // Track state on the calibrated ray (TRK-2), then confidence (CNF-1, CNF-4).
        out.track_state = ocular.unwrap_or_else(|| {
            geometric_state(
                out.ray_status,
                out.calibrated_point_norm,
                out.eye_speed_deg_per_sec,
                self.track.params(),
                true,
            )
        });
        out.confidence =
            self.confidence_of(&out, out.track_state, self.pose_shift_pct(out.distance_cm));
        out.valid = self.is_valid(out.track_state, out.confidence);

        // Filter (tracked only, sample time), then the AOI hit test.
        if out.track_state == TrackState::Tracked {
            if let Some(p) = out.calibrated_point_norm {
                let f = self.filter.filter(p, t);
                out.filtered_point_norm = Some(f);
                if !self.aois.is_empty() {
                    let origin = *self.clip_origin_sec.get_or_insert(t);
                    let clip_t = raw.clip_media_time_sec.unwrap_or((t - origin).max(0.0));
                    out.active_aoi_names = active_aoi_names(&self.aois, f, clip_t);
                }
            }
        }

        self.finish_frame(&out, raw, Some((ocular, raw_hit)));
        out
    }

    fn is_valid(&self, state: TrackState, confidence: f64) -> bool {
        state == TrackState::Tracked && confidence >= self.config.quality_gate.min_confidence
    }

    /// CNF-1 for a frame judged with `state` and the given pose shift.
    fn confidence_of(
        &self,
        p: &ProcessedSample,
        state: TrackState,
        pose_shift_pct: Option<f64>,
    ) -> f64 {
        let mut f = ConfidenceFeatures::from_processed(p, pose_shift_pct);
        f.track_state = state;
        confidence::sample(&f, &self.limits)
    }

    /// Distance shift from the calibration pose, percent; screen models only (CNF-1).
    fn pose_shift_pct(&self, distance_cm: Option<f64>) -> Option<f64> {
        let screen_model = self
            .calibration
            .as_ref()
            .is_some_and(|m| m.kind() != CalModelKind::AngularAffine);
        if !screen_model {
            return None;
        }
        let cal = self.cal_state.as_ref()?.record.pose?.distance_cm;
        let d = distance_cm?;
        (cal > 0.0).then(|| (d - cal).abs() * 100.0 / cal)
    }

    /// Bookkeeping after a frame: target windows, recording, recent poses.
    fn finish_frame(
        &mut self,
        out: &ProcessedSample,
        raw: &RawTrackingSample,
        face_parts: Option<(Option<TrackState>, Option<RayHit>)>,
    ) {
        let t = out.t_sec;
        self.last_t_sec = Some(t);

        if self.target.is_some() {
            self.feed_target(out, raw, face_parts);
        }

        if self.recording {
            if self.first_recorded_sec.is_none() {
                self.first_recorded_sec = Some(t);
                self.age_at_start_sec = self.cal_state.as_ref().map(|c| t - c.at_sec);
            }
            self.recorded.push(Rec {
                t_sec: t,
                state: out.track_state,
                valid: out.valid,
                confidence: out.confidence,
                point: out.filtered_point_norm,
                eye: out.eye_pos_mm,
                distance_cm: out.distance_cm,
                head_speed: out.head_speed_deg_per_sec,
                ambient: out.ambient_intensity,
            });
        }

        if out.track_state == TrackState::Tracked {
            self.recent_poses.push_back(pose_only(out));
        }
        let window = self.config.params.recalibration.pose_window_sec;
        while self
            .recent_poses
            .front()
            .is_some_and(|p| p.t_sec < t - window)
        {
            self.recent_poses.pop_front();
        }
    }

    fn feed_target(
        &mut self,
        out: &ProcessedSample,
        raw: &RawTrackingSample,
        face_parts: Option<(Option<TrackState>, Option<RayHit>)>,
    ) {
        let t = out.t_sec;
        let Some(kind) = self.target.as_ref().map(|o| o.kind) else {
            return;
        };
        let just_started = self
            .target
            .as_ref()
            .is_some_and(|o| o.started_at_sec.is_none());
        if kind != TargetKind::Calibration {
            if let Some(open) = self.target.as_mut() {
                open.started_at_sec.get_or_insert(t);
                open.samples.push(out.clone());
            }
            return;
        }
        if self.target.as_ref().is_some_and(|o| o.result.is_some()) {
            return;
        }
        let cal_view = self.calibration_view(out, face_parts);
        let Some(session) = self.cal_session.as_mut() else {
            return;
        };
        if just_started {
            // Cannot fail: the pending target was checked in begin_target and no window is
            // open in the session.
            let _ = session.begin_target(t);
        }
        let result = session.push(&cal_view, Some(raw));
        if let Some(open) = self.target.as_mut() {
            open.started_at_sec.get_or_insert(t);
            if result.is_some() {
                open.result = result;
            }
        }
    }

    /// The frame as the calibration sees it: state and confidence from the raw ray, without
    /// the off-screen rule and without the CNF-1 pose-shift factor (module docs).
    fn calibration_view(
        &self,
        out: &ProcessedSample,
        face_parts: Option<(Option<TrackState>, Option<RayHit>)>,
    ) -> ProcessedSample {
        let mut p = out.clone();
        if let Some((ocular, raw_hit)) = face_parts {
            let state = ocular.unwrap_or_else(|| {
                geometric_state(
                    raw_hit.map(|h| h.status),
                    raw_hit.and_then(|h| h.point_norm),
                    out.eye_speed_deg_per_sec,
                    self.track.params(),
                    false,
                )
            });
            p.track_state = state;
            // No pose-shift factor: the samples of a calibration define its pose, so they are
            // never judged against the pose of the calibration they replace.
            p.confidence = self.confidence_of(out, state, None);
            p.valid = self.is_valid(state, p.confidence);
        }
        p
    }

    // --------------------------------------------------------------------------------------
    // Targets (PIPE-1: calibration, validation and drift checks share this API)
    // --------------------------------------------------------------------------------------

    /// Starts a (re-)calibration with `layout` (default `calibration.layout5`). `reason` is
    /// recorded when this is not the session's first calibration.
    pub fn start_calibration(
        &mut self,
        layout: Option<Vec<[f64; 2]>>,
        reason: Option<RecalReason>,
    ) -> Result<(), PipelineError> {
        if self.target.is_some() {
            return Err(PipelineError::TargetOpen);
        }
        let params = &self.config.params;
        let layout = layout.unwrap_or_else(|| params.calibration.layout5.clone());
        self.cal_session = Some(CalibrationSession::new(
            layout,
            params.calibration.clone(),
            SampleGate::from_params(&self.config.quality_gate, &params.confidence),
            self.geo,
        ));
        self.cal_reason = reason;
        self.cal_orientation_changed = false;
        Ok(())
    }

    /// What the calibration UI should show next; `None` when no calibration is running.
    pub fn calibration_status(&self) -> Option<TargetStatus> {
        self.cal_session.as_ref().map(CalibrationSession::status)
    }

    /// Gives up on the pending calibration target (operator skip, or a replayed log that never
    /// re-showed a target the pipeline wants to retry): it is closed without samples at the
    /// last frame time, through its remaining retries, so it ends `missed` (CAL-3) and the
    /// next target becomes pending. Returns the final (`missed`) result.
    pub fn skip_calibration_target(&mut self) -> Result<TargetResult, PipelineError> {
        if self.target.is_some() {
            return Err(PipelineError::TargetOpen);
        }
        let t = self.last_t_sec.unwrap_or(0.0);
        let session = self
            .cal_session
            .as_mut()
            .ok_or(PipelineError::NoCalibrationSession)?;
        loop {
            session.begin_target(t).map_err(|e| match e {
                CalSessionError::AlreadyRunning => PipelineError::TargetOpen,
                CalSessionError::Complete => PipelineError::CalibrationComplete,
            })?;
            let r = session
                .end_target(t)
                .ok_or(PipelineError::CalibrationComplete)?;
            if r.outcome != TargetOutcome::NeedsRetry {
                return Ok(r);
            }
        }
    }

    /// Opens a target window at the next pushed frame. For `calibration` the target must be
    /// the calibration's pending target.
    pub fn begin_target(
        &mut self,
        kind: TargetKind,
        target: [f64; 2],
    ) -> Result<(), PipelineError> {
        if self.target.is_some() {
            return Err(PipelineError::TargetOpen);
        }
        if kind == TargetKind::Calibration {
            let status = self
                .calibration_status()
                .ok_or(PipelineError::NoCalibrationSession)?;
            match status {
                TargetStatus::Pending { target_norm, .. } => {
                    let same = (target_norm[0] - target[0]).abs() <= TARGET_EPS
                        && (target_norm[1] - target[1]).abs() <= TARGET_EPS;
                    if !same {
                        return Err(PipelineError::TargetMismatch {
                            expected: target_norm,
                            got: target,
                        });
                    }
                }
                TargetStatus::Running { .. } => return Err(PipelineError::TargetOpen),
                TargetStatus::Complete => return Err(PipelineError::CalibrationComplete),
            }
        }
        self.target = Some(OpenTarget {
            kind,
            target,
            started_at_sec: None,
            samples: Vec::new(),
            result: None,
        });
        Ok(())
    }

    /// Whether the open window has collected its time: a calibration window that the session
    /// closed, or a validation / check window with `settleMs + collectMs` of frame time.
    pub fn target_complete(&self) -> bool {
        let Some(open) = &self.target else {
            return false;
        };
        let p = &self.config.params;
        let window_ms = match open.kind {
            TargetKind::Calibration => return open.result.is_some(),
            TargetKind::Validation => p.validation.settle_ms + p.validation.collect_ms,
            TargetKind::DriftCheck => p.drift.settle_ms + p.drift.collect_ms,
        };
        match (open.started_at_sec, self.last_t_sec) {
            (Some(s), Some(t)) => (t - s) * 1000.0 >= window_ms - EPS_MS,
            _ => false,
        }
    }

    /// Closes the open window and returns its result (`None` without an open window, or for
    /// a calibration window that never saw a frame). Validation results become the drift
    /// reference; drift-check results go to the drift monitor (DRF-2, DRF-3).
    pub fn end_target(&mut self) -> Option<TargetResult> {
        let open = self.target.take()?;
        let last = self.last_t_sec.unwrap_or(0.0);
        let p = &self.config.params;
        match open.kind {
            TargetKind::Calibration => open
                .result
                .or_else(|| self.cal_session.as_mut()?.end_target(last)),
            kind => {
                let start = open.started_at_sec.unwrap_or(last);
                let end = if open.started_at_sec.is_some() {
                    last
                } else {
                    start
                };
                let window = if kind == TargetKind::Validation {
                    TargetWindow::validation(open.target, start, end, &p.validation)
                } else {
                    TargetWindow::drift_check(open.target, start, end, &p.drift)
                };
                let r = validation::target_result(&window, &open.samples, &self.geo);
                if kind == TargetKind::Validation {
                    self.validation_targets.push(r.clone());
                    self.drift.set_reference(&self.validation_targets);
                } else {
                    self.drift.record_check(&r);
                }
                Some(r)
            }
        }
    }

    /// Fits and scores the running calibration (CAL-5, CAL-6) and applies it when its quality
    /// is not `failed` (module docs).
    pub fn finish_calibration(&mut self) -> Result<CalibrationOutcome, PipelineError> {
        if self.target.is_some() {
            return Err(PipelineError::TargetOpen);
        }
        let session = self
            .cal_session
            .take()
            .ok_or(PipelineError::NoCalibrationSession)?;
        let mut outcome = session.finish();
        if std::mem::take(&mut self.cal_orientation_changed) {
            // L3 / GEO-3: the session's geometry no longer matches the screen.
            outcome.record.quality = CalibrationQuality::Failed;
        }
        let at = self.last_t_sec.unwrap_or(0.0);
        self.cal_attempts += 1;
        let quality = outcome.record.quality;
        if self.cal_attempts > 1 {
            self.recalibrations.push(RecalibrationRecord {
                at_sec: at,
                reason: self
                    .cal_reason
                    .take()
                    .unwrap_or(RecalReason::OperatorRequest),
                new_quality: quality,
            });
        }
        let applied = match (&outcome.fit, quality == CalibrationQuality::Failed) {
            (Ok(fit), false) => Some(fit.selected.model.clone()),
            _ => None,
        };
        let keeps_applied = self
            .cal_state
            .as_ref()
            .is_some_and(|c| c.record.quality != CalibrationQuality::Failed);
        let Some(model) = applied else {
            if keeps_applied {
                // M1: the calibration in effect stays in effect with its record, pose,
                // orientation, age, validation and drift reference; the failed attempt is
                // recorded in `recalibrations[]` only.
                return Ok(outcome);
            }
            // No in-session calibration to keep: the failed attempt is the record.
            self.cal_state = Some(CalState {
                record: outcome.record.clone(),
                at_sec: at,
                orientation: self.orientation,
            });
            self.validation_targets.clear();
            self.drift.on_recalibrated();
            self.filter.reset();
            return Ok(outcome);
        };
        self.calibration = Some(model);
        self.cal_state = Some(CalState {
            record: outcome.record.clone(),
            at_sec: at,
            orientation: self.orientation,
        });
        self.validation_targets.clear();
        self.drift.on_recalibrated();
        self.filter.reset();
        Ok(outcome)
    }

    /// Validation of the current calibration (VAL-2, VAL-3); `None` before any target.
    pub fn validation(&self) -> Option<ValidationRecord> {
        validation::summarize(&self.validation_targets, &self.config.params.validation)
    }

    /// Every drift check of the session.
    pub fn drift_checks(&self) -> &[DriftCheckRecord] {
        self.drift.checks()
    }

    /// The DRF-2 drift reference (mean validation offset over hit targets, degrees) of the
    /// calibration in effect. `None` before a validation, or when no validation target was
    /// hit: drift checks then record no `driftDeg` and the advice asks for a re-calibration.
    pub fn drift_reference_deg(&self) -> Option<[f64; 2]> {
        self.drift.reference_offset_deg()
    }

    /// Position of the next drift check target (DRF-1).
    pub fn next_check_target(&self) -> [f64; 2] {
        self.drift.next_check_target()
    }

    /// CAL-9 / VAL-4 re-calibration advice; act on it only at a clip boundary.
    pub fn advice(&self) -> RecalAdvice {
        let last = self.last_t_sec.unwrap_or(0.0);
        let window = self.config.params.recalibration.pose_window_sec;
        let recent: Vec<ProcessedSample> = self.recent_poses.iter().cloned().collect();
        let in_battery = self.first_recorded_sec.is_some();
        let in_battery_count = self.first_recorded_sec.map_or(0, |s| {
            self.recalibrations.iter().filter(|r| r.at_sec >= s).count()
        });
        let validation = self.validation();
        let state = AdviceState {
            calibration_quality: self.cal_state.as_ref().map(|c| c.record.quality),
            calibration_model: self.calibration.as_ref().map(CalModel::kind),
            validation_accuracy_deg: validation.as_ref().and_then(|v| v.accuracy_deg),
            validation_complete: validation.as_ref().map(|v| v.complete),
            drift_triggered: self.drift.triggered(),
            calibration_pose: self.cal_state.as_ref().and_then(|c| c.record.pose),
            recent_pose: drift::median_pose(&recent, last - window, last),
            calibration_orientation: self.cal_state.as_ref().map(|c| c.orientation),
            current_orientation: Some(self.orientation),
            calibration_age_sec: self.cal_state.as_ref().map(|c| last - c.at_sec),
            recalibrations_done: self.recalibrations.len() as u32,
            recalibrations_in_battery: in_battery_count as u32,
            in_battery,
        };
        drift::advise(&state, &self.config.params)
    }

    // --------------------------------------------------------------------------------------
    // Session accounting
    // --------------------------------------------------------------------------------------

    /// GAP-1 over a caller's processed frames with the session geometry (for the frames file).
    pub fn gap_fill(&self, samples: &[ProcessedSample]) -> Vec<ProcessedSample> {
        gapfill::fill_processed(samples, &self.geo, &self.config.params.gap_fill)
    }

    /// `validRatio` of the recorded frames (valid / all; 0 with none). Gap-filled frames are
    /// never valid (GAP-2).
    pub fn valid_ratio(&self) -> f64 {
        if self.recorded.is_empty() {
            return 0.0;
        }
        self.recorded.iter().filter(|r| r.valid).count() as f64 / self.recorded.len() as f64
    }

    /// Share of recorded frame time not `tracked` (GAP-3 intervals; 1 with no frames).
    pub fn data_loss(&self) -> f64 {
        self.accounting().1
    }

    fn filled_recording(&self) -> Vec<GapSample> {
        let view: Vec<GapSample> = self
            .recorded
            .iter()
            .map(|r| GapSample {
                t_sec: r.t_sec,
                tracked: r.state == TrackState::Tracked,
                point_norm: r.point,
                eye_pos_mm: r.eye,
                confidence: r.confidence,
                gap_filled: false,
            })
            .collect();
        gapfill::fill(&view, &self.geo, &self.config.params.gap_fill)
    }

    fn accounting(&self) -> (LossSummary, f64, Vec<GapSample>) {
        let filled = self.filled_recording();
        let frames: Vec<LossFrame> = self
            .recorded
            .iter()
            .zip(&filled)
            .map(|(r, g)| LossFrame {
                t_sec: r.t_sec,
                state: r.state,
                gap_filled: g.gap_filled,
            })
            .collect();
        let (loss, data_loss) = gapfill::loss(&frames, &self.config.params.gap_fill);
        (loss, data_loss, filled)
    }

    fn confidence_summary(&self, filled: &[GapSample]) -> ConfidenceSummary {
        let tracked: Vec<f64> = self
            .recorded
            .iter()
            .filter(|r| r.state == TrackState::Tracked)
            .map(|r| r.confidence)
            .collect();
        let min = self.config.quality_gate.min_confidence;
        let (mean_sample, low_ratio) = if tracked.is_empty() {
            (0.0, 1.0)
        } else {
            let n = tracked.len() as f64;
            (
                tracked.iter().sum::<f64>() / n,
                tracked.iter().filter(|&&c| c.is_nan() || c < min).count() as f64 / n,
            )
        };
        let fixations = gapfill::fixations(
            filled,
            &self.geo,
            self.config.params.scoring.ivt_velocity_deg_per_sec,
            MIN_FIXATION_DURATION_SEC,
        );
        ConfidenceSummary {
            mean_sample,
            mean_fixation: confidence::fixation(fixations.iter().filter_map(|f| f.mean_confidence)),
            low_confidence_sample_ratio: low_ratio,
        }
    }

    /// QS-1 `conditions` over the tracked recorded frames: the rules of
    /// `summary::conditions`, on the compact records.
    fn conditions(&self) -> TrackingConditions {
        let tracked = || {
            self.recorded
                .iter()
                .filter(|r| r.state == TrackState::Tracked)
        };
        let distances = || tracked().filter_map(|r| r.distance_cm);
        let iqr = match (quantile(distances(), 0.75), quantile(distances(), 0.25)) {
            (Some(hi), Some(lo)) => Some(hi - lo),
            _ => None,
        };
        TrackingConditions {
            distance_cm_median: median(distances()),
            distance_cm_iqr: iqr,
            head_speed_deg_per_sec_p90: quantile(tracked().filter_map(|r| r.head_speed), 0.9),
            ambient_intensity_median: median(tracked().filter_map(|r| r.ambient)),
            light_asymmetry: None,
            glasses: self.config.glasses,
        }
    }

    fn sample_rate_hz(&self) -> Option<f64> {
        let interval = median(self.recorded.windows(2).map(|w| w[1].t_sec - w[0].t_sec))?;
        (interval > 0.0).then(|| 1.0 / interval)
    }

    fn calibration_record(&self) -> CalibrationRecord {
        match &self.cal_state {
            Some(c) => CalibrationRecord {
                attempts: self.cal_attempts,
                age_sec_at_start: self.age_at_start_sec,
                ..c.record.clone()
            },
            None => CalibrationRecord {
                model: self.calibration.as_ref().map(CalModel::kind),
                layout_points: 0,
                usable_points: 0,
                missing_points: 0,
                residual_deg: None,
                loo_cv_deg: None,
                condition_number: None,
                quality: CalibrationQuality::Skipped,
                pose: None,
                age_sec_at_start: None,
                attempts: 0,
            },
        }
    }

    /// The session `trackingQuality` (QS-1) over the recorded frames, with QS-2 and QS-4 from
    /// `summary::tracking_quality`.
    pub fn summary(&self) -> TrackingQuality {
        let (loss, data_loss, filled) = self.accounting();
        let input = SummaryInput {
            device: TrackingDevice {
                model_identifier: self.config.model_identifier.clone(),
                geometry_source: self.config.geometry_source,
                orientation: self.orientation,
                screen_width_mm: self.geo.width_mm,
                screen_height_mm: self.geo.height_mm,
                sample_rate_hz: self.sample_rate_hz(),
            },
            calibration: self.calibration_record(),
            validation: self.validation(),
            drift_checks: self.drift.checks().to_vec(),
            recalibrations: self.recalibrations.clone(),
            conditions: self.conditions(),
            loss,
            confidence: self.confidence_summary(&filled),
            valid_ratio: self.valid_ratio(),
            data_loss,
        };
        summary::tracking_quality(input, &self.config.params)
    }
}

/// Copy of a tracked frame with only the fields `drift::median_pose` reads.
fn pose_only(p: &ProcessedSample) -> ProcessedSample {
    ProcessedSample {
        t_sec: p.t_sec,
        track_state: p.track_state,
        ray_status: None,
        raw_point_norm: None,
        calibrated_point_norm: None,
        filtered_point_norm: None,
        eye_pos_mm: None,
        distance_cm: p.distance_cm,
        head_yaw_rad: p.head_yaw_rad,
        head_pitch_rad: p.head_pitch_rad,
        head_roll_rad: None,
        head_speed_deg_per_sec: None,
        eye_speed_deg_per_sec: None,
        left_eye_openness: None,
        right_eye_openness: None,
        ambient_intensity: None,
        confidence: p.confidence,
        valid: p.valid,
        gap_filled: false,
        active_aoi_names: Vec::new(),
        stimulus_id: None,
        clip_media_time_sec: None,
    }
}

/// `value / dt` for a positive frame interval and a finite result.
fn per_sec(value: f64, dt: f64) -> Option<f64> {
    let v = value / dt;
    (dt > 0.0 && v.is_finite()).then_some(v)
}

/// Unit face-frame direction of `[yaw, pitch]` degrees (yaw = atan2(x, z),
/// pitch = atan2(y, √(x² + z²)), trap 4).
fn dir_of_angles(a: [f64; 2]) -> [f64; 3] {
    let (sy, cy) = a[0].to_radians().sin_cos();
    let (sp, cp) = a[1].to_radians().sin_cos();
    [cp * sy, sp, cp * cy]
}

/// Rotation angle (degrees) between the orientations of two column-major transforms:
/// the angle of `R_a^T R_b`, as `atan2(|skew| / 2, (trace - 1) / 2)` (well conditioned for
/// small and large angles).
fn rotation_angle_deg(a: &[f64; 16], b: &[f64; 16]) -> f64 {
    let col = |m: &[f64; 16], i: usize| [m[4 * i], m[4 * i + 1], m[4 * i + 2]];
    let dot = |u: [f64; 3], v: [f64; 3]| u[0] * v[0] + u[1] * v[1] + u[2] * v[2];
    // q[i][j] = col_i(a) · col_j(b) = (R_a^T R_b)_ij
    let mut q = [[0.0; 3]; 3];
    for (i, row) in q.iter_mut().enumerate() {
        for (j, v) in row.iter_mut().enumerate() {
            *v = dot(col(a, i), col(b, j));
        }
    }
    let trace = q[0][0] + q[1][1] + q[2][2];
    let s = [q[2][1] - q[1][2], q[0][2] - q[2][0], q[1][0] - q[0][1]];
    let sin2 = (s[0] * s[0] + s[1] * s[1] + s[2] * s[2]).sqrt();
    (sin2 / 2.0).atan2((trace - 1.0) / 2.0).to_degrees()
}

/// Converts processed frames to the frames-file `GazeTelemetrySample` (CNF-4), holding the
/// last known values for frames without them.
///
/// - `confidence` is the computed CNF-1 value (replacing the constant 0.95, G-F6);
///   `valid` is `ProcessedSample::valid`.
/// - The point is the filtered point for `tracked` and gap-filled frames, the calibrated
///   (unclamped) point for `offscreen` frames, and the last tracked point otherwise (the
///   Swift actor froze the point during blinks).
/// - Pose, distance and eye openness are the frame's own values, or the last known ones for a
///   frame without a face (trap 6: never zeros). Before the first face frame the Swift
///   "searching" values are used (point (0.5, 0.5), distance 48 cm, pose 0) except that the
///   eyes read as open (1.0, not the Swift 0.0), so a frame with no face is never counted as
///   a blink by the CORE-7 gate; such frames are invalid with confidence 0. The frame fields
///   are not optional on the wire, so pose and distance cannot be absent here; consumers must
///   rely on `isValid` / `confidence` for these frames.
/// - Times are relative to `origin_sec` (the session clock origin).
#[derive(Debug, Clone, PartialEq)]
pub struct TelemetryWriter {
    origin_sec: f64,
    gate: QualityGate,
    point: [f64; 2],
    yaw: f64,
    pitch: f64,
    roll: f64,
    distance_cm: f64,
    left_open: f64,
    right_open: f64,
}

impl TelemetryWriter {
    pub fn new(origin_sec: f64, gate: QualityGate) -> Self {
        Self {
            origin_sec,
            gate,
            point: [0.5, 0.5],
            yaw: 0.0,
            pitch: 0.0,
            roll: 0.0,
            distance_cm: 48.0,
            left_open: 1.0,
            right_open: 1.0,
        }
    }

    pub fn sample(&mut self, p: &ProcessedSample) -> GazeTelemetrySample {
        let keep = |slot: &mut f64, v: Option<f64>| {
            if let Some(v) = v {
                *slot = v;
            }
            *slot
        };
        let point = if p.track_state == TrackState::Tracked || p.gap_filled {
            match p.filtered_point_norm {
                Some(f) => {
                    if p.track_state == TrackState::Tracked {
                        self.point = f;
                    }
                    f
                }
                None => self.point,
            }
        } else if p.track_state == TrackState::Offscreen {
            p.calibrated_point_norm.unwrap_or(self.point)
        } else {
            self.point
        };
        let elapsed = p.t_sec - self.origin_sec;
        GazeTelemetrySample {
            timestamp_ms: elapsed * 1000.0,
            video_time_sec: elapsed,
            x_norm: point[0],
            y_norm: point[1],
            confidence: p.confidence,
            left_eye_openness: keep(&mut self.left_open, p.left_eye_openness),
            right_eye_openness: keep(&mut self.right_open, p.right_eye_openness),
            head_pitch_rad: keep(&mut self.pitch, p.head_pitch_rad),
            head_yaw_rad: keep(&mut self.yaw, p.head_yaw_rad),
            head_roll_rad: keep(&mut self.roll, p.head_roll_rad),
            distance_to_screen_cm: keep(&mut self.distance_cm, p.distance_cm),
            is_valid: p.valid,
            stimulus_id: p.stimulus_id.clone(),
            clip_media_time_sec: p.clip_media_time_sec,
        }
    }

    /// The CORE-7 gate on a written sample (its `qualityVerdict` in the frame).
    pub fn evaluate(&self, s: &GazeTelemetrySample) -> Evaluation {
        quality::evaluate_sample(s, &self.gate)
    }
}
