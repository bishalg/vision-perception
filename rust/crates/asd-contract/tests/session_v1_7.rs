//! owner: WP-G.6
//!
//! SCH-1, QS-1, QS-3, QS-5 (spec `asd-gaze-robustness/technical.md`, ADR-0007 D10): session
//! schema 1.7 is 1.6 plus typed optional `rawPayload.trackingQuality`, optional per-clip
//! confidence fields, optional `rawPayload.calibratedDistanceCm` and the per-frame
//! `track_state` / `gap_filled` keys. Additive only: every 1.6 payload decodes as 1.7 and the
//! committed `session-v1.6.json` keeps validating the 1.6 fixture.

use std::path::PathBuf;

use asd_contract::schema;
use asd_contract::session_v1_6;
use asd_contract::session_v1_7::{
    self, calibrated_distance_cm, ClipTracking, FrameTracking, SessionIngestRequest,
    TrackingQuality, SCHEMA_ID, SCHEMA_VERSION,
};
use asd_core::types::tracking::{CalibrationQuality, TrackState, TrackingVerdict};
use asd_core::types::GazeFrame;
use serde_json::{json, Value};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn read_json(rel: &str) -> Value {
    let path = repo_root().join(rel);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).expect("json")
}

fn fixture_v1_7() -> Value {
    read_json("apps/ios_asd_gaze_app/Tests/Fixtures/expected-session-v1.7.json")
}

fn fixture_v1_6() -> Value {
    read_json("apps/ios_asd_gaze_app/Tests/Fixtures/expected-session-v1.6.json")
}

fn jcs<T: serde::Serialize>(v: &T) -> Vec<u8> {
    asd_contract::signing::canonical_bytes(v).expect("jcs")
}

fn decode(v: &Value) -> Result<SessionIngestRequest, serde_json::Error> {
    serde_json::from_value(v.clone())
}

fn strict(mut schema: Value) -> Value {
    schema::forbid_additional_properties(&mut schema);
    schema
}

// ---- the 1.7 fixture ----------------------------------------------------------------------

#[test]
fn v1_7_fixture_roundtrips_jcs_equal_with_typed_tracking_quality() {
    let original = fixture_v1_7();
    let typed = decode(&original).expect("decode 1.7 fixture");
    assert_eq!(
        jcs(&typed),
        jcs(&original),
        "JCS bytes differ after round trip"
    );

    assert_eq!(typed.schema_version, SCHEMA_VERSION);
    assert_eq!(typed.raw_payload.schema_version, "v1.7");
    let tq: &TrackingQuality = typed
        .raw_payload
        .tracking_quality
        .as_ref()
        .expect("trackingQuality is typed, not an extra key");
    assert!(
        !typed.raw_payload.extra.contains_key("trackingQuality"),
        "trackingQuality must not fall into the extras"
    );
    assert_eq!(tq.verdict, TrackingVerdict::Good);
    assert_eq!(tq.calibration.quality, CalibrationQuality::Good);
    assert_eq!(tq.pipeline_version, asd_core::gaze::PIPELINE_VERSION);
    assert_eq!(
        tq.params_version,
        asd_core::types::tracking::TRACKING_PARAMS_SCHEMA_VERSION
    );

    let rows = typed.per_video.as_ref().unwrap();
    assert_eq!(rows[0].details.tracking.mean_sample_confidence, Some(0.84));
    assert_eq!(rows[0].details.tracking.aoi_min_size_deg, Some(4.6));
    assert_eq!(rows[0].details.base.valid_sample_ratio, 0.95);
    assert_eq!(rows[1].details.tracking.mean_fixation_confidence, None);
    let raw_clips = typed.raw_payload.per_video.as_ref().unwrap();
    assert_eq!(raw_clips[0].tracking.nearest_check_offset_deg, Some(1.1));
    assert_eq!(raw_clips[0].report.stimulus_id, "stim_05_geopref_split_10s");
}

#[test]
fn v1_7_fixture_validates_against_the_1_7_schema_with_unknown_keys_forbidden() {
    // Strict validation guards the schema-only mirrors of the asd-core tracking types: a key
    // the real type writes but the mirror forgets fails here.
    let fixture = fixture_v1_7();
    schema::validate(&schema::session_v1_7(), &fixture).expect("1.7 fixture validates");
    schema::validate(&strict(schema::session_v1_7()), &fixture)
        .expect("every key of the 1.7 fixture is declared");
    schema::validate_session_v1_7(&fixture).expect("validate_session_v1_7");

    // Re-encoding the typed value must also validate strictly (mirror requires nothing the
    // real types omit).
    let typed = decode(&fixture).unwrap();
    let encoded = serde_json::to_value(&typed).unwrap();
    schema::validate(&strict(schema::session_v1_7()), &encoded).expect("re-encoded validates");
}

