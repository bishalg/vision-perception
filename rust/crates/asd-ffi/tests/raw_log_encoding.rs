//! owner: WP-G.8 (audit fix)
//!
//! REP-1 encoding for the bench capture build happens in Rust, not Swift: the app collects
//! `RawSampleRecord`s and the encoders here write the `asd-raw-tracking/1` header and sample
//! lines. The committed WP-G.1 sample (`asd-core/tests/fixtures/raw_tracking_sample.jsonl`)
//! is the oracle, and every document written here parses with `asd_core::gaze::raw_log`
//! (what `asd-cli gaze replay --check` runs).

use std::path::PathBuf;

use asd_core::gaze::raw_log;
use asd_ffi::gaze::{GazeOrientation, GazeTargetKind, GazeVec3, RawSampleRecord};
use asd_ffi::raw_log::{
    gaze_bench_condition_name, gaze_bench_conditions, gaze_raw_log_document,
    gaze_raw_log_header_line, gaze_raw_log_line, gaze_scheduled_targets_json, GazeBenchCondition,
    GazeRawLogHeaderInput, GazeScheduledTarget,
};
use asd_ffi::AsdError;
use serde_json::Value;

fn fixture_lines() -> Vec<Value> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../asd-core/tests/fixtures/raw_tracking_sample.jsonl");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
        .lines()
        .map(|l| serde_json::from_str(l).expect("fixture line is JSON"))
        .collect()
}

fn translation(x: f32, y: f32, z: f32) -> Vec<f64> {
    // Column-major identity with a translation, single precision as ARKit hands it.
    let m: [f32; 16] = [
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, x, y, z, 1.0,
    ];
    m.iter().map(|&v| f64::from(v)).collect()
}

fn vec3(x: f32, y: f32, z: f32) -> GazeVec3 {
    GazeVec3 {
        x: f64::from(x),
        y: f64::from(y),
        z: f64::from(z),
    }
}

/// Line 2 of the fixture, rebuilt from ARKit-typed (f32) values the way the kit hands them
/// over (`Double(Float)`, so 0.004 arrives as 0.004000000189989805).
fn tracked(t_sec: f64) -> RawSampleRecord {
    RawSampleRecord {
        t_sec,
        face_tracked: true,
        face_transform: Some(translation(0.004, -0.11, -0.45)),
        look_at: Some(vec3(0.002, 0.04, 0.25)),
        left_eye_transform: Some(translation(0.031, 0.025, 0.02)),
        right_eye_transform: Some(translation(-0.031, 0.025, 0.02)),
        eye_blink_left: Some(f64::from(0.05_f32)),
        eye_blink_right: Some(f64::from(0.07_f32)),
        ambient_intensity: Some(1000.0),
        ambient_color_temp: Some(5500.0),
        light_direction: None,
        orientation: GazeOrientation::Portrait,
        clip_media_time_sec: None,
        stimulus_id: None,
    }
}

fn untracked(t_sec: f64) -> RawSampleRecord {
    RawSampleRecord {
        t_sec,
        face_tracked: false,
        // ARKit-free garbage the encoder must not write for an untracked frame.
        face_transform: Some(translation(0.0, 0.0, 0.0)),
        look_at: Some(vec3(0.0, 0.0, 0.0)),
        left_eye_transform: None,
        right_eye_transform: None,
        eye_blink_left: Some(0.0),
        eye_blink_right: Some(0.0),
        ambient_intensity: Some(1000.0),
        ambient_color_temp: Some(5500.0),
        light_direction: None,
        orientation: GazeOrientation::Portrait,
        clip_media_time_sec: None,
        stimulus_id: None,
    }
}

fn json(line: &str) -> Value {
    serde_json::from_str(line).expect("encoder output is JSON")
}

#[test]
fn tracked_sample_line_equals_the_fixture_line_exactly() {
    let line = gaze_raw_log_line(tracked(100.0)).unwrap();
    assert_eq!(json(&line), fixture_lines()[1]);
}

#[test]
fn untracked_sample_omits_face_fields_like_the_fixture() {
    let line = gaze_raw_log_line(untracked(100.05)).unwrap();
    let v = json(&line);
    assert_eq!(v, fixture_lines()[3]);
    assert!(
        v.get("face_transform").is_none(),
        "zeros would read as a frontal pose"
    );
    assert!(v.get("eye_blink_left").is_none());
}

#[test]
fn single_precision_vectors_are_written_at_their_shortest_decimal() {
    let mut r = tracked(1.0);
    r.light_direction = Some(vec3(0.0, -1.0, 0.1));
    r.orientation = GazeOrientation::LandscapeLeft;
    let v = json(&gaze_raw_log_line(r).unwrap());
    assert_eq!(v["light_direction"], serde_json::json!([0.0, -1.0, 0.1]));
    assert_eq!(v["look_at"], serde_json::json!([0.002, 0.04, 0.25]));
    assert_eq!(v["orientation"], "landscapeLeft");
    // Doubles from ARKit (CGFloat light estimate, the frame clock) are written as given.
    let mut r = tracked(100.016667);
    r.ambient_intensity = Some(1234.5678);
    let v = json(&gaze_raw_log_line(r).unwrap());
    assert_eq!(v["t_sec"], serde_json::json!(100.016667));
    assert_eq!(v["ambient_intensity"], serde_json::json!(1234.5678));
}

