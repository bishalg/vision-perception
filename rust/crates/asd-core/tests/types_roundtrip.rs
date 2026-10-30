//! owner: WP-R.0
//!
//! CON-1: every domain type decodes the real Swift/bundled fixtures, re-encodes, and the
//! RFC 8785 (JCS) canonical bytes equal the canonical bytes of the original document.
//! Raw bytes are never compared across languages (Swift writes `1` or `1.0` for the same
//! `Double`; JCS collapses both).

use asd_core::types::{
    AttentionFlag, AttentionFlagAssessment, CaptureSource, ExpectationStatus, GazeFrame,
    GazeFrameSample, GazeQualityVerdict, GazeTelemetrySample, NormativeBaselineSet,
    NormativeDirection, PerVideoMetricSet, PerVideoScoreReport, PerVideoScoreRow,
    PerVideoSessionScore, StimulusCatalog,
};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value};

const BASELINES: &str = include_str!("fixtures/normative_baselines.json");
const CATALOG: &str = include_str!("fixtures/stimulus_catalog.json");
const SESSION: &str = include_str!("fixtures/expected-session-v1.6.json");
const BUNDLE: &str = include_str!("fixtures/expected-bundle.json");

fn jcs<T: Serialize>(v: &T) -> Vec<u8> {
    serde_jcs::to_vec(v).expect("jcs")
}

/// Swift's synthesised `Codable` omits `nil` optionals (`encodeIfPresent`), so a hand-authored
/// file with explicit `null` members round-trips without them. Drop nulls from the original
/// before comparing; used only for the two hand-authored config files.
fn strip_nulls(v: &mut Value) {
    match v {
        Value::Object(m) => {
            m.retain(|_, x| !x.is_null());
            m.values_mut().for_each(strip_nulls);
        }
        Value::Array(a) => a.iter_mut().for_each(strip_nulls),
        _ => {}
    }
}

fn roundtrip<T: Serialize + DeserializeOwned>(original: &Value) -> T {
    let typed: T = serde_json::from_value(original.clone()).expect("decode");
    assert_eq!(
        jcs(&typed),
        jcs(original),
        "JCS bytes differ after round trip"
    );
    typed
}

#[test]
fn normative_baselines_roundtrip() {
    let mut original: Value = serde_json::from_str(BASELINES).unwrap();
    strip_nulls(&mut original);
    let set: NormativeBaselineSet = roundtrip(&original);
    assert_eq!(set.status, "provisional-literature-seeded");
    assert_eq!(set.age_brackets.len(), 5);
    assert_eq!(set.videos.len(), 10);
    let video = &set.videos["stim_05_geopref_split_10s"];
    assert!(video.weight > 0.0);
    let exp = &video.segments[0].expectations[0];
    assert!(matches!(
        exp.direction,
        NormativeDirection::HigherBetter
            | NormativeDirection::LowerBetter
            | NormativeDirection::Band
    ));
}

#[test]
fn stimulus_catalog_roundtrip() {
    let original: Value = serde_json::from_str(CATALOG).unwrap();
    let catalog: StimulusCatalog = roundtrip(&original);
    assert_eq!(catalog.schema_version, "1.0.0");
    assert_eq!(catalog.fallback_language, "en");
    assert_eq!(catalog.age_packs.len(), 3);
    assert!(catalog
        .clips
        .iter()
        .any(|c| c.id == "stim_01_social_peekaboo_8s"));
    assert!(catalog
        .clips
        .iter()
        .flat_map(|c| &c.areas)
        .any(|a| !a.roles.is_empty()));
}

#[test]
fn session_fixture_per_video_reports_roundtrip() {
    let original: Value = serde_json::from_str(SESSION).unwrap();
    // rawPayload.perVideo[] are camelCase PerVideoScoreReport values.
    let raw = original["rawPayload"]["perVideo"].clone();
    let reports: Vec<PerVideoScoreReport> = roundtrip(&raw);
    assert_eq!(reports.len(), 2);
    assert_eq!(reports[0].stimulus_id, "stim_05_geopref_split_10s");
    assert_eq!(reports[0].rows.len(), 6);
    let latency = &reports[0].rows[3];
    assert!(latency.imputed);
    assert_eq!(latency.target_aoi.as_deref(), Some("Gaze_Target_Right"));
    assert_eq!(latency.status, ExpectationStatus::Atypical);
    assert_eq!(reports[1].stimulus_id, "untagged");
    assert!(reports[1].rows.is_empty());
}

