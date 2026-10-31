//! owner: WP-G.3
//!
//! Per-sample confidence (CNF-1) and per-fixation confidence (CNF-3).
//!
//! The sample confidence is the **minimum** of six factors. Each factor is a piecewise-linear
//! ramp: 1 inside its comfort zone, falling linearly to exactly 0.5 at the hard limit, and 0
//! beyond the limit (strictly beyond, with the same comparison the CORE-7 gate uses). So the
//! existing gate (`minConfidence = 0.5`) keeps exactly its distance, pose and blink
//! boundaries, and the computed confidence newly rejects only fast head motion, a large
//! distance shift from the calibration pose, and (through the track state) artefacts.
//!
//! | factor       | comfort                         | hard limit (0.5)                            |
//! | ------------ | ------------------------------- | ------------------------------------------- |
//! | eye openness | ≥ `eyeOpennessComfort` (0.70)   | `qualityGate.blinkThreshold`                |
//! | \|yaw\|      | ≤ `poseComfortFraction` × limit | `qualityGate.maxYawRad`                     |
//! | \|pitch\|    | ≤ `poseComfortFraction` × limit | `qualityGate.maxPitchRad`                   |
//! | distance     | 40–60 cm                        | `qualityGate.minDistanceCm`/`maxDistanceCm` |
//! | head speed   | ≤ 30°/s                         | `confidence.maxHeadSpeedDegPerSec` (120)    |
//! | pose shift   | ≤ 10 %                          | `recalibration.poseShiftDistancePct` (20)   |
//!
//! The gate limits are read from [`QualityGate`] (`ScoringProfile.qualityGate`), never copied
//! into `TrackingQualityParams`, so the 0.5 crossing cannot drift from the gate. Eye openness
//! is the smaller of the two eyes (the gate rejects when either eye is closed). An unmeasured
//! feature (`None`, e.g. head speed on the first frame, pose shift without a calibration pose
//! or with `angularAffine`) does not lower the confidence; a non-finite one gives 0. Any track
//! state other than `tracked` gives 0.

use crate::quality::QualityGate;
use crate::types::tracking::{
    ConfidenceParams, ProcessedSample, TrackState, TrackingQualityParams,
};

/// Hard limits and comfort zones of every factor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConfidenceLimits {
    /// Distance, pose and blink limits (CORE-7).
    pub gate: QualityGate,
    /// Comfort zones and the head-speed limit.
    pub comfort: ConfidenceParams,
    /// `recalibration.poseShiftDistancePct`.
    pub pose_shift_limit_pct: f64,
}

impl ConfidenceLimits {
    pub fn new(gate: &QualityGate, params: &TrackingQualityParams) -> Self {
        Self {
            gate: *gate,
            comfort: params.confidence,
            pose_shift_limit_pct: params.recalibration.pose_shift_distance_pct,
        }
    }
}

/// The measured features of one frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConfidenceFeatures {
    pub track_state: TrackState,
    pub left_eye_openness: Option<f64>,
    pub right_eye_openness: Option<f64>,
    pub head_yaw_rad: Option<f64>,
    pub head_pitch_rad: Option<f64>,
    pub distance_cm: Option<f64>,
    pub head_speed_deg_per_sec: Option<f64>,
    /// `|distance - calibration distance| / calibration distance × 100`; `None` when there is
    /// no calibration pose or the model is not a screen model.
    pub pose_shift_pct: Option<f64>,
}

impl ConfidenceFeatures {
    /// Features of a processed frame (its own track state).
    pub fn from_processed(p: &ProcessedSample, pose_shift_pct: Option<f64>) -> Self {
        Self {
            track_state: p.track_state,
            left_eye_openness: p.left_eye_openness,
            right_eye_openness: p.right_eye_openness,
            head_yaw_rad: p.head_yaw_rad,
            head_pitch_rad: p.head_pitch_rad,
            distance_cm: p.distance_cm,
            head_speed_deg_per_sec: p.head_speed_deg_per_sec,
            pose_shift_pct,
        }
    }
}

/// Every factor of one frame, each in [0, 1].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConfidenceFactors {
    pub eye_openness: f64,
    pub yaw: f64,
    pub pitch: f64,
    pub distance: f64,
    pub head_speed: f64,
    pub pose_shift: f64,
}

impl ConfidenceFactors {
    /// The minimum of the factors.
    pub fn min(&self) -> f64 {
        [
            self.eye_openness,
            self.yaw,
            self.pitch,
            self.distance,
            self.head_speed,
            self.pose_shift,
        ]
        .into_iter()
        .fold(1.0, f64::min)
    }
}

