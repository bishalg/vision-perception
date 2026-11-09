//! owner: WP-C.5 (moved from asd-backend `config_docs::lifecycle`, WP-B.4)
//!
//! Range checks on a [`ScoringProfile`]. The type itself only checks the shape (it
//! decodes); these are the value rules a profile must also pass before it can be created,
//! approved or published. One implementation for every caller: the backend
//! (`config_docs::lifecycle::validate_body`) and the console editor through asd-wasm
//! (`validateScoringProfile`).
//!
//! Each problem names the profile member it is about (`path`, camelCase dotted as on the
//! wire, e.g. `flag.greenFloor`) so an editor can show it next to the field; the backend
//! reports only the `message`.
//!
//! `trackingQuality` (SCH-3) is checked by asd-core `TrackingQualityParams::validate`; its
//! problems appear here under `trackingQuality.` (WP-A.13). The device runs the same checks
//! before it accepts a delivered bundle (`asd_sync::accept_bundle`).
//!
//! WP-X.0 (PRM-3): the parameter registry (`crate::params`) holds the per-member kind and
//! range of every knob; [`profile_problems`] runs it first and then these rules.
//! [`scoring_profile_problems`] stays the cross-field rule set `params::validate` appends
//! (and the answer of the callers not yet moved to the registry).

use serde::Serialize;

use crate::config::ScoringProfile;

/// One failed check: the camelCase dotted member path and the human-readable reason.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProfileProblem {
    pub path: String,
    pub message: String,
}

impl ProfileProblem {
    fn new(path: &str, message: impl Into<String>) -> Self {
        Self {
            path: path.to_owned(),
            message: message.into(),
        }
    }
}

/// Every range problem of `p`, in a fixed order (empty when valid).
pub fn scoring_profile_problems(p: &ScoringProfile) -> Vec<ProfileProblem> {
    let mut problems = Vec::new();
    let mut unit = |path: &str, v: f64| {
        if !(0.0..=1.0).contains(&v) {
            problems.push(ProfileProblem::new(
                path,
                format!("{path} must be in [0, 1]"),
            ));
        }
    };
    unit("flag.greenFloor", p.flag.green_floor);
    unit("flag.yellowFloor", p.flag.yellow_floor);
    unit("flag.lowConfidenceShare", p.flag.low_confidence_share);
    unit("flag.atypicalShareForRed", p.flag.atypical_share_for_red);
    unit("lowConfidenceValidRatio", p.low_confidence_valid_ratio);
    unit("qualityGate.minConfidence", p.quality_gate.min_confidence);
    unit("qualityGate.blinkThreshold", p.quality_gate.blink_threshold);
    if p.flag.yellow_floor > p.flag.green_floor {
        problems.push(ProfileProblem::new(
            "flag.yellowFloor",
            "flag.yellowFloor must not exceed flag.greenFloor",
        ));
    }
    let q = &p.quality_gate;
    if !(q.min_distance_cm > 0.0 && q.min_distance_cm < q.max_distance_cm) {
        let path = if q.min_distance_cm > 0.0 {
            "qualityGate.maxDistanceCm"
        } else {
            "qualityGate.minDistanceCm"
        };
        problems.push(ProfileProblem::new(
            path,
            "qualityGate needs 0 < minDistanceCm < maxDistanceCm",
        ));
    }
    if !(q.max_yaw_rad > 0.0 && q.max_pitch_rad > 0.0) {
        let path = if q.max_yaw_rad > 0.0 {
            "qualityGate.maxPitchRad"
        } else {
            "qualityGate.maxYawRad"
        };
        problems.push(ProfileProblem::new(
            path,
            "qualityGate.maxYawRad and maxPitchRad must be positive",
        ));
    }
    if ![
        q.min_distance_cm,
        q.max_distance_cm,
        q.max_yaw_rad,
        q.max_pitch_rad,
    ]
    .iter()
    .all(|v| v.is_finite())
    {
        problems.push(ProfileProblem::new(
            "qualityGate",
            "qualityGate distances and angles must be finite numbers",
        ));
    }
    if p.schema_version.trim().is_empty() {
        problems.push(ProfileProblem::new(
            "schemaVersion",
            "schemaVersion is required",
        ));
    }
    // DSP-1 (1.3.0): band and verdict bounds in order. Only finite values are compared; the
    // registry reports the rest.
    let b = &p.display.bands;
    let q = &p.session_quality;
    for (path, lower, upper, message) in [
        (
            "display.bands.socialPreferencePct.monitorMin",
            b.social_preference_pct.monitor_min,
            b.social_preference_pct.ok_min,
            "display.bands.socialPreferencePct.monitorMin must not exceed okMin",
        ),
        (
            "display.bands.eyeDwellPct.monitorMin",
            b.eye_dwell_pct.monitor_min,
            b.eye_dwell_pct.ok_min,
            "display.bands.eyeDwellPct.monitorMin must not exceed okMin",
        ),
        (
            "display.bands.mouthDwellPct.okMin",
            b.mouth_dwell_pct.ok_min,
            b.mouth_dwell_pct.ok_max,
            "display.bands.mouthDwellPct.okMin must not exceed okMax",
        ),
        (
            "display.bands.firstEyeLatencyMs.okMax",
            b.first_eye_latency_ms.ok_max,
            b.first_eye_latency_ms.monitor_max,
            "display.bands.firstEyeLatencyMs.okMax must not exceed monitorMax",
        ),
        (
            "display.bands.smileIntensityPct.monitorMin",
            b.smile_intensity_pct.monitor_min,
            b.smile_intensity_pct.ok_min,
            "display.bands.smileIntensityPct.monitorMin must not exceed okMin",
        ),
        (
            "sessionQuality.acceptableMinValidRatio",
            q.acceptable_min_valid_ratio,
            q.optimal_min_valid_ratio,
            "sessionQuality.acceptableMinValidRatio must not exceed optimalMinValidRatio",
        ),
        // WP-X.19b: the replay blink window.
        (
            "metrics.blinkMinSec",
            p.metrics.blink_min_sec,
            p.metrics.blink_max_sec,
            "metrics.blinkMinSec must not exceed blinkMaxSec",
        ),
    ] {
        if lower.is_finite() && upper.is_finite() && lower > upper {
            problems.push(ProfileProblem::new(path, message));
        }
    }
    // WP-A.13: the delivered gaze-robustness parameters (SCH-3) the device pipeline runs on.
    if let Err(tracking) = p.tracking_quality.validate() {
        problems.extend(tracking.into_iter().map(|t| ProfileProblem {
            path: format!("trackingQuality.{}", t.path),
            message: format!("trackingQuality.{}", t.message),
        }));
    }
    problems
}

