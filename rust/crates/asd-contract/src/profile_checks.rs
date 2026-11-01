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
    if p.schema_version.trim().is_empty() {
        problems.push(ProfileProblem::new(
            "schemaVersion",
            "schemaVersion is required",
        ));
    }
    problems
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

        let p = ScoringProfile {
            schema_version: "  ".into(),
            ..Default::default()
        };
        assert_eq!(paths(&p), ["schemaVersion"]);
    }
}