#[test]
fn session_fixture_row_variants_roundtrip() {
    // `perVideo[].details.rows[]` reuse PerVideoScoreRow; the rows of the snake_case
    // perVideo entries are a different struct from the report (trap 4).
    let original: Value = serde_json::from_str(SESSION).unwrap();
    let rows = original["perVideo"][0]["details"]["rows"].clone();
    let typed: Vec<PerVideoScoreRow> = roundtrip(&rows);
    assert_eq!(typed.len(), 6);
    // Rows with absent optional bounds must stay absent (encodeIfPresent).
    let first = serde_json::to_value(&typed[0]).unwrap();
    assert!(first.get("acceptableHigh").is_none());
    assert!(first.get("targetAOI").is_none());
}

#[test]
fn bundle_fixture_is_jcs_stable_json() {
    // The FHIR bundle has no typed core struct (FHIR is WP-B.8); prove only that the fixture
    // is valid JSON whose canonical form is stable under a parse/serialise cycle.
    let original: Value = serde_json::from_str(BUNDLE).unwrap();
    assert_eq!(original["resourceType"], "Bundle");
    let again: Value = serde_json::from_slice(&jcs(&original)).unwrap();
    assert_eq!(jcs(&again), jcs(&original));
}

#[test]
fn unknown_fields_are_tolerated_everywhere() {
    // Swift ignores unknown keys; `deny_unknown_fields` must NOT be set on any type.
    let mut b: Value = serde_json::from_str(BASELINES).unwrap();
    strip_nulls(&mut b);
    b["futureTop"] = json!(1);
    b["ageBrackets"][0]["futureKey"] = json!("x");
    b["videos"]["stim_05_geopref_split_10s"]["futureKey"] = json!([1, 2]);
    b["videos"]["stim_05_geopref_split_10s"]["segments"][0]["futureKey"] = json!(null);
    b["videos"]["stim_05_geopref_split_10s"]["segments"][0]["expectations"][0]["futureKey"] =
        json!({});
    assert!(serde_json::from_value::<NormativeBaselineSet>(b).is_ok());

    let mut c: Value = serde_json::from_str(CATALOG).unwrap();
    c["futureTop"] = json!(true);
    c["clips"][0]["futureKey"] = json!(1);
    c["clips"][0]["areas"][0]["futureKey"] = json!(1);
    c["clips"][0]["protocols"][0]["futureKey"] = json!(1);
    c["agePacks"][0]["futureKey"] = json!(1);
    assert!(serde_json::from_value::<StimulusCatalog>(c).is_ok());

    let s: Value = serde_json::from_str(SESSION).unwrap();
    let mut report = s["rawPayload"]["perVideo"][0].clone();
    report["futureKey"] = json!(1);
    report["rows"][0]["futureKey"] = json!(1);
    assert!(serde_json::from_value::<PerVideoScoreReport>(report).is_ok());

    let mut m = json!({
        "stimulusId": "a", "clipFrameCount": 2, "validFrameCount": 1, "validSampleRatio": 0.5,
        "observedDurationSec": 1.0, "aoiDwellRatios": {}, "metrics": {}, "futureKey": 1
    });
    assert!(serde_json::from_value::<PerVideoMetricSet>(m.clone()).is_ok());
    m["metrics"] = json!({"fixation_count": 3});
    assert!(serde_json::from_value::<PerVideoMetricSet>(m).is_ok());

    let a = json!({"flag": "green", "headline": "h", "detail": "d", "isScreeningSignalOnly": true, "x": 1});
    assert!(serde_json::from_value::<AttentionFlagAssessment>(a).is_ok());

    let score = json!({
        "ageBracketId": "24-35", "ageMonths": 30, "compositeScore": 0.5, "videos": [],
        "baselineStatus": "s", "baselineSchemaVersion": "1", "scoredAtUtc": "2026-10-30T00:00:00Z",
        "x": 1
    });
    assert!(serde_json::from_value::<PerVideoSessionScore>(score).is_ok());
}

