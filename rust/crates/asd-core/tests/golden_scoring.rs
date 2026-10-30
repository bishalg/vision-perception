//! owner: WP-R.1
//!
//! Golden parity for `scoring` and `baselines` (CORE-1..3, FFI-4).
//!
//! Vectors in `tests/golden/scoring/*.json` were produced by the unchanged Swift sources
//! (see each file's `derivation`). Floats compare to 1e-9, strings and enums exactly.
//! Frames and samples in the vectors are in the Swift wire format (`JSONEncoder` of
//! `GazeFrame`) and decode directly into the Rust types.

use std::path::PathBuf;

use asd_core::types::{
    AgeBracket, GazeFrame, NormativeBaselineSet, NormativeDirection, NormativeRange,
    NormativeVideo, PerVideoMetricSet,
};
use asd_core::{baselines, scoring};
use serde::de::DeserializeOwned;
use serde_json::{json, Value};

const TOLERANCE: f64 = 1e-9;

struct Vector {
    name: String,
    input: Value,
    expected: Value,
}

fn load(file: &str, function: &str) -> Vec<Vector> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden/scoring")
        .join(file);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path:?}: {e}"));
    let doc: Value = serde_json::from_str(&text).expect("golden file is JSON");
    assert_eq!(doc["schema"], "asd-golden/1", "{file}");
    assert_eq!(doc["function"], function, "{file}");
    let vectors: Vec<Vector> = doc["vectors"]
        .as_array()
        .expect("vectors array")
        .iter()
        .map(|v| Vector {
            name: v["name"].as_str().expect("name").to_owned(),
            input: v["input"].clone(),
            expected: v["expected"].clone(),
        })
        .collect();
    assert!(!vectors.is_empty(), "{file} has no vectors");
    vectors
}

fn decode<T: DeserializeOwned>(v: &Value, what: &str) -> T {
    serde_json::from_value(v.clone()).unwrap_or_else(|e| panic!("decode {what}: {e}"))
}

fn frames(v: &Value) -> Vec<GazeFrame> {
    v.as_array()
        .expect("frames array")
        .iter()
        .map(|f| decode(f, "GazeFrame"))
        .collect()
}

fn assert_close(actual: &Value, expected: &Value, path: &str) {
    match (actual, expected) {
        (Value::Number(a), Value::Number(e)) => {
            let (a, e) = (a.as_f64().unwrap(), e.as_f64().unwrap());
            assert!(
                (a - e).abs() <= TOLERANCE,
                "{path}: {a} != {e} (tolerance {TOLERANCE})"
            );
        }
        (Value::Object(a), Value::Object(e)) => {
            let ak: Vec<_> = a.keys().collect();
            let ek: Vec<_> = e.keys().collect();
            assert_eq!(ak, ek, "{path}: key sets differ");
            for (k, ev) in e {
                assert_close(&a[k], ev, &format!("{path}.{k}"));
            }
        }
        (Value::Array(a), Value::Array(e)) => {
            assert_eq!(a.len(), e.len(), "{path}: array lengths differ");
            for (i, (av, ev)) in a.iter().zip(e).enumerate() {
                assert_close(av, ev, &format!("{path}[{i}]"));
            }
        }
        _ => assert_eq!(actual, expected, "{path}"),
    }
}

#[test]
fn golden_evaluate() {
    for v in load("evaluate_v1.json", "evaluate") {
        let observed = v.input["observed"].as_f64().unwrap();
        let direction: NormativeDirection = decode(&v.input["direction"], "direction");
        let range: NormativeRange = decode(&v.input["range"], "range");
        let (status, score) = scoring::evaluate(observed, direction, &range);
        let actual = json!({ "status": status, "score": score });
        assert_close(&actual, &v.expected, &v.name);
    }
}

#[test]
fn golden_piecewise() {
    for v in load("piecewise_v1.json", "piecewise") {
        let score = scoring::piecewise(
            v.input["observed"].as_f64().unwrap(),
            v.input["goodEnd"].as_f64().unwrap(),
            v.input["badEnd"].as_f64().unwrap(),
        );
        assert_close(&json!({ "score": score }), &v.expected, &v.name);
    }
}