#[test]
fn absent_and_non_finite_values_are_omitted_never_defaulted() {
    let mut r = tracked(1.0);
    r.look_at = Some(GazeVec3 {
        x: f64::NAN,
        y: 0.0,
        z: 0.0,
    });
    r.ambient_intensity = Some(f64::INFINITY);
    r.ambient_color_temp = None;
    let mut m = translation(0.0, 0.0, -0.4);
    m[3] = f64::NAN;
    r.left_eye_transform = Some(m);
    let v = json(&gaze_raw_log_line(r).unwrap());
    assert!(v.get("look_at").is_none());
    assert!(v.get("ambient_intensity").is_none());
    assert!(v.get("ambient_color_temp").is_none());
    assert!(v.get("left_eye_transform").is_none());
    assert!(v.get("face_transform").is_some());
}

#[test]
fn non_finite_time_or_bad_matrix_is_an_invalid_argument() {
    assert!(matches!(
        gaze_raw_log_line(tracked(f64::NAN)),
        Err(AsdError::InvalidArgument { .. })
    ));
    let mut r = tracked(1.0);
    r.face_transform = Some(vec![1.0; 15]);
    assert!(matches!(
        gaze_raw_log_line(r),
        Err(AsdError::InvalidArgument { .. })
    ));
}

fn fixture_header_input() -> GazeRawLogHeaderInput {
    GazeRawLogHeaderInput {
        model_identifier: "iPhone17,3".into(),
        orientation: GazeOrientation::Portrait,
        targets: vec![GazeScheduledTarget {
            kind: GazeTargetKind::Calibration,
            x_norm: 0.5,
            y_norm: 0.5,
            start_sec: 100.0,
            end_sec: 101.6,
            label: Some("cal-1".into()),
        }],
        app_version: Some("1.0 (1)".into()),
        condition: Some(GazeBenchCondition::Baseline),
    }
}

#[test]
fn header_line_equals_the_fixture_header_with_the_rust_geometry() {
    let line = gaze_raw_log_header_line(fixture_header_input()).unwrap();
    assert_eq!(json(&line), fixture_lines()[0]);
}

#[test]
fn header_condition_is_the_ben4_name_and_optional_fields_are_omitted() {
    let mut h = fixture_header_input();
    h.condition = Some(GazeBenchCondition::LowLight);
    assert_eq!(
        json(&gaze_raw_log_header_line(h).unwrap())["condition"],
        "lowLight"
    );
    let mut h = fixture_header_input();
    h.condition = None;
    h.app_version = None;
    h.targets.clear();
    let v = json(&gaze_raw_log_header_line(h).unwrap());
    assert!(v.get("condition").is_none());
    assert!(v.get("appVersion").is_none());
    assert_eq!(v["targets"], serde_json::json!([]));
    // An unknown model gets the idiom default, as the pipeline would.
    let mut h = fixture_header_input();
    h.model_identifier = "iPad99,9".into();
    assert_eq!(
        json(&gaze_raw_log_header_line(h).unwrap())["geometrySource"],
        "idiomDefault"
    );
}

#[test]
fn bench_conditions_are_the_ben4_matrix() {
    let names: Vec<String> = gaze_bench_conditions()
        .into_iter()
        .map(gaze_bench_condition_name)
        .collect();
    assert_eq!(
        names,
        [
            "baseline",
            "distance",
            "head",
            "motion",
            "glasses",
            "lowLight",
            "sideLight",
            "occlusion",
            "blinks"
        ]
    );
}

#[test]
fn document_is_a_valid_raw_log_and_drops_repeated_or_invalid_times() {
    let records = vec![
        tracked(100.0),
        tracked(100.016667),
        tracked(100.016667), // repeated frame time: REP-1 needs strictly increasing
        tracked(f64::NAN),   // unusable time
        tracked(99.0),       // older
        untracked(100.05),
    ];
    let doc = gaze_raw_log_document(fixture_header_input(), records).unwrap();
    assert_eq!(doc.sample_count, 3);
    assert_eq!(doc.dropped_sample_count, 3);
    assert!(doc.jsonl.ends_with('\n'));
    let parsed = raw_log::parse(&doc.jsonl).expect("replay --check accepts it");
    assert_eq!(parsed.samples.len(), 3);
    let lines: Vec<Value> = doc.jsonl.lines().map(json).collect();
    assert_eq!(lines, fixture_lines(), "the whole fixture, line for line");
}

#[test]
fn target_log_is_the_header_schedule() {
    let targets = fixture_header_input().targets;
    let v = json(&gaze_scheduled_targets_json(targets).unwrap());
    assert_eq!(v, fixture_lines()[0]["targets"]);
}