#[test]
fn enums_use_swift_raw_values() {
    let to = |v: &dyn erased::Ser| v.to_value();
    assert_eq!(to(&NormativeDirection::HigherBetter), json!("higherBetter"));
    assert_eq!(to(&NormativeDirection::LowerBetter), json!("lowerBetter"));
    assert_eq!(to(&NormativeDirection::Band), json!("band"));
    assert_eq!(to(&ExpectationStatus::OnTarget), json!("onTarget"));
    assert_eq!(to(&ExpectationStatus::Monitor), json!("monitor"));
    assert_eq!(to(&ExpectationStatus::Atypical), json!("atypical"));
    assert_eq!(to(&AttentionFlag::Green), json!("green"));
    assert_eq!(to(&AttentionFlag::Yellow), json!("yellow"));
    assert_eq!(to(&AttentionFlag::Red), json!("red"));
    assert_eq!(to(&GazeQualityVerdict::Optimal), json!("optimal"));
    assert_eq!(to(&GazeQualityVerdict::Acceptable), json!("acceptable"));
    assert_eq!(to(&GazeQualityVerdict::Degraded), json!("degraded"));
    assert_eq!(to(&GazeQualityVerdict::Rejected), json!("rejected"));
    assert_eq!(to(&CaptureSource::TrueDepth), json!("truedepth"));
    assert_eq!(to(&CaptureSource::Simulated), json!("simulated"));
    assert_eq!(to(&CaptureSource::Synthetic), json!("synthetic"));
    assert!(serde_json::from_value::<AttentionFlag>(json!("unreliable")).is_err());
}

mod erased {
    pub trait Ser {
        fn to_value(&self) -> serde_json::Value;
    }
    impl<T: serde::Serialize> Ser for T {
        fn to_value(&self) -> serde_json::Value {
            serde_json::to_value(self).unwrap()
        }
    }
}

#[test]
fn gaze_frame_reads_only_extractor_fields_and_tolerates_the_rest() {
    // A frame as Swift encodes it carries many more keys (blend shapes, aoiDwellMs, ...).
    // The gaze sample uses `GazeTelemetrySample.CodingKeys`: snake_case and `valid`.
    let frame = json!({
        "gaze": {
            "timestamp_ms": 1000.0, "video_time_sec": 1.5, "x_norm": 0.4, "y_norm": 0.6,
            "confidence": 0.9, "left_eye_openness": 1.0, "valid": true,
            "stimulus_id": "stim_05_geopref_split_10s", "clip_media_time_sec": 1.25
        },
        "affect": {
            "mouthSmileLeft": 0.1, "socialSmileIntensity": 0.4, "expressivityIndex": 0.01,
            "surpriseScore": 0.2
        },
        "activeAOINames": ["Eye_Region"],
        "aoiDwellMs": 16.0,
        "qualityVerdict": "optimal",
        "stimulusId": "stim_05_geopref_split_10s"
    });
    let f: GazeFrame = serde_json::from_value(frame).unwrap();
    assert!(f.gaze.is_valid);
    assert_eq!(f.gaze.clip_media_time_sec, Some(1.25));
    assert_eq!(f.stimulus_id.as_deref(), Some("stim_05_geopref_split_10s"));
    assert_eq!(f.active_aoi_names, vec!["Eye_Region".to_string()]);
    assert_eq!(f.quality_verdict, GazeQualityVerdict::Optimal);
    assert_eq!(f.affect.social_smile_intensity, Some(0.4));

    // Untagged frame: no stimulusId, no clip clock, no affect values.
    let bare = json!({
        "gaze": {"valid": false, "x_norm": 0.0, "y_norm": 0.0, "video_time_sec": 0.0},
        "activeAOINames": [],
        "qualityVerdict": "rejected"
    });
    let f: GazeFrame = serde_json::from_value(bare).unwrap();
    assert!(f.stimulus_id.is_none() && f.gaze.clip_media_time_sec.is_none());
    assert!(f.affect.social_smile_intensity.is_none());
}