#[test]
fn golden_brackets() {
    for v in load("brackets_v1.json", "brackets") {
        let set = NormativeBaselineSet {
            schema_version: "t".into(),
            status: "t".into(),
            provenance: None,
            age_brackets: decode::<Vec<AgeBracket>>(&v.input["ageBrackets"], "ageBrackets"),
            videos: Default::default(),
        };
        let age = v.input["ageMonths"].as_i64().unwrap();
        let actual = json!({
            "bracketId": baselines::bracket_id_for(&set, age),
            "resolvedBracketId": scoring::resolved_bracket_id(&set, age),
        });
        assert_close(&actual, &v.expected, &v.name);
    }
}

fn run_score_video(file: &str) {
    for v in load(file, "score_video") {
        let metric_set: PerVideoMetricSet = decode(&v.input["metricSet"], "metricSet");
        let video: NormativeVideo = decode(&v.input["video"], "video");
        let bracket = v.input["bracketId"].as_str().unwrap();
        let clip_frames = frames(&v.input["clipFrames"]);
        let report = scoring::score_video(&metric_set, &video, bracket, &clip_frames);
        let actual = serde_json::to_value(&report).unwrap();
        assert_close(&actual, &v.expected, &v.name);
    }
}

#[test]
fn golden_score_video() {
    run_score_video("score_video_v1.json");
}

#[test]
fn golden_score_video_edges() {
    run_score_video("score_video_edges_v1.json");
}

#[test]
fn golden_score_video_segment_windows() {
    run_score_video("score_video_frames_v1.json");
}

#[test]
fn golden_score_session() {
    for v in load("score_session_v1.json", "score_session") {
        let metric_sets: Vec<PerVideoMetricSet> = decode(&v.input["metricSets"], "metricSets");
        let baseline: NormativeBaselineSet = decode(&v.input["baseline"], "baseline");
        let age = v.input["ageMonths"].as_i64().unwrap();
        let all_frames = frames(&v.input["frames"]);
        let scored_at = v.input["scoredAtUtc"].as_str().unwrap();
        let score = scoring::score_session(&metric_sets, &baseline, age, &all_frames, scored_at);
        let actual = serde_json::to_value(&score).unwrap();
        assert_close(&actual, &v.expected, &v.name);
    }
}

#[test]
fn golden_format_scored_at() {
    for v in load("scored_at_v1.json", "format_scored_at") {
        let secs = v.input["unixSeconds"].as_f64().unwrap();
        let actual = json!({ "scoredAtUtc": scoring::format_scored_at(secs) });
        assert_close(&actual, &v.expected, &v.name);
    }
}

#[test]
fn golden_validate_baselines() {
    for v in load("validate_baselines_v1.json", "validate_baselines") {
        let set: NormativeBaselineSet = decode(&v.input["baseline"], "baseline");
        let ids: Vec<String> = decode(&v.input["requiredVideoIds"], "requiredVideoIds");
        let actual = json!({ "problems": baselines::validate(&set, &ids) });
        assert_close(&actual, &v.expected, &v.name);
    }
}

/// The bundled fixture is the set the app ships: it must validate against the bundled
/// catalog with no problems (NormativeBaselineLibraryTests.testAllTenStimulusVideosHaveProfiles).
#[test]
fn bundled_baselines_cover_bundled_catalog() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let set: NormativeBaselineSet = serde_json::from_str(
        &std::fs::read_to_string(dir.join("normative_baselines.json")).unwrap(),
    )
    .unwrap();
    let catalog: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("stimulus_catalog.json")).unwrap())
            .unwrap();
    let ids: Vec<String> = catalog["clips"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(ids.len(), 10);
    assert!(baselines::validate(&set, &ids).is_empty());
    assert_eq!(set.videos.len(), ids.len());
}

#[test]
fn composed_key_matches_swift() {
    assert_eq!(scoring::composed_key("x", None), "x");
    assert_eq!(scoring::composed_key("x", Some("")), "x");
    assert_eq!(scoring::composed_key("x", Some("A")), "x::A");
}

#[test]
fn untagged_clip_id_is_exact() {
    assert_eq!(scoring::UNTAGGED_CLIP_ID, "session_untagged");
    assert_eq!(scoring::LOW_CONFIDENCE_VALID_RATIO, 0.60);
}
