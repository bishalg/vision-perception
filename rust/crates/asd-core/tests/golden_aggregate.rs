//! owner: WP-R.2
//!
//! CORE-6 / FFI-4: every `tests/golden/aggregate/*.json` vector is `{input, expected}`
//! produced by the Swift oracle (`SessionBiomarkerAggregator.aggregate` at 9655fcc).
//! Unmeasured values (`eyeDwellRatio`, `mouthDwellRatio`, `firstOrientLatencyMs`) are absent
//! keys in `expected`, never a default.

use std::path::PathBuf;

use asd_core::aggregate::{self, SessionAggregates};
use asd_core::metrics::AoiRole;
use asd_core::types::{GazeFrame, StimulusCatalog};
use serde_json::{json, Value};

#[test]
fn golden_aggregate_vectors_match_swift_oracle() {
    let cases = load_dir("aggregate");
    assert!(
        cases.len() >= 13,
        "expected at least 13 aggregate vectors, found {}",
        cases.len()
    );
    for (name, case) in &cases {
        assert_eq!(case["function"], "session", "{name}");
        let input = &case["input"];
        let frames: Vec<GazeFrame> = serde_json::from_value(input["validFrames"].clone()).unwrap();
        let result = aggregate::session(&frames, input["durationSec"].as_f64().unwrap());
        let actual = serde_json::to_value(&result).unwrap();
        assert_json_close(name, "$", &case["expected"], &actual);
        let decoded: SessionAggregates = serde_json::from_value(case["expected"].clone()).unwrap();
        let back = serde_json::to_value(&decoded).unwrap();
        assert_json_close(name, "$roundtrip", &case["expected"], &back);
    }
}

fn frame(t: f64, names: &[&str]) -> GazeFrame {
    serde_json::from_value(json!({
        "gaze": {"valid": true, "x_norm": 0.4, "y_norm": 0.4, "video_time_sec": t},
        "activeAOINames": names,
        "qualityVerdict": "optimal",
    }))
    .unwrap()
}

#[test]
fn latency_is_none_when_never_oriented_never_a_default() {
    let r = aggregate::session(&[frame(0.0, &["Geometric_Pattern"])], 10.0);
    assert_eq!(r.first_orient_latency_ms, None);
    assert_eq!(r.eye_dwell_ratio, None);
    assert_eq!(r.mouth_dwell_ratio, None);
    let v = serde_json::to_value(&r).unwrap();
    assert!(
        v.get("firstOrientLatencyMs").is_none(),
        "absent key, not null: {v}"
    );
    assert!(v.get("eyeDwellRatio").is_none());
}

#[test]
fn empty_session_defaults() {
    let r = aggregate::session(&[], 5.0);
    assert_eq!(
        r,
        SessionAggregates {
            social_dwell_sec: 0.0,
            geometric_dwell_sec: 0.0,
            social_preference_score: 0.0,
            eye_dwell_ratio: None,
            mouth_dwell_ratio: None,
            first_orient_latency_ms: None,
            social_smile_intensity_mean: 0.0,
            smile_reactivity_index: 0.0,
            flat_affect_score: 1.0,
            surprise_score_mean: 0.0,
        }
    );
}

#[test]
fn catalog_roles_feed_the_session_aggregate() {
    let catalog: StimulusCatalog = serde_json::from_value(json!({
        "schemaVersion": "t", "languages": ["en"], "fallbackLanguage": "en", "agePacks": [],
        "clips": [{
            "id": "c", "title": "t", "category": "x", "durationSec": 1.0,
            "minMonths": 0, "maxMonths": 99, "protocols": [],
            "areas": [{"name": "Grandma_Face", "startSec": 0.0, "endSec": 1.0,
                       "xMin": 0.0, "yMin": 0.0, "xMax": 1.0, "yMax": 1.0,
                       "roles": ["social", "orienting"]}]
        }]
    }))
    .unwrap();
    let frames = [frame(0.3, &["Grandma_Face"])];
    let with = aggregate::session_with_roles(&frames, 1.0, &AoiRole::from_catalog(&catalog));
    assert_eq!(with.first_orient_latency_ms, Some(300.0));
    assert_eq!(with.social_preference_score, 1.0);
    let without = aggregate::session(&frames, 1.0);
    assert_eq!(without.first_orient_latency_ms, None);
    assert_eq!(without.social_preference_score, 0.0);
}

// --- Golden loader and tolerant comparison (floats to 1e-9; keys, strings, order exact).
// Duplicated in golden_metrics.rs and golden_aggregate.rs so each test file stays self-contained.

const TOLERANCE: f64 = 1e-9;

/// Every `tests/golden/<dir>/*.json`, sorted by file name.
fn load_dir(dir: &str) -> Vec<(String, Value)> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(dir);
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&root)
        .unwrap_or_else(|e| panic!("read {}: {e}", root.display()))
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    paths.sort();
    paths
        .into_iter()
        .map(|p| {
            let name = p.file_stem().unwrap().to_string_lossy().into_owned();
            let text = std::fs::read_to_string(&p).unwrap();
            let value: Value =
                serde_json::from_str(&text).unwrap_or_else(|e| panic!("{name}: {e}"));
            (name, value)
        })
        .collect()
}

/// Panics with the JSON path of the first difference.
fn assert_json_close(case: &str, path: &str, expected: &Value, actual: &Value) {
    match (expected, actual) {
        (Value::Number(e), Value::Number(a)) => {
            if e.is_f64() || a.is_f64() {
                let (e, a) = (e.as_f64().unwrap(), a.as_f64().unwrap());
                assert!(
                    (e - a).abs() <= TOLERANCE,
                    "{case} {path}: expected {e}, got {a} (diff {})",
                    (e - a).abs()
                );
            } else {
                assert_eq!(e, a, "{case} {path}");
            }
        }
        (Value::Object(e), Value::Object(a)) => {
            let ek: Vec<&String> = e.keys().collect();
            let ak: Vec<&String> = a.keys().collect();
            assert_eq!(ek, ak, "{case} {path}: key sets differ");
            for (k, ev) in e {
                assert_json_close(case, &format!("{path}.{k}"), ev, &a[k]);
            }
        }
        (Value::Array(e), Value::Array(a)) => {
            assert_eq!(e.len(), a.len(), "{case} {path}: array length");
            for (i, (ev, av)) in e.iter().zip(a).enumerate() {
                assert_json_close(case, &format!("{path}[{i}]"), ev, av);
            }
        }
        _ => assert_eq!(expected, actual, "{case} {path}"),
    }
}
