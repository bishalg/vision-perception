//! owner: WP-G.4
//!
//! Drift monitor (DRF-1..DRF-3) and re-calibration advice (CAL-9, VAL-4).
//!
//! Decisions recorded here:
//! - The drift **reference** is the mean offset vector over the hit validation targets of the
//!   current calibration (DRF-2). Without one (no validation, or none hit) a check still
//!   records its `offsetDeg` but `driftDeg` is absent: there is nothing to compare with, and
//!   an unmeasured value is never defaulted (CON-14).
//! - `driftDeg` is the Euclidean distance between two offset vectors that are already in
//!   degrees (x, y); the offsets themselves are exact visual angles (`validation`).
//! - DRF-3 "two consecutive checks": a missed check (no `driftDeg`) breaks the run, like a
//!   check at or below `warnDeg`. Thresholds are strict (`>`).
//! - Once fired, the drift trigger stays latched until [`DriftMonitor::on_recalibrated`];
//!   the check history is session-wide and survives re-calibration.
//! - A check's `atSec` is the `started_at_sec` of its target window, in whatever session
//!   clock the caller uses for the window.

use crate::gaze::validation::median;
use crate::types::tracking::{
    CalModelKind, CalibrationPose, CalibrationQuality, DriftCheckRecord, DriftParams, Orientation,
    ProcessedSample, RecalAction, RecalAdvice, RecalReason, TargetOutcome, TargetResult,
    TrackState, TrackingQualityParams,
};

/// Session drift state: reference offset, check history and the DRF-3 trigger.
#[derive(Debug, Clone, PartialEq)]
pub struct DriftMonitor {
    params: DriftParams,
    reference: Option<[f64; 2]>,
    checks: Vec<DriftCheckRecord>,
    warn_run: u32,
    triggered: bool,
}

impl DriftMonitor {
    pub fn new(params: DriftParams) -> Self {
        Self {
            params,
            reference: None,
            checks: Vec::new(),
            warn_run: 0,
            triggered: false,
        }
    }

    /// Sets the reference from the validation that followed the current calibration (mean
    /// offset over hit targets; none hit → no reference). Restarts the DRF-3 run.
    pub fn set_reference(&mut self, validation_targets: &[TargetResult]) {
        let offsets: Vec<[f64; 2]> = validation_targets
            .iter()
            .filter(|t| t.outcome == TargetOutcome::Hit)
            .filter_map(|t| t.offset_deg)
            .collect();
        self.reference = if offsets.is_empty() {
            None
        } else {
            let n = offsets.len() as f64;
            Some([
                offsets.iter().map(|o| o[0]).sum::<f64>() / n,
                offsets.iter().map(|o| o[1]).sum::<f64>() / n,
            ])
        };
        self.warn_run = 0;
        self.triggered = false;
    }

    /// A new calibration invalidates the reference and the trigger; history is kept.
    pub fn on_recalibrated(&mut self) {
        self.reference = None;
        self.warn_run = 0;
        self.triggered = false;
    }

    /// The current reference offset vector (degrees), if any.
    pub fn reference_offset_deg(&self) -> Option<[f64; 2]> {
        self.reference
    }

    /// Position of the next check target: `checkLayout` cycled by the number of checks so
    /// far (DRF-1). An empty layout falls back to the screen centre.
    pub fn next_check_target(&self) -> [f64; 2] {
        let layout = &self.params.check_layout;
        if layout.is_empty() {
            return [0.5, 0.5];
        }
        layout[self.checks.len() % layout.len()]
    }

    /// Records one check target result (DRF-2) and updates the DRF-3 trigger.
    pub fn record_check(&mut self, result: &TargetResult) -> DriftCheckRecord {
        let hit = result.outcome == TargetOutcome::Hit;
        let offset_vec = if hit { result.offset_deg } else { None };
        let drift_deg = match (offset_vec, self.reference) {
            (Some(o), Some(r)) => Some((o[0] - r[0]).hypot(o[1] - r[1])),
            _ => None,
        };
        let record = DriftCheckRecord {
            at_sec: result.started_at_sec,
            x_norm: result.x_norm,
            y_norm: result.y_norm,
            offset_deg: if hit { result.accuracy_deg } else { None },
            drift_deg,
            missed: !hit,
        };
        match drift_deg {
            Some(d) if d > self.params.warn_deg => {
                self.warn_run += 1;
                if d > self.params.hard_deg || self.warn_run >= self.params.warn_consecutive {
                    self.triggered = true;
                }
            }
            _ => self.warn_run = 0,
        }
        self.checks.push(record.clone());
        record
    }

    /// Every check of the session, in order.
    pub fn checks(&self) -> &[DriftCheckRecord] {
        &self.checks
    }

    /// Largest `driftDeg` of the session; absent when no check measured drift.
    pub fn max_drift_deg(&self) -> Option<f64> {
        max_drift_deg(&self.checks)
    }

    /// DRF-3 fired since the last (re-)calibration.
    pub fn triggered(&self) -> bool {
        self.triggered
    }
}

/// Largest `driftDeg` over `checks`; absent when none is measured.
pub fn max_drift_deg(checks: &[DriftCheckRecord]) -> Option<f64> {
    checks
        .iter()
        .filter_map(|c| c.drift_deg)
        .filter(|d| d.is_finite())
        .reduce(f64::max)
}

