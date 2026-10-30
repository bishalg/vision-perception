//! owner: WP-R.1
//!
//! Per-sample acquisition quality gate: port of `GazeQualityGate.evaluateSample`
//! (`GazeQualityGate.swift`), including the reason strings shown in the UI. CORE-7.

use serde::{Deserialize, Serialize};

use crate::types::{GazeQualityVerdict, GazeTelemetrySample};

/// Gate parameters. Serialised with the `ScoringProfile.qualityGate` key names of CON-5
/// so the contract crate can embed this struct; defaults are the Swift `init` defaults.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QualityGate {
    pub min_distance_cm: f64,
    pub max_distance_cm: f64,
    /// About 35 degrees by default.
    pub max_yaw_rad: f64,
    /// About 30 degrees by default.
    pub max_pitch_rad: f64,
    pub min_confidence: f64,
    pub blink_threshold: f64,
}

impl Default for QualityGate {
    fn default() -> Self {
        Self {
            min_distance_cm: 35.0,
            max_distance_cm: 65.0,
            max_yaw_rad: 0.61,
            max_pitch_rad: 0.52,
            min_confidence: 0.50,
            blink_threshold: 0.45,
        }
    }
}

/// `GazeQualityGate.Evaluation`. Swift does not encode it; the camelCase keys here are the
/// JSON shape across the FFI boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Evaluation {
    pub is_valid: bool,
    pub verdict: GazeQualityVerdict,
    pub failure_reasons: Vec<String>,
    pub is_blink: bool,
}

/// Swift `Int(Double)`: truncation toward zero. Swift traps on non-finite values; Rust
/// saturates instead (NaN becomes 0) so a malformed gate never panics.
fn swift_int(value: f64) -> i64 {
    value as i64
}

/// Evaluates one sample against the pediatric acquisition criteria.
///
/// Any failure makes the sample invalid; the verdict is `rejected` for a blink or more than
/// two reasons, else `degraded`. A passing sample is `optimal` (never `acceptable`).
pub fn evaluate_sample(sample: &GazeTelemetrySample, gate: &QualityGate) -> Evaluation {
    let mut reasons = Vec::new();

    if sample.distance_to_screen_cm < gate.min_distance_cm {
        reasons.push(format!(
            "Subject too close (< {} cm)",
            swift_int(gate.min_distance_cm)
        ));
    } else if sample.distance_to_screen_cm > gate.max_distance_cm {
        reasons.push(format!(
            "Subject too far (> {} cm)",
            swift_int(gate.max_distance_cm)
        ));
    }

    if sample.head_yaw_rad.abs() > gate.max_yaw_rad {
        reasons.push(format!(
            "Excessive head yaw (> {}°)",
            swift_int(gate.max_yaw_rad * 180.0 / std::f64::consts::PI)
        ));
    }
    if sample.head_pitch_rad.abs() > gate.max_pitch_rad {
        reasons.push(format!(
            "Excessive head pitch (> {}°)",
            swift_int(gate.max_pitch_rad * 180.0 / std::f64::consts::PI)
        ));
    }

    let is_blink = sample.left_eye_openness < gate.blink_threshold
        || sample.right_eye_openness < gate.blink_threshold;
    if is_blink {
        reasons.push("Blink or ocular occlusion".to_owned());
    }

    if sample.confidence < gate.min_confidence {
        reasons.push(format!(
            "Low tracking confidence (< {}%)",
            swift_int(gate.min_confidence * 100.0)
        ));
    }

    if reasons.is_empty() {
        return Evaluation {
            is_valid: true,
            verdict: GazeQualityVerdict::Optimal,
            failure_reasons: reasons,
            is_blink: false,
        };
    }

    let verdict = if is_blink || reasons.len() > 2 {
        GazeQualityVerdict::Rejected
    } else {
        GazeQualityVerdict::Degraded
    };
    Evaluation {
        is_valid: false,
        verdict,
        failure_reasons: reasons,
        is_blink,
    }
}
