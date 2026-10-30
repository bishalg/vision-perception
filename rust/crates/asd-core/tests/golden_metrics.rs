//! owner: WP-R.2
//!
//! CORE-5 / FFI-4: every `tests/golden/metrics/*.json` vector is `{input, expected}` produced
//! by the Swift oracle (`PerVideoMetricsExtractor` at 9655fcc); the Rust port must reproduce
//! `expected` exactly for keys, strings and integers and to 1e-9 for floats.
//!
//! Plus unit tests for the parts the vectors cannot reach (catalog-driven AOI roles, key
//! composition).

use std::collections::BTreeSet;
use std::path::PathBuf;

use asd_core::metrics::{
    self, AoiRole, AoiRoleKind, FixationStats, PerVideoMetricKey, UNTAGGED_CLIP_ID,
};
use asd_core::types::{GazeFrame, PerVideoMetricSet, StimulusCatalog};
use serde_json::{json, Value};

const CATALOG: &str = include_str!("fixtures/stimulus_catalog.json");

fn frames_of(input: &Value, key: &str) -> Vec<GazeFrame> {
    serde_json::from_value(input[key].clone()).expect("frames decode into GazeFrame")
}

#[test]
fn golden_metrics_vectors_match_swift_oracle() {
    let cases = load_dir("metrics");
    assert!(
        cases.len() >= 23,
        "expected at least 23 metrics vectors, found {}",
        cases.len()
    );
    for (name, case) in &cases {
        let input = &case["input"];
        let actual = match case["function"].as_str() {
            Some("extract") => {
                let sets = metrics::extract(&frames_of(input, "frames"));
                serde_json::to_value(sets).unwrap()
            }
            Some("extract_clip") => {
                let set = metrics::extract_clip(
                    input["stimulusId"].as_str().unwrap(),
                    &frames_of(input, "frames"),
                    input["timeOriginSec"].as_f64().unwrap(),
                );
                serde_json::to_value(set).unwrap()
            }
            other => panic!("{name}: unknown function {other:?}"),
        };
        assert_json_close(name, "$", &case["expected"], &actual);
        // `expected` must itself decode as the typed struct (shape check on the oracle side).
        match case["function"].as_str() {
            Some("extract") => {
                let _: Vec<PerVideoMetricSet> =
                    serde_json::from_value(case["expected"].clone()).unwrap();
            }
            _ => {
                let _: PerVideoMetricSet =
                    serde_json::from_value(case["expected"].clone()).unwrap();
            }
        }
    }
}

// --- Traps called out in work-packages.md, asserted directly as well as via vectors.

fn frame(stimulus: Option<&str>, media: Option<f64>, x: f64, names: &[&str]) -> GazeFrame {
    let mut v = json!({
        "gaze": {"valid": true, "x_norm": x, "y_norm": 0.5, "video_time_sec": media.unwrap_or(0.0)},
        "affect": {},
        "activeAOINames": names,
        "qualityVerdict": "optimal",
    });
    if let Some(m) = media {
        v["gaze"]["clip_media_time_sec"] = json!(m);
    }
    if let Some(s) = stimulus {
        v["stimulusId"] = json!(s);
    }
    serde_json::from_value(v).unwrap()
}

#[test]
fn group_order_is_first_appearance_not_sorted() {
    let frames = vec![
        frame(Some("zeta"), Some(0.1), 0.5, &[]),
        frame(None, Some(0.1), 0.5, &[]),
        frame(Some("alpha"), Some(0.1), 0.5, &[]),
        frame(Some("zeta"), Some(0.2), 0.5, &[]),
    ];
    let ids: Vec<String> = metrics::extract(&frames)
        .into_iter()
        .map(|s| s.stimulus_id)
        .collect();
    assert_eq!(ids, vec!["zeta", UNTAGGED_CLIP_ID, "alpha"]);
    assert_eq!(UNTAGGED_CLIP_ID, "session_untagged");
}

#[test]
fn dt_zero_or_negative_closes_the_run_and_restarts() {
    // 10 steady frames, then the clock jumps back: two fixation runs, not one.
    let mut frames: Vec<GazeFrame> = (0..10)
        .map(|i| frame(Some("c"), Some(f64::from(i) / 60.0), 0.5, &[]))
        .collect();
    frames.extend((0..10).map(|i| frame(Some("c"), Some(f64::from(i) / 60.0), 0.5, &[])));
    let FixationStats {
        count,
        mean_duration_ms,
    } = metrics::fixation_stats(&frames);
    assert_eq!(count, 2);
    assert!(
        (mean_duration_ms - 150.0).abs() < 1e-9,
        "{mean_duration_ms}"
    );
}

#[test]
fn fixation_stats_degenerate_inputs() {
    assert_eq!(
        metrics::fixation_stats(&[]),
        FixationStats {
            count: 0,
            mean_duration_ms: 0.0
        }
    );
    let one = [frame(None, Some(0.0), 0.5, &[])];
    assert_eq!(metrics::fixation_stats(&one).count, 1);
    assert!((metrics::fixation_stats(&one).mean_duration_ms - 80.0).abs() < 1e-9);
}

#[test]
fn observed_duration_prefers_positive_media_clock() {
    let with_media = [
        frame(None, Some(0.0), 0.5, &[]),
        frame(None, Some(2.25), 0.5, &[]),
    ];
    assert_eq!(metrics::observed_duration_sec(&with_media), 2.25);
    let zero_media = [
        frame(None, Some(0.0), 0.5, &[]),
        frame(None, Some(0.0), 0.5, &[]),
    ];
    assert!((metrics::observed_duration_sec(&zero_media) - 2.0 / 60.0).abs() < 1e-15);
    let no_media = vec![frame(None, None, 0.5, &[]); 3];
    assert!((metrics::observed_duration_sec(&no_media) - 3.0 / 60.0).abs() < 1e-15);
}

