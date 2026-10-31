//! owner: WP-G.3
//!
//! CNF-1 per-sample confidence (minimum of ramped factors, each exactly 0.5 at today's hard
//! limit, 0 beyond it) and CNF-3 per-fixation confidence. The hard limits are read from
//! `QualityGate` (`ScoringProfile.qualityGate`), never duplicated.

use std::path::PathBuf;

use asd_core::gaze::confidence::{
    self, factors, ramp_down, ramp_up, ConfidenceFeatures, ConfidenceLimits,
};
use asd_core::quality::{self, QualityGate};
use asd_core::types::tracking::{TrackState, TrackingQualityParams};
use asd_core::types::GazeTelemetrySample;
use serde_json::Value;

fn limits() -> ConfidenceLimits {
    ConfidenceLimits::new(&QualityGate::default(), &TrackingQualityParams::default())
}

/// A comfortable, tracked frame: every factor 1.
fn comfy() -> ConfidenceFeatures {
    ConfidenceFeatures {
        track_state: TrackState::Tracked,
        left_eye_openness: Some(0.9),
        right_eye_openness: Some(0.95),
        head_yaw_rad: Some(0.05),
        head_pitch_rad: Some(-0.05),
        distance_cm: Some(48.0),
        head_speed_deg_per_sec: Some(10.0),
        pose_shift_pct: Some(3.0),
    }
}

#[test]
fn comfortable_frame_is_one() {
    assert_eq!(confidence::sample(&comfy(), &limits()), 1.0);
}

#[test]
fn limits_come_from_the_quality_gate() {
    let gate = QualityGate {
        min_distance_cm: 30.0,
        max_distance_cm: 70.0,
        max_yaw_rad: 0.5,
        max_pitch_rad: 0.4,
        min_confidence: 0.5,
        blink_threshold: 0.3,
    };
    let l = ConfidenceLimits::new(&gate, &TrackingQualityParams::default());
    assert_eq!(l.gate, gate);
    let mut f = comfy();
    f.distance_cm = Some(30.0);
    assert_eq!(factors(&f, &l).distance, 0.5);
    f.distance_cm = Some(70.0);
    assert_eq!(factors(&f, &l).distance, 0.5);
    f.head_yaw_rad = Some(-0.5);
    assert_eq!(factors(&f, &l).yaw, 0.5);
    f.head_pitch_rad = Some(0.4);
    assert_eq!(factors(&f, &l).pitch, 0.5);
    f.left_eye_openness = Some(0.3);
    assert_eq!(factors(&f, &l).eye_openness, 0.5);
}

/// (name, set the feature to its limit, read the factor).
type LimitCase = (
    &'static str,
    Box<dyn Fn(&mut ConfidenceFeatures)>,
    fn(&confidence::ConfidenceFactors) -> f64,
);

#[test]
fn every_factor_is_exactly_half_at_its_hard_limit() {
    let l = limits();
    let g = QualityGate::default();
    let p = TrackingQualityParams::default();
    let cases: Vec<LimitCase> = vec![
        (
            "eye",
            Box::new(move |f| f.right_eye_openness = Some(g.blink_threshold)),
            |c| c.eye_openness,
        ),
        (
            "yaw+",
            Box::new(move |f| f.head_yaw_rad = Some(g.max_yaw_rad)),
            |c| c.yaw,
        ),
        (
            "yaw-",
            Box::new(move |f| f.head_yaw_rad = Some(-g.max_yaw_rad)),
            |c| c.yaw,
        ),
        (
            "pitch+",
            Box::new(move |f| f.head_pitch_rad = Some(g.max_pitch_rad)),
            |c| c.pitch,
        ),
        (
            "pitch-",
            Box::new(move |f| f.head_pitch_rad = Some(-g.max_pitch_rad)),
            |c| c.pitch,
        ),
        (
            "near",
            Box::new(move |f| f.distance_cm = Some(g.min_distance_cm)),
            |c| c.distance,
        ),
        (
            "far",
            Box::new(move |f| f.distance_cm = Some(g.max_distance_cm)),
            |c| c.distance,
        ),
        (
            "head speed",
            Box::new(move |f| {
                f.head_speed_deg_per_sec = Some(p.confidence.max_head_speed_deg_per_sec)
            }),
            |c| c.head_speed,
        ),
        (
            "pose shift",
            Box::new(move |f| f.pose_shift_pct = Some(p.recalibration.pose_shift_distance_pct)),
            |c| c.pose_shift,
        ),
    ];
    for (name, set, get) in cases {
        let mut f = comfy();
        set(&mut f);
        let c = factors(&f, &l);
        assert_eq!(get(&c), 0.5, "{name}");
        assert_eq!(confidence::sample(&f, &l), 0.5, "{name}: min composition");
    }
}