/// Median head pose over the `tracked` frames with `from_sec <= t <= to_sec` (CAL-9.4 uses the
/// last `poseWindowSec`; the same function gives the calibration pose over the calibration).
/// `None` when no frame in the window has a complete pose.
pub fn median_pose(
    samples: &[ProcessedSample],
    from_sec: f64,
    to_sec: f64,
) -> Option<CalibrationPose> {
    let poses: Vec<[f64; 3]> = samples
        .iter()
        .filter(|s| {
            s.track_state == TrackState::Tracked && s.t_sec >= from_sec && s.t_sec <= to_sec
        })
        .filter_map(|s| {
            Some([
                s.distance_cm?,
                s.head_yaw_rad?.to_degrees(),
                s.head_pitch_rad?.to_degrees(),
            ])
        })
        .collect();
    Some(CalibrationPose {
        distance_cm: median(poses.iter().map(|p| p[0]))?,
        yaw_deg: median(poses.iter().map(|p| p[1]))?,
        pitch_deg: median(poses.iter().map(|p| p[2]))?,
    })
}

/// Everything CAL-9 looks at, evaluated at a clip boundary. `None` means "not measured" and
/// never fires a trigger.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AdviceState {
    pub calibration_quality: Option<CalibrationQuality>,
    pub calibration_model: Option<CalModelKind>,
    /// Accuracy of the validation after the current calibration.
    pub validation_accuracy_deg: Option<f64>,
    /// `complete` of that validation (VAL-2); `None` while no validation target was shown.
    pub validation_complete: Option<bool>,
    /// [`DriftMonitor::triggered`].
    pub drift_triggered: bool,
    pub calibration_pose: Option<CalibrationPose>,
    /// [`median_pose`] over the last `recalibration.poseWindowSec`.
    pub recent_pose: Option<CalibrationPose>,
    pub calibration_orientation: Option<Orientation>,
    pub current_orientation: Option<Orientation>,
    pub calibration_age_sec: Option<f64>,
    /// Re-calibrations already done this session (the first calibration is not one).
    pub recalibrations_done: u32,
    /// Of those, the ones done after the battery started.
    pub recalibrations_in_battery: u32,
    /// The battery (first clip) has started.
    pub in_battery: bool,
}

fn pose_shifted(
    cal: &CalibrationPose,
    now: &CalibrationPose,
    params: &TrackingQualityParams,
) -> bool {
    let r = &params.recalibration;
    let distance_pct = (now.distance_cm - cal.distance_cm).abs() * 100.0 / cal.distance_cm;
    distance_pct > r.pose_shift_distance_pct
        || (now.yaw_deg - cal.yaw_deg).abs() > r.pose_shift_angle_deg
        || (now.pitch_deg - cal.pitch_deg).abs() > r.pose_shift_angle_deg
}

/// CAL-9 advice. Reasons are listed in trigger order (1-6). Trigger 2 (`validationAccuracy`)
/// also fires when a validation was shown but no target was hit (no accuracy, no drift
/// reference), and warns when the validation is `incomplete` with an accuracy inside the
/// bands. A re-calibration trigger whose
/// ceiling is spent (`maxRecalibrationsPerSession`, or `maxRecalibrationsInBattery` inside the
/// battery) becomes a warning with `limitReached`. The VAL-4 warn band alone gives `warn`
/// (when a re-calibration is advised anyway, the warn-band reason is not listed).
/// The caller acts on the advice only at a clip boundary.
pub fn advise(state: &AdviceState, params: &TrackingQualityParams) -> RecalAdvice {
    let mut recal = Vec::new();
    let mut warn = Vec::new();

    if matches!(
        state.calibration_quality,
        Some(CalibrationQuality::Poor | CalibrationQuality::Failed)
    ) {
        recal.push(RecalReason::CalibrationQuality);
    }
    match crate::gaze::validation::accuracy_action(
        state.validation_accuracy_deg,
        &params.validation,
    ) {
        RecalAction::Recalibrate => recal.push(RecalReason::ValidationAccuracy),
        RecalAction::Warn => warn.push(RecalReason::ValidationAccuracy),
        RecalAction::None => match (state.validation_complete, state.validation_accuracy_deg) {
            // A validation was shown but no target was hit: the calibration is unverified
            // and there is no drift reference (DRF-2), so drift cannot be monitored.
            (Some(_), None) => recal.push(RecalReason::ValidationAccuracy),
            // VAL-2 `incomplete` (fewer than `minTargetsHit`): the accuracy rests on too few
            // targets; the operator is warned but may continue.
            (Some(false), Some(_)) => warn.push(RecalReason::ValidationAccuracy),
            _ => {}
        },
    }
    if state.drift_triggered {
        recal.push(RecalReason::Drift);
    }
    let screen_model = matches!(
        state.calibration_model,
        Some(
            CalModelKind::ScreenAffine | CalModelKind::ScreenHomography | CalModelKind::ScreenPoly2
        )
    );
    if let (true, Some(cal), Some(now)) =
        (screen_model, &state.calibration_pose, &state.recent_pose)
    {
        if pose_shifted(cal, now, params) {
            recal.push(RecalReason::PoseShift);
        }
    }
    if let (Some(a), Some(b)) = (state.calibration_orientation, state.current_orientation) {
        if a != b {
            recal.push(RecalReason::OrientationChanged);
        }
    }
    if let Some(age) = state.calibration_age_sec {
        if age > params.recalibration.max_calibration_age_sec {
            recal.push(RecalReason::CalibrationAge);
        }
    }

    if !recal.is_empty() {
        let r = &params.recalibration;
        let limit_reached = state.recalibrations_done >= r.max_recalibrations_per_session
            || (state.in_battery
                && state.recalibrations_in_battery >= r.max_recalibrations_in_battery);
        return RecalAdvice {
            action: if limit_reached {
                RecalAction::Warn
            } else {
                RecalAction::Recalibrate
            },
            reasons: recal,
            limit_reached,
        };
    }
    if !warn.is_empty() {
        return RecalAdvice {
            action: RecalAction::Warn,
            reasons: warn,
            limit_reached: false,
        };
    }
    RecalAdvice::default()
}