/// Factor for a value that is good when small: 1 at or below `comfort`, 0.5 at `limit`,
/// linear between, 0 strictly above `limit` or for NaN. A misconfigured zone
/// (`limit <= comfort`) degrades to a step: 1 below the limit, 0.5 at it, 0 above.
pub fn ramp_down(value: f64, comfort: f64, limit: f64) -> f64 {
    if value.is_nan() || value > limit {
        return 0.0;
    }
    if value <= comfort {
        return if value == limit { 0.5 } else { 1.0 };
    }
    1.0 - 0.5 * (value - comfort) / (limit - comfort)
}

/// Factor for a value that is good when large (mirror of [`ramp_down`]): 1 at or above
/// `comfort`, 0.5 at `limit`, 0 strictly below `limit` or for NaN.
pub fn ramp_up(value: f64, comfort: f64, limit: f64) -> f64 {
    if value.is_nan() || value < limit {
        return 0.0;
    }
    if value >= comfort {
        return if value == limit { 0.5 } else { 1.0 };
    }
    1.0 - 0.5 * (comfort - value) / (comfort - limit)
}

/// Two-sided distance factor: comfort `[cmin, cmax]`, limits `[lmin, lmax]`.
fn ramp_band(value: f64, cmin: f64, cmax: f64, lmin: f64, lmax: f64) -> f64 {
    if value.is_nan() {
        0.0
    } else if value < cmin {
        ramp_up(value, cmin, lmin)
    } else {
        ramp_down(value, cmax, lmax)
    }
}

fn measured(v: Option<f64>, f: impl Fn(f64) -> f64) -> f64 {
    v.map_or(1.0, f)
}

/// Every factor of a frame (the track state is ignored here; see [`sample`]).
pub fn factors(f: &ConfidenceFeatures, l: &ConfidenceLimits) -> ConfidenceFactors {
    let g = &l.gate;
    let c = &l.comfort;
    let openness = match (f.left_eye_openness, f.right_eye_openness) {
        (Some(a), Some(b)) => Some(if a.is_nan() || b.is_nan() {
            f64::NAN
        } else {
            a.min(b)
        }),
        (a, b) => a.or(b),
    };
    let pose = |v: Option<f64>, limit: f64| {
        measured(v, |v| {
            ramp_down(v.abs(), c.pose_comfort_fraction * limit, limit)
        })
    };
    ConfidenceFactors {
        eye_openness: measured(openness, |v| {
            ramp_up(v, c.eye_openness_comfort, g.blink_threshold)
        }),
        yaw: pose(f.head_yaw_rad, g.max_yaw_rad),
        pitch: pose(f.head_pitch_rad, g.max_pitch_rad),
        distance: measured(f.distance_cm, |v| {
            ramp_band(
                v,
                c.distance_comfort_min_cm,
                c.distance_comfort_max_cm,
                g.min_distance_cm,
                g.max_distance_cm,
            )
        }),
        head_speed: measured(f.head_speed_deg_per_sec, |v| {
            ramp_down(
                v.abs(),
                c.head_speed_comfort_deg_per_sec,
                c.max_head_speed_deg_per_sec,
            )
        }),
        pose_shift: measured(f.pose_shift_pct, |v| {
            ramp_down(v.abs(), c.pose_shift_comfort_pct, l.pose_shift_limit_pct)
        }),
    }
}

/// CNF-1 sample confidence in [0, 1]: 0 unless `tracked`, else the minimum factor.
pub fn sample(f: &ConfidenceFeatures, l: &ConfidenceLimits) -> f64 {
    if f.track_state != TrackState::Tracked {
        return 0.0;
    }
    factors(f, l).min()
}

/// CNF-3 fixation confidence: the mean of its samples' confidences; `None` for no samples.
pub fn fixation(confidences: impl IntoIterator<Item = f64>) -> Option<f64> {
    let (sum, n) = confidences
        .into_iter()
        .fold((0.0, 0usize), |(s, n), c| (s + c, n + 1));
    (n > 0).then(|| sum / n as f64)
}

/// CNF-3: a fixation below `qualityGate.minConfidence` is `lowConfidence`.
pub fn is_low_confidence(mean_confidence: f64, gate: &QualityGate) -> bool {
    mean_confidence.is_nan() || mean_confidence < gate.min_confidence
}