#[test]
fn factors_are_one_in_comfort_and_zero_beyond_the_limit() {
    let l = limits();
    // Comfort edges: openness 0.70, |pose| 0.6 x limit, distance 40..60, speed 30, shift 10 %.
    let mut f = comfy();
    f.left_eye_openness = Some(0.70);
    f.head_yaw_rad = Some(0.6 * 0.61);
    f.head_pitch_rad = Some(-0.6 * 0.52);
    f.distance_cm = Some(40.0);
    f.head_speed_deg_per_sec = Some(30.0);
    f.pose_shift_pct = Some(10.0);
    let c = factors(&f, &l);
    assert_eq!(
        [
            c.eye_openness,
            c.yaw,
            c.pitch,
            c.distance,
            c.head_speed,
            c.pose_shift
        ],
        [1.0; 6]
    );
    f.distance_cm = Some(60.0);
    assert_eq!(factors(&f, &l).distance, 1.0);

    // Just beyond each limit: 0.
    let beyond = [
        (
            "eye",
            ConfidenceFeatures {
                left_eye_openness: Some(0.4499),
                ..comfy()
            },
        ),
        (
            "yaw",
            ConfidenceFeatures {
                head_yaw_rad: Some(0.6101),
                ..comfy()
            },
        ),
        (
            "pitch",
            ConfidenceFeatures {
                head_pitch_rad: Some(-0.5201),
                ..comfy()
            },
        ),
        (
            "near",
            ConfidenceFeatures {
                distance_cm: Some(34.99),
                ..comfy()
            },
        ),
        (
            "far",
            ConfidenceFeatures {
                distance_cm: Some(65.01),
                ..comfy()
            },
        ),
        (
            "speed",
            ConfidenceFeatures {
                head_speed_deg_per_sec: Some(120.01),
                ..comfy()
            },
        ),
        (
            "shift",
            ConfidenceFeatures {
                pose_shift_pct: Some(20.01),
                ..comfy()
            },
        ),
    ];
    for (name, f) in beyond {
        assert_eq!(confidence::sample(&f, &l), 0.0, "{name}");
    }
}

#[test]
fn ramps_are_linear_between_comfort_and_limit() {
    // Halfway between comfort and limit: 0.75.
    assert_eq!(ramp_down(45.0, 30.0, 60.0), 0.75);
    assert!((ramp_up(0.575, 0.70, 0.45) - 0.75).abs() < 1e-12);
    // Monotone.
    let mut prev = 1.0;
    for i in 0..=100 {
        let v = ramp_down(30.0 + 0.9 * i as f64, 30.0, 120.0);
        assert!(v <= prev && v >= 0.5, "{i}: {v}");
        prev = v;
    }
    // Non-finite input → 0.
    assert_eq!(ramp_down(f64::NAN, 30.0, 120.0), 0.0);
    assert_eq!(ramp_up(f64::INFINITY, 0.7, 0.45), 1.0);
    assert_eq!(ramp_up(f64::NAN, 0.7, 0.45), 0.0);
}