#[test]
fn committed_v1_7_schema_is_the_fresh_export() {
    let committed = read_json("docs/spec/schemas/asd/session-v1.7.json");
    assert_eq!(committed, schema::session_v1_7());
    assert_eq!(committed["$id"], json!(SCHEMA_ID));
    assert_eq!(SCHEMA_ID, "https://schemas.curv.org/asd/session-v1.7.json");
    assert_eq!(committed["title"], json!("SessionIngestRequest"));
    assert!(committed["$defs"]["SessionPerVideoScore"]["properties"]["clip_id"].is_object());
    assert!(schema::documents()
        .iter()
        .any(|d| d.file_name == "session-v1.7.json"));
    assert!(
        schema::documents()
            .iter()
            .any(|d| d.file_name == "session-v1.6.json"),
        "1.6 stays exported and checked"
    );
}

// ---- 1.6 compatibility --------------------------------------------------------------------

#[test]
fn v1_6_contract_is_unchanged_and_still_validates_its_fixture() {
    assert_eq!(session_v1_6::SCHEMA_VERSION, "1.6");
    let committed = read_json("docs/spec/schemas/asd/session-v1.6.json");
    assert_eq!(committed, schema::session_v1_6());
    schema::validate_session(&fixture_v1_6()).expect("1.6 fixture validates against 1.6");
    let v16: session_v1_6::SessionIngestRequest = serde_json::from_value(fixture_v1_6()).unwrap();
    assert_eq!(jcs(&v16), jcs(&fixture_v1_6()));
}

#[test]
fn v1_6_fixture_is_rejected_by_nothing_in_1_7_decoding() {
    let original = fixture_v1_6();
    let typed = decode(&original).expect("a 1.6 payload decodes with the 1.7 types");
    assert_eq!(jcs(&typed), jcs(&original), "and re-encodes without loss");
    assert!(typed.raw_payload.tracking_quality.is_none());
    assert_eq!(typed.raw_payload.calibrated_distance_cm, Some(38.0));
    let rows = typed.per_video.as_ref().unwrap();
    assert_eq!(rows[0].details.tracking, ClipTracking::default());
    // Schema-wise as well: the 1.7 schema adds no required key.
    schema::validate(&schema::session_v1_7(), &original).expect("1.6 shape validates as 1.7");
}

#[test]
fn v1_7_schema_adds_no_required_key_anywhere() {
    // Additive only: every `required` list in 1.7 equals the one 1.6 has for the same type.
    let v16 = schema::session_v1_6();
    let v17 = schema::session_v1_7();
    assert_eq!(v16["required"], v17["required"]);
    let req = |s: &Value, def: &str| s["$defs"][def]["required"].clone();
    assert_eq!(
        req(&v16, "SessionPerVideoScore"),
        req(&v17, "SessionPerVideoScore")
    );
    assert_eq!(
        req(&v16, "SessionPerVideoDetails"),
        req(&v17, "SessionPerVideoDetails")
    );
    let mut raw16: Vec<String> = serde_json::from_value(req(&v16, "GazeSessionRecord")).unwrap();
    raw16.retain(|k| k != "calibratedDistanceCm");
    let raw17: Vec<String> = serde_json::from_value(req(&v17, "GazeSessionRecord")).unwrap();
    assert_eq!(raw16, raw17, "only calibratedDistanceCm became optional");
}

// ---- optional pieces ----------------------------------------------------------------------

#[test]
fn tracking_quality_absent_is_valid() {
    let mut v = fixture_v1_7();
    v["rawPayload"]
        .as_object_mut()
        .unwrap()
        .remove("trackingQuality");
    let typed = decode(&v).expect("decodes without trackingQuality");
    assert!(typed.raw_payload.tracking_quality.is_none());
    let out = serde_json::to_value(&typed).unwrap();
    assert!(
        out["rawPayload"].get("trackingQuality").is_none(),
        "absent, never null"
    );
    schema::validate(&strict(schema::session_v1_7()), &v).expect("valid without trackingQuality");
}

#[test]
fn calibrated_distance_absent_in_raw_payload_is_valid() {
    let mut v = fixture_v1_7();
    v["rawPayload"]
        .as_object_mut()
        .unwrap()
        .remove("calibratedDistanceCm");
    v.as_object_mut().unwrap().remove("calibratedDistanceCm");
    let typed = decode(&v).expect("decodes without calibratedDistanceCm");
    assert_eq!(typed.raw_payload.calibrated_distance_cm, None);
    let out = serde_json::to_value(&typed).unwrap();
    assert!(out["rawPayload"].get("calibratedDistanceCm").is_none());
    schema::validate(&schema::session_v1_7(), &v).expect("1.7 accepts it");
    assert!(
        schema::validate(&schema::session_v1_6(), &v).is_err(),
        "1.6 still requires rawPayload.calibratedDistanceCm"
    );
}

#[test]
fn per_clip_fields_are_absent_keys_when_unmeasured() {
    let typed = decode(&fixture_v1_7()).unwrap();
    let out = serde_json::to_value(&typed).unwrap();
    let second = &out["perVideo"][1]["details"];
    assert_eq!(second["meanSampleConfidence"], json!(0.41));
    for key in [
        "meanFixationConfidence",
        "nearestCheckOffsetDeg",
        "aoiMinSizeDeg",
    ] {
        assert!(second.get(key).is_none(), "{key} absent, never null");
        assert!(out["rawPayload"]["perVideo"][1].get(key).is_none(), "{key}");
    }
}