/// Every problem of a typed profile: the parameter registry first (kind, range, PRM-3), then
/// the cross-field rules of [`scoring_profile_problems`] for members the registry did not
/// already refuse. The same answer as `params::validate` on the serialised profile; callers
/// moving off [`scoring_profile_problems`] use this (WP-X.0).
pub fn profile_problems(p: &ScoringProfile) -> Vec<ProfileProblem> {
    match serde_json::to_value(p) {
        Ok(body) => crate::params::validate(crate::params::DocumentKind::ScoringProfile, &body),
        Err(e) => vec![ProfileProblem::new(
            "",
            format!("profile does not serialise: {e}"),
        )],
    }
}

/// [`scoring_profile_problems`] as messages only (the backend's `invalid_params` reasons).
pub fn scoring_profile_problem_messages(p: &ScoringProfile) -> Vec<String> {
    scoring_profile_problems(p)
        .into_iter()
        .map(|p| p.message)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(p: &ScoringProfile) -> Vec<String> {
        scoring_profile_problems(p)
            .into_iter()
            .map(|p| p.path)
            .collect()
    }

    #[test]
    fn default_profile_is_valid() {
        assert!(scoring_profile_problems(&ScoringProfile::default()).is_empty());
    }

    #[test]
    fn unit_ranges_name_their_member() {
        let mut p = ScoringProfile::default();
        p.flag.green_floor = 1.5;
        p.flag.low_confidence_share = -0.1;
        p.low_confidence_valid_ratio = 2.0;
        p.quality_gate.blink_threshold = f64::NAN;
        let problems = scoring_profile_problems(&p);
        assert_eq!(
            paths(&p),
            [
                "flag.greenFloor",
                "flag.lowConfidenceShare",
                "lowConfidenceValidRatio",
                "qualityGate.blinkThreshold"
            ]
        );
        assert_eq!(problems[0].message, "flag.greenFloor must be in [0, 1]");
    }

    #[test]
    fn cross_field_rules() {
        let mut p = ScoringProfile::default();
        p.flag.yellow_floor = 0.9;
        assert_eq!(paths(&p), ["flag.yellowFloor"]);
        assert_eq!(
            scoring_profile_problem_messages(&p),
            ["flag.yellowFloor must not exceed flag.greenFloor"]
        );

        let mut p = ScoringProfile::default();
        p.quality_gate.min_distance_cm = 80.0;
        assert_eq!(paths(&p), ["qualityGate.maxDistanceCm"]);
        p.quality_gate.min_distance_cm = 0.0;
        assert_eq!(paths(&p), ["qualityGate.minDistanceCm"]);

        let mut p = ScoringProfile::default();
        p.quality_gate.max_pitch_rad = 0.0;
        assert_eq!(paths(&p), ["qualityGate.maxPitchRad"]);
        p.quality_gate.max_yaw_rad = -1.0;
        assert_eq!(paths(&p), ["qualityGate.maxYawRad"]);
        assert_eq!(
            scoring_profile_problem_messages(&p),
            ["qualityGate.maxYawRad and maxPitchRad must be positive"]
        );

        let mut p = ScoringProfile::default();
        p.metrics.blink_min_sec = 0.6;
        assert_eq!(paths(&p), ["metrics.blinkMinSec"]);

        let p = ScoringProfile {
            schema_version: "  ".into(),
            ..Default::default()
        };
        assert_eq!(paths(&p), ["schemaVersion"]);
    }

    #[test]
    fn quality_gate_values_must_be_finite() {
        let mut p = ScoringProfile::default();
        p.quality_gate.max_distance_cm = f64::INFINITY;
        assert_eq!(paths(&p), ["qualityGate"]);
        assert_eq!(
            scoring_profile_problem_messages(&p),
            ["qualityGate distances and angles must be finite numbers"]
        );
    }

    /// WP-A.13: `trackingQuality` value rules (asd-core `TrackingQualityParams::validate`)
    /// are part of the profile checks, so backend publish and the console editor refuse what
    /// the device would refuse.
    #[test]
    fn tracking_quality_problems_are_profile_problems() {
        let mut p = ScoringProfile::default();
        p.tracking_quality.calibration.layout5.clear();
        p.tracking_quality.calibration.min_samples_per_target = 0;
        p.tracking_quality.confidence.distance_comfort_min_cm = 90.0;
        assert_eq!(
            paths(&p),
            [
                "trackingQuality.calibration.minSamplesPerTarget",
                "trackingQuality.calibration.layout5",
                "trackingQuality.confidence.distanceComfortMinCm"
            ]
        );
        assert_eq!(
            scoring_profile_problem_messages(&p)[1],
            "trackingQuality.calibration.layout5 must not be empty"
        );
    }
}