#[test]
fn missing_affect_values_read_as_neutral_zero() {
    let set = metrics::extract_clip("c", &[frame(Some("c"), Some(0.1), 0.5, &[])], 0.0);
    assert_eq!(set.metrics["social_smile_intensity_mean"], 0.0);
    assert_eq!(set.metrics["flat_affect_score"], 1.0);
    assert_eq!(set.metrics["surprise_score_mean"], 0.0);
}

#[test]
fn metric_key_composition_matches_swift() {
    assert_eq!(
        PerVideoMetricKey::FirstEntryLatencyMs.key(None),
        "first_entry_latency_ms"
    );
    assert_eq!(
        PerVideoMetricKey::FirstEntryLatencyMs.key(Some("")),
        "first_entry_latency_ms"
    );
    assert_eq!(
        PerVideoMetricKey::AoiDwellRatio.key(Some("Gaze_Target_Right")),
        "aoi_dwell_ratio::Gaze_Target_Right"
    );
    let raw: Vec<&str> = PerVideoMetricKey::ALL.iter().map(|k| k.as_str()).collect();
    assert_eq!(
        raw,
        vec![
            "social_dwell_ratio",
            "geometric_dwell_ratio",
            "social_preference_score",
            "eye_dwell_ratio",
            "mouth_dwell_ratio",
            "aoi_dwell_ratio",
            "aoi_dwell_window_ratio",
            "first_entry_latency_ms",
            "fixation_count",
            "mean_fixation_duration_ms",
            "social_smile_intensity_mean",
            "smile_reactivity_index",
            "flat_affect_score",
            "surprise_score_mean",
            "valid_sample_ratio",
        ]
    );
}

// --- AOI roles: catalog data first, hard-coded sets second (CORE-5).

#[test]
fn builtin_roles_match_swift_sets() {
    let r = AoiRole::builtin();
    for name in ["Eye_Region", "Actor_Eyes"] {
        assert!(r.matches(name, AoiRoleKind::Eye), "{name}");
    }
    for name in ["Mouth_Region", "Teacher_Mouth"] {
        assert!(r.matches(name, AoiRoleKind::Mouth), "{name}");
    }
    assert!(r.matches("Face_Bounding_Box", AoiRoleKind::Social));
    assert!(r.matches("Gift_Box", AoiRoleKind::Geometric));
    assert!(!r.matches("Teacher_Mouth", AoiRoleKind::Orienting));
    assert!(!r.matches("Cartoon_Character", AoiRoleKind::Orienting));
    assert!(!r.matches("Animal_Puppies", AoiRoleKind::Social));
    assert!(!r.matches("Gaze_Target_Left", AoiRoleKind::Social));
    assert_eq!(AoiRoleKind::Orienting.as_str(), "orienting");
}

/// Swift's `extract` consults the bundled catalog; Rust's `extract` uses built-ins only. That
/// is equivalent only while the bundled catalog adds no role the built-ins lack. This guards it.
#[test]
fn bundled_catalog_roles_are_covered_by_builtins() {
    let catalog: StimulusCatalog = serde_json::from_str(CATALOG).unwrap();
    let builtin = AoiRole::builtin();
    let kinds = [
        AoiRoleKind::Social,
        AoiRoleKind::Geometric,
        AoiRoleKind::Eye,
        AoiRoleKind::Mouth,
        AoiRoleKind::Orienting,
    ];
    let mut seen = BTreeSet::new();
    for clip in &catalog.clips {
        for area in &clip.areas {
            for role in &area.roles {
                let kind = kinds.iter().find(|k| k.as_str() == role);
                let Some(kind) = kind else { continue };
                assert!(
                    builtin.matches(&area.name, *kind),
                    "{} lacks {role}",
                    area.name
                );
                seen.insert((area.name.clone(), role.clone()));
            }
        }
    }
    assert!(!seen.is_empty());
}

#[test]
fn catalog_roles_extend_builtins() {
    let catalog: StimulusCatalog = serde_json::from_value(json!({
        "schemaVersion": "t", "languages": ["en"], "fallbackLanguage": "en", "agePacks": [],
        "clips": [{
            "id": "c", "title": "t", "category": "x", "durationSec": 1.0,
            "minMonths": 0, "maxMonths": 99, "protocols": [],
            "areas": [{"name": "Custom_Eyes", "startSec": 0.0, "endSec": 1.0,
                       "xMin": 0.0, "yMin": 0.0, "xMax": 1.0, "yMax": 1.0,
                       "roles": ["eye", "orienting"]}]
        }]
    }))
    .unwrap();
    let roles = AoiRole::from_catalog(&catalog);
    assert!(roles.matches("Custom_Eyes", AoiRoleKind::Eye));
    assert!(
        roles.matches("Eye_Region", AoiRoleKind::Eye),
        "built-ins still apply"
    );
    assert!(!AoiRole::builtin().matches("Custom_Eyes", AoiRoleKind::Eye));
    assert!(roles.intersects(
        &["x".to_string(), "Custom_Eyes".to_string()],
        AoiRoleKind::Orienting
    ));

    let frames = [frame(Some("c"), Some(0.1), 0.5, &["Custom_Eyes"])];
    let with = metrics::extract_with_roles(&frames, &roles);
    assert_eq!(with[0].metrics["eye_dwell_ratio"], 1.0);
    let without = metrics::extract(&frames);
    assert_eq!(without[0].metrics["eye_dwell_ratio"], 0.0);
    let clip = metrics::extract_clip_with_roles("c", &frames, 0.0, &roles);
    assert_eq!(clip, with[0]);
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