#[test]
fn schema_rejects_bad_tracking_values() {
    let s = schema::session_v1_7();
    let cases = [
        ("/rawPayload/trackingQuality/verdict", json!("excellent")),
        (
            "/rawPayload/trackingQuality/calibration/quality",
            json!("great"),
        ),
        (
            "/rawPayload/trackingQuality/device/geometrySource",
            json!("guess"),
        ),
        (
            "/rawPayload/trackingQuality/device/orientation",
            json!("sideways"),
        ),
        (
            "/rawPayload/trackingQuality/recalibrations/0/reason",
            json!("bored"),
        ),
        ("/perVideo/0/details/meanSampleConfidence", json!(1.5)),
        ("/rawPayload/perVideo/0/meanSampleConfidence", json!(-0.1)),
        ("/rawPayload/trackingQuality/loss/blinkSec", json!("0.6")),
    ];
    for (pointer, bad) in cases {
        let mut v = fixture_v1_7();
        *v.pointer_mut(pointer)
            .unwrap_or_else(|| panic!("{pointer}")) = bad;
        assert!(
            schema::validate(&s, &v).is_err(),
            "{pointer} must be rejected"
        );
    }
    for pointer in [
        "/rawPayload/trackingQuality/verdict",
        "/rawPayload/trackingQuality/calibration",
        "/rawPayload/trackingQuality/loss",
    ] {
        let mut v = fixture_v1_7();
        let (parent, key) = pointer.rsplit_once('/').unwrap();
        v.pointer_mut(parent)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .remove(key);
        assert!(
            schema::validate(&s, &v).is_err(),
            "missing {pointer} must be rejected"
        );
        assert!(decode(&v).is_err(), "missing {pointer} must fail to decode");
    }
}

// ---- QS-5 --------------------------------------------------------------------------------

#[test]
fn calibrated_distance_is_the_calibration_pose_distance_when_a_calibration_ran() {
    let typed = decode(&fixture_v1_7()).unwrap();
    let tq = typed.raw_payload.tracking_quality.clone().unwrap();
    assert_eq!(calibrated_distance_cm(Some(&tq)), Some(38.0));
    assert_eq!(
        typed.raw_payload.calibrated_distance_cm,
        calibrated_distance_cm(Some(&tq)),
        "the fixture follows QS-5"
    );

    let mut skipped = tq.clone();
    skipped.calibration.quality = CalibrationQuality::Skipped;
    assert_eq!(calibrated_distance_cm(Some(&skipped)), None);
    let mut no_pose = tq;
    no_pose.calibration.pose = None;
    assert_eq!(calibrated_distance_cm(Some(&no_pose)), None);
    assert_eq!(calibrated_distance_cm(None), None);
}

// ---- per-frame keys (frames file) ---------------------------------------------------------

#[test]
fn frame_tracking_keys_are_snake_case_and_tolerated_by_the_1_6_frame_reader() {
    let sample = json!({
        "valid": false,
        "x_norm": 0.41,
        "y_norm": 0.52,
        "video_time_sec": 3.25,
        "track_state": "blink",
        "gap_filled": true
    });
    let keys: FrameTracking = serde_json::from_value(sample.clone()).unwrap();
    assert_eq!(keys.track_state, Some(TrackState::Blink));
    assert_eq!(keys.gap_filled, Some(true));
    let out = serde_json::to_value(keys).unwrap();
    assert_eq!(out, json!({"track_state": "blink", "gap_filled": true}));
    assert_eq!(
        serde_json::to_value(FrameTracking::default()).unwrap(),
        json!({}),
        "absent on a 1.6 frame, never null"
    );

    // A 1.6 reader (asd-core `GazeFrame`) ignores the new keys.
    let frame = json!({"gaze": sample, "qualityVerdict": "optimal"});
    let decoded: GazeFrame = serde_json::from_value(frame).expect("1.6 frame reader");
    assert!(!decoded.gaze.is_valid);

    for state in ["tracked", "blink", "lost", "offscreen", "artifact"] {
        let k: FrameTracking = serde_json::from_value(json!({ "track_state": state })).unwrap();
        assert!(k.track_state.is_some(), "{state}");
    }
    assert!(serde_json::from_value::<FrameTracking>(json!({"track_state": "Tracked"})).is_err());
}

#[test]
fn module_reexports_the_core_types_instead_of_copying_them() {
    // Single source: the contract's TrackingQuality *is* the asd-core type.
    fn same<T>(_: Option<T>, _: Option<T>) {}
    same::<asd_core::types::tracking::TrackingQuality>(None, None::<session_v1_7::TrackingQuality>);
    same::<asd_core::types::tracking::TrackState>(None, None::<session_v1_7::TrackState>);
}