#[test]
fn telemetry_sample_roundtrip_omits_nil_optionals() {
    let sample = json!({
        "timestamp_ms": 1000.0, "video_time_sec": 1.0, "x_norm": 0.5, "y_norm": 0.5,
        "confidence": 0.8, "left_eye_openness": 1.0, "right_eye_openness": 1.0,
        "head_pitch_rad": 0.0, "head_yaw_rad": 0.1, "head_roll_rad": 0.0,
        "distance_to_screen_cm": 45.0, "valid": true
    });
    let typed: GazeTelemetrySample = roundtrip(&sample);
    assert!(typed.stimulus_id.is_none() && typed.clip_media_time_sec.is_none());
}

/// CON-1: the camelCase property names decode to the same sample as the Swift wire keys,
/// and encoding always writes the wire keys.
#[test]
fn gaze_samples_accept_both_spellings_and_encode_swift_keys() {
    let wire = json!({
        "timestamp_ms": 1000.0, "video_time_sec": 1.0, "x_norm": 0.5, "y_norm": 0.25,
        "confidence": 0.8, "left_eye_openness": 1.0, "right_eye_openness": 0.9,
        "head_pitch_rad": 0.0, "head_yaw_rad": 0.1, "head_roll_rad": 0.2,
        "distance_to_screen_cm": 45.0, "valid": true,
        "stimulus_id": "stim_05_geopref_split_10s", "clip_media_time_sec": 0.5
    });
    let camel = json!({
        "timestampMs": 1000.0, "videoTimeSec": 1.0, "xNorm": 0.5, "yNorm": 0.25,
        "confidence": 0.8, "leftEyeOpenness": 1.0, "rightEyeOpenness": 0.9,
        "headPitchRad": 0.0, "headYawRad": 0.1, "headRollRad": 0.2,
        "distanceToScreenCm": 45.0, "isValid": true,
        "stimulusId": "stim_05_geopref_split_10s", "clipMediaTimeSec": 0.5
    });
    let a: GazeTelemetrySample = serde_json::from_value(wire.clone()).unwrap();
    let b: GazeTelemetrySample = serde_json::from_value(camel).unwrap();
    assert_eq!(a, b);
    assert_eq!(serde_json::to_value(&b).unwrap(), wire);

    let frame_wire = json!({
        "valid": false, "x_norm": 0.1, "y_norm": 0.2, "video_time_sec": 3.0,
        "clip_media_time_sec": 1.0
    });
    let frame_camel = json!({
        "isValid": false, "xNorm": 0.1, "yNorm": 0.2, "videoTimeSec": 3.0,
        "clipMediaTimeSec": 1.0
    });
    let a: GazeFrameSample = serde_json::from_value(frame_wire.clone()).unwrap();
    let b: GazeFrameSample = serde_json::from_value(frame_camel).unwrap();
    assert_eq!(a, b);
    assert_eq!(serde_json::to_value(&b).unwrap(), frame_wire);
}

#[test]
fn whole_number_doubles_canonicalise_identically() {
    // Trap 2: Swift may write `1` where Rust writes `1.0`.
    let swift_style = json!({"expected": 1, "observed": 0, "score": 1, "weight": 0});
    let rust_style = json!({"expected": 1.0, "observed": 0.0, "score": 1.0, "weight": 0.0});
    assert_eq!(jcs(&swift_style), jcs(&rust_style));
    let row = json!({
        "segmentId": "s", "segmentDescription": "d", "biomarker": "b", "metricKey": "b",
        "direction": "band", "observed": 10, "expected": 12, "acceptableLow": 6,
        "acceptableHigh": 20, "status": "onTarget", "score": 1, "imputed": false,
        "citation": "c", "source": "literature"
    });
    let typed: PerVideoScoreRow = roundtrip(&row);
    assert_eq!(typed.observed, 10.0);
}