#[test]
fn unmeasured_features_do_not_lower_confidence() {
    let f = ConfidenceFeatures {
        head_speed_deg_per_sec: None,
        pose_shift_pct: None,
        ..comfy()
    };
    assert_eq!(confidence::sample(&f, &limits()), 1.0);
}

#[test]
fn any_state_other_than_tracked_is_zero() {
    for s in [
        TrackState::Blink,
        TrackState::Lost,
        TrackState::Offscreen,
        TrackState::Artifact,
    ] {
        let f = ConfidenceFeatures {
            track_state: s,
            ..comfy()
        };
        assert_eq!(confidence::sample(&f, &limits()), 0.0, "{s:?}");
    }
}

#[test]
fn min_composition_takes_the_worst_factor() {
    let f = ConfidenceFeatures {
        distance_cm: Some(62.5),            // 0.75
        head_speed_deg_per_sec: Some(84.0), // 1 - 0.5 * 54/90 = 0.7
        ..comfy()
    };
    let c = factors(&f, &limits());
    assert_eq!(c.distance, 0.75);
    assert!((c.head_speed - 0.7).abs() < 1e-15);
    assert_eq!(confidence::sample(&f, &limits()), c.head_speed);
}

/// CNF-1: the computed confidence reproduces the CORE-7 pass/fail of every existing
/// `golden/quality` input on distance, pose and blink (the sample's own constant
/// `confidence` is replaced by the computed one).
#[test]
fn computed_confidence_reproduces_core7_pass_fail_on_golden_quality_inputs() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden/quality/evaluate_sample_v1.json");
    let doc: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let vectors = doc["vectors"].as_array().unwrap();
    assert!(vectors.len() >= 3);
    let mut checked = 0;
    for v in vectors {
        let name = v["name"].as_str().unwrap();
        let gate: QualityGate = serde_json::from_value(v["input"]["gate"].clone()).unwrap();
        let mut s: GazeTelemetrySample =
            serde_json::from_value(v["input"]["sample"].clone()).unwrap();
        let l = ConfidenceLimits::new(&gate, &TrackingQualityParams::default());
        let f = ConfidenceFeatures {
            track_state: TrackState::Tracked,
            left_eye_openness: Some(s.left_eye_openness),
            right_eye_openness: Some(s.right_eye_openness),
            head_yaw_rad: Some(s.head_yaw_rad),
            head_pitch_rad: Some(s.head_pitch_rad),
            distance_cm: Some(s.distance_to_screen_cm),
            head_speed_deg_per_sec: None,
            pose_shift_pct: None,
        };
        let c = confidence::sample(&f, &l);

        // Gate verdict on distance, pose and blink only (its confidence check disabled).
        s.confidence = 1.0;
        let gate_pass = quality::evaluate_sample(&s, &gate).is_valid;
        assert_eq!(
            c >= gate.min_confidence,
            gate_pass,
            "{name}: confidence {c}"
        );

        // With the computed confidence written into the sample (CNF-4), the gate's verdict is
        // unchanged.
        s.confidence = c;
        assert_eq!(
            quality::evaluate_sample(&s, &gate).is_valid,
            gate_pass,
            "{name}"
        );

        // Where the golden's failure is not the old constant confidence alone, the golden's
        // own isValid is reproduced.
        let reasons: Vec<&str> = v["expected"]["failureReasons"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r.as_str().unwrap())
            .filter(|r| !r.starts_with("Low tracking confidence"))
            .collect();
        assert_eq!(reasons.is_empty(), gate_pass, "{name}");
        checked += 1;
    }
    assert_eq!(checked, vectors.len());
}

#[test]
fn fixation_confidence_is_the_mean_and_low_below_min_confidence() {
    assert_eq!(confidence::fixation([1.0, 0.5, 0.75]), Some(0.75));
    assert_eq!(confidence::fixation(std::iter::empty()), None);
    let g = QualityGate::default();
    assert!(confidence::is_low_confidence(0.49, &g));
    assert!(!confidence::is_low_confidence(0.5, &g));
}
