//! owner: WP-G.3
//!
//! Per-frame track state (TRK-1, TRK-2).
//!
//! Precedence, first match wins:
//! 1. `lost`: no face anchor, or ARKit reports it untracked;
//! 2. `blink`: either eye's openness below `qualityGate.blinkThreshold` (strict, the CORE-7
//!    gate's rule), and every frame within `trackState.blinkRecoveryMs` of frame time after the
//!    last blink frame (time based, fixes G-F8; the Swift actor counted 3 frames, which is
//!    100 ms at 30 Hz);
//! 3. `artifact`: eye-in-head angular speed above `maxEyeSpeedDegPerSec` (strict), or a ray
//!    that is not `ok` (GEO-5), or a face frame without any ray result;
//! 4. `offscreen`: the **calibrated** point (TRK-2) outside the screen by more than
//!    `offscreenMarginNorm` on either axis (strict; a non-finite point is off screen);
//! 5. otherwise `tracked`.
//!
//! The blink timer is the only state. Lost frames neither extend nor cancel a running
//! recovery: it runs in frame time. The machine is split into the stateful ocular part
//! ([`TrackStateMachine::ocular_state`]) and the pure geometric part ([`geometric_state`]) so
//! the pipeline can judge a calibration sample on the raw ray without touching the timer.

use crate::types::tracking::{RayStatus, TrackState, TrackStateParams};

/// Tolerance on time-window edges, ms (frame clocks accumulate rounding error).
const EPS_MS: f64 = 1e-6;

/// Everything the state decision looks at for one frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrackInputs {
    pub t_sec: f64,
    /// A tracked face anchor with pose data.
    pub face: bool,
    /// `1 - eyeBlink*`, clamped to [0, 1].
    pub left_eye_openness: f64,
    pub right_eye_openness: f64,
    /// Status of the (calibrated) ray; `None` when there was no ray to cast.
    pub ray_status: Option<RayStatus>,
    /// Calibrated screen-normalised point (unclamped).
    pub point_norm: Option<[f64; 2]>,
    /// Eye-in-head angular speed; `None` when not measurable (first frame after a gap).
    pub eye_speed_deg_per_sec: Option<f64>,
}

/// TRK-2 off-screen rule: outside `[-margin, 1 + margin]` on either axis (strict).
pub fn is_offscreen(p: [f64; 2], margin: f64) -> bool {
    p.iter()
        .any(|&v| v.is_nan() || v < -margin || v > 1.0 + margin)
}

/// Rules 3-5 of the module docs. `check_offscreen = false` skips rule 4 (calibration samples
/// are judged on the raw ray, which is legitimately off the glass before calibration).
pub fn geometric_state(
    ray_status: Option<RayStatus>,
    point_norm: Option<[f64; 2]>,
    eye_speed_deg_per_sec: Option<f64>,
    params: &TrackStateParams,
    check_offscreen: bool,
) -> TrackState {
    if eye_speed_deg_per_sec.is_some_and(|v| v > params.max_eye_speed_deg_per_sec) {
        return TrackState::Artifact;
    }
    let point = match (ray_status, point_norm) {
        (Some(RayStatus::Ok), Some(p)) => p,
        _ => return TrackState::Artifact,
    };
    if check_offscreen && is_offscreen(point, params.offscreen_margin_norm) {
        return TrackState::Offscreen;
    }
    TrackState::Tracked
}

/// The blink timer plus the precedence rules.
#[derive(Debug, Clone, PartialEq)]
pub struct TrackStateMachine {
    params: TrackStateParams,
    blink_threshold: f64,
    last_blink_sec: Option<f64>,
}

impl TrackStateMachine {
    /// `blink_threshold` is `ScoringProfile.qualityGate.blinkThreshold` (not duplicated in
    /// `TrackingQualityParams`).
    pub fn new(params: TrackStateParams, blink_threshold: f64) -> Self {
        Self {
            params,
            blink_threshold,
            last_blink_sec: None,
        }
    }

    pub fn params(&self) -> &TrackStateParams {
        &self.params
    }

    /// Rules 1-2. `Some(lost | blink)` or `None` (the eyes are open and out of recovery).
    /// Updates the blink timer.
    pub fn ocular_state(
        &mut self,
        t_sec: f64,
        face: bool,
        left_eye_openness: f64,
        right_eye_openness: f64,
    ) -> Option<TrackState> {
        if !face {
            return Some(TrackState::Lost);
        }
        let closed =
            left_eye_openness < self.blink_threshold || right_eye_openness < self.blink_threshold;
        if closed {
            self.last_blink_sec = Some(t_sec);
            return Some(TrackState::Blink);
        }
        if let Some(last) = self.last_blink_sec {
            let since_ms = (t_sec - last) * 1000.0;
            if since_ms <= self.params.blink_recovery_ms + EPS_MS {
                return Some(TrackState::Blink);
            }
            self.last_blink_sec = None;
        }
        None
    }

    /// The frame's state (all five rules).
    pub fn classify(&mut self, inp: &TrackInputs) -> TrackState {
        self.ocular_state(
            inp.t_sec,
            inp.face,
            inp.left_eye_openness,
            inp.right_eye_openness,
        )
        .unwrap_or_else(|| {
            geometric_state(
                inp.ray_status,
                inp.point_norm,
                inp.eye_speed_deg_per_sec,
                &self.params,
                true,
            )
        })
    }

    /// Clears the blink timer.
    pub fn reset(&mut self) {
        self.last_blink_sec = None;
    }
}
