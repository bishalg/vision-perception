//! owner: WP-R.1
//!
//! Golden parity for `quality::evaluate_sample` (CORE-7, FFI-4). Vectors in
//! `tests/golden/quality/*.json` were produced by the unchanged Swift `GazeQualityGate`;
//! the reason strings are shown in the UI and must match exactly.

use std::path::PathBuf;

use asd_core::quality::{self, QualityGate};
use asd_core::types::{GazeQualityVerdict, GazeTelemetrySample};
use serde_json::Value;

/// Decodes a sample as Swift encodes it (snake_case keys, `valid`).
fn swift_sample(v: &Value) -> GazeTelemetrySample {
    serde_json::from_value(v.clone()).expect("decode GazeTelemetrySample")
}

fn vectors() -> Vec<(String, Value, Value)> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden/quality/evaluate_sample_v1.json");
    let doc: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(doc["schema"], "asd-golden/1");
    assert_eq!(doc["function"], "evaluate_sample");
    doc["vectors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| {
            (
                v["name"].as_str().unwrap().to_owned(),
                v["input"].clone(),
                v["expected"].clone(),
            )
        })
        .collect()
}

#[test]
fn golden_evaluate_sample() {
    let all = vectors();
    assert!(all.len() >= 3);
    for (name, input, expected) in all {
        let gate: QualityGate = serde_json::from_value(input["gate"].clone())
            .unwrap_or_else(|e| panic!("{name}: gate: {e}"));
        let sample = swift_sample(&input["sample"]);
        let actual = serde_json::to_value(quality::evaluate_sample(&sample, &gate)).unwrap();
        assert_eq!(actual, expected, "{name}");
    }
}

#[test]
fn default_gate_equals_swift_init_defaults() {
    let g = QualityGate::default();
    assert_eq!(g.min_distance_cm, 35.0);
    assert_eq!(g.max_distance_cm, 65.0);
    assert_eq!(g.max_yaw_rad, 0.61);
    assert_eq!(g.max_pitch_rad, 0.52);
    assert_eq!(g.min_confidence, 0.50);
    assert_eq!(g.blink_threshold, 0.45);
}

#[test]
fn verdict_is_never_acceptable() {
    // Swift returns only optimal, degraded or rejected from evaluateSample.
    for (_, input, _) in vectors() {
        let gate: QualityGate = serde_json::from_value(input["gate"].clone()).unwrap();
        let e = quality::evaluate_sample(&swift_sample(&input["sample"]), &gate);
        assert_ne!(e.verdict, GazeQualityVerdict::Acceptable);
        assert_eq!(e.is_valid, e.failure_reasons.is_empty());
    }
}

/// Non-finite gate values must not panic (Swift `Int(Double)` would trap); the reason text
/// then carries a saturated number. This is a documented divergence, not a parity case.
#[test]
fn non_finite_gate_does_not_panic() {
    let gate = QualityGate {
        min_distance_cm: f64::NAN,
        max_distance_cm: f64::INFINITY,
        min_confidence: f64::INFINITY,
        ..QualityGate::default()
    };
    let (_, input, _) = vectors().remove(0);
    let e = quality::evaluate_sample(&swift_sample(&input["sample"]), &gate);
    assert!(!e.is_valid);
}
