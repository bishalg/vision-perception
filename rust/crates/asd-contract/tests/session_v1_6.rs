//! owner: WP-R.3
//!
//! CON-2, CON-3, CON-4, CON-12, CON-13, CON-14: the session ingest contract decodes and
//! re-encodes the iOS golden payload without loss (JCS-equal), rejects payloads without a
//! required field, keeps the two explicit-null fields and the absent-key optionals apart,
//! and the committed `session-v1.6.json` is the fresh `schemars` export and accepts the
//! iOS fixture.

use std::path::PathBuf;

use asd_contract::schema;
use asd_contract::session_v1_6::{
    DwellUnits, GazeSessionRecord, SessionIngestRequest, SessionPerVideoRow, SCHEMA_VERSION,
};
use asd_core::types::{AttentionFlag, CaptureSource, ExpectationStatus, GazeQualityVerdict};
use serde_json::{json, Value};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn ios_fixture() -> Value {
    let path = repo_root().join("apps/ios_asd_gaze_app/Tests/Fixtures/expected-session-v1.6.json");
    serde_json::from_str(&std::fs::read_to_string(path).expect("iOS fixture")).expect("json")
}

fn committed_session_schema() -> Value {
    let path = repo_root().join("docs/spec/schemas/asd/session-v1.6.json");
    serde_json::from_str(&std::fs::read_to_string(path).expect("schema file")).expect("json")
}

fn jcs<T: serde::Serialize>(v: &T) -> Vec<u8> {
    asd_contract::signing::canonical_bytes(v).expect("jcs")
}

fn decode(v: &Value) -> Result<SessionIngestRequest, serde_json::Error> {
    serde_json::from_value(v.clone())
}

// ---- CON-2 / CON-12: lossless round trip of the iOS payload ----------------------------

#[test]
fn ios_golden_payload_roundtrips_jcs_equal() {
    let original = ios_fixture();
    let typed = decode(&original).expect("decode iOS payload");
    assert_eq!(
        jcs(&typed),
        jcs(&original),
        "JCS bytes differ after round trip"
    );

    assert_eq!(typed.session_id, "SESS-GOLDEN-0001");
    assert_eq!(typed.capture_source, CaptureSource::TrueDepth);
    assert_eq!(typed.quality_verdict, GazeQualityVerdict::Optimal);
    assert_eq!(typed.schema_version, SCHEMA_VERSION);
    assert_eq!(typed.attention_flag, Some(Some(AttentionFlag::Yellow)));
    assert_eq!(typed.battery, Some(None), "explicit null battery");
    assert_eq!(typed.raw_payload.schema_version, "v1.6");
    assert_eq!(typed.raw_payload.per_video.as_ref().map(Vec::len), Some(2));
}

#[test]
fn per_video_rows_are_snake_case_and_details_rows_camel_case() {
    let typed = decode(&ios_fixture()).unwrap();
    let rows = typed.per_video.as_ref().expect("perVideo");
    let first: &SessionPerVideoRow = &rows[0];
    assert_eq!(first.clip_id, "stim_05_geopref_split_10s");
    assert_eq!(first.order, 1);
    assert_eq!(first.dwell_social, Some(0.62));
    assert_eq!(first.dwell_mouth, None);
    assert_eq!(first.fixation_count, Some(14));
    assert_eq!(first.status, Some(ExpectationStatus::Atypical));
    assert_eq!(first.details.dwell_units, DwellUnits::Ratio);
    assert_eq!(first.details.rows.len(), 6);
    assert_eq!(
        first.details.rows[3].target_aoi.as_deref(),
        Some("Gaze_Target_Right")
    );

    let encoded = serde_json::to_value(first).unwrap();
    for key in [
        "clip_id",
        "dwell_social",
        "fixation_count",
        "mean_fixation_ms",
    ] {
        assert!(encoded.get(key).is_some(), "snake_case key {key}");
    }
    assert!(encoded.get("dwell_mouth").is_none(), "unmeasured: absent");
    let detail_row = &encoded["details"]["rows"][0];
    assert!(detail_row.get("segmentId").is_some(), "camelCase score row");

    let second = &rows[1];
    assert_eq!(second.status, None, "no scored rows: no status");
    assert!(serde_json::to_value(second)
        .unwrap()
        .get("status")
        .is_none());
}

// ---- trap 3: explicit null vs absent key on the same struct -----------------------------

#[test]
fn attention_flag_and_battery_keep_explicit_null() {
    let mut v = ios_fixture();
    v["attentionFlag"] = Value::Null;
    v["battery"] = Value::Null;
    let typed = decode(&v).unwrap();
    assert_eq!(typed.attention_flag, Some(None));
    let out = serde_json::to_value(&typed).unwrap();
    assert_eq!(
        out.get("attentionFlag"),
        Some(&Value::Null),
        "null is written"
    );
    assert_eq!(out.get("battery"), Some(&Value::Null));

    let obj = v.as_object_mut().unwrap();
    obj.remove("attentionFlag");
    obj.insert("battery".into(), json!("express_30s"));
    let typed = decode(&v).unwrap();
    assert_eq!(typed.attention_flag, None, "absent stays absent");
    assert_eq!(typed.battery, Some(Some("express_30s".into())));
    let out = serde_json::to_value(&typed).unwrap();
    assert!(out.get("attentionFlag").is_none());
    assert_eq!(out["battery"], json!("express_30s"));
}

#[test]
fn unmeasured_metrics_are_absent_keys_never_null_or_default() {
    let mut v = ios_fixture();
    let obj = v.as_object_mut().unwrap();
    obj.remove("eyeDwellRatio");
    obj.remove("calibratedDistanceCm");
    let typed = decode(&v).unwrap();
    assert_eq!(typed.eye_dwell_ratio, None);
    let out = serde_json::to_value(&typed).unwrap();
    assert!(out.get("eyeDwellRatio").is_none());
    assert!(out.get("mouthDwellRatio").is_none());
    assert!(out.get("firstEyeLatencyMs").is_none());
    assert!(out.get("calibratedDistanceCm").is_none());
}

// ---- CON-14: no server defaults --------------------------------------------------------

#[test]
fn every_required_field_is_rejected_when_missing() {
    for key in [
        "sessionId",
        "participantToken",
        "captureSource",
        "durationSec",
        "validRatio",
        "socialPreferenceScore",
        "qualityVerdict",
        "schemaVersion",
        "rawPayload",
    ] {
        let mut v = ios_fixture();
        v.as_object_mut().unwrap().remove(key);
        assert!(decode(&v).is_err(), "payload without {key} must fail");
    }
}

#[test]
fn unknown_enum_values_are_rejected() {
    for (key, bad) in [
        ("captureSource", json!("webcam")),
        ("qualityVerdict", json!("great")),
        ("attentionFlag", json!("unreliable")),
    ] {
        let mut v = ios_fixture();
        v[key] = bad;
        assert!(decode(&v).is_err(), "{key} must reject an unknown value");
    }
    let mut v = ios_fixture();
    v["perVideo"][0]["status"] = json!("offTarget");
    assert!(
        decode(&v).is_err(),
        "status offTarget is not a contract value (CON-13)"
    );
    let mut v = ios_fixture();
    v["perVideo"][0]["details"]["dwellUnits"] = json!("seconds");
    assert!(decode(&v).is_err(), "dwell units are ratios (CON-13)");
}

#[test]
fn synthetic_capture_source_is_a_contract_value() {
    let mut v = ios_fixture();
    v["captureSource"] = json!("synthetic");
    v["rawPayload"]["captureSource"] = json!("synthetic");
    let typed = decode(&v).unwrap();
    assert_eq!(typed.capture_source, CaptureSource::Synthetic);
}

#[test]
fn unknown_keys_are_tolerated_and_raw_payload_extras_are_preserved() {
    let mut v = ios_fixture();
    v["someFutureField"] = json!(1);
    v["rawPayload"]["bundleVersion"] = json!("2026.10.1");
    v["rawPayload"]["normCollection"] = json!({"consented": true});
    let typed = decode(&v).expect("extra keys tolerated");
    let raw: &GazeSessionRecord = &typed.raw_payload;
    assert_eq!(raw.extra.get("bundleVersion"), Some(&json!("2026.10.1")));
    let out = serde_json::to_value(&typed).unwrap();
    assert_eq!(
        out["rawPayload"]["normCollection"]["consented"],
        json!(true)
    );
    assert!(
        out.get("someFutureField").is_none(),
        "top level is the typed contract"
    );
}

// ---- number and key formatting traps ---------------------------------------------------

#[test]
fn floats_serialise_with_a_decimal_point_and_canonicalise_like_swift() {
    let typed = decode(&ios_fixture()).unwrap();
    let text = serde_json::to_string(&typed).unwrap();
    // serde_json writes an f64 whole number as `38.0`; Swift's JSONEncoder writes `38`.
    assert!(text.contains("\"calibratedDistanceCm\":38.0"), "{text}");
    // JCS (RFC 8785) collapses both to `38`, so comparisons go through JCS (trap 2).
    let canonical = String::from_utf8(jcs(&typed)).unwrap();
    assert!(
        canonical.contains("\"calibratedDistanceCm\":38,"),
        "{canonical}"
    );
    // Integer-typed fields stay integers.
    assert!(text.contains("\"fixation_count\":14,"), "{text}");
    assert!(text.contains("\"order\":1,"), "{text}");
}

#[test]
fn serde_json_maps_keep_sorted_keys() {
    // Fails if any crate in the build turns on serde_json/preserve_order.
    let v: Value = serde_json::from_str(r#"{"b":1,"a":2,"c":{"z":0,"y":1}}"#).unwrap();
    assert_eq!(
        serde_json::to_string(&v).unwrap(),
        r#"{"a":2,"b":1,"c":{"y":1,"z":0}}"#
    );
}

// ---- CON-3 / CON-4: schema export and validation ---------------------------------------

#[test]
fn committed_schemas_match_a_fresh_export() {
    let dir = repo_root().join("docs/spec/schemas/asd");
    if let Err(drift) = schema::check(&dir) {
        panic!("schema drift, run `asd-cli schema export`: {drift:?}");
    }
}

#[test]
fn ios_fixture_validates_against_the_session_schema() {
    let schema = schema::session_v1_6();
    assert_eq!(schema, committed_session_schema());
    schema::validate(&schema, &ios_fixture()).expect("iOS payload validates");
}

#[test]
fn schema_rejects_a_payload_without_session_id_or_with_wrong_types() {
    let schema = schema::session_v1_6();
    let mut v = ios_fixture();
    v.as_object_mut().unwrap().remove("sessionId");
    let errors = schema::validate(&schema, &v).unwrap_err();
    assert!(errors.iter().any(|e| e.contains("sessionId")), "{errors:?}");

    let mut v = ios_fixture();
    v["durationSec"] = json!("thirty");
    assert!(schema::validate(&schema, &v).is_err());
}

#[test]
fn schema_enforces_ratio_units_and_contract_enums() {
    let schema = schema::session_v1_6();
    let cases = [
        ("/perVideo/0/dwell_social", json!(6.5)),
        ("/validRatio", json!(1.5)),
        ("/perVideo/0/status", json!("offTarget")),
        ("/captureSource", json!("webcam")),
        ("/attentionFlag", json!("unreliable")),
    ];
    for (pointer, bad) in cases {
        let mut v = ios_fixture();
        *v.pointer_mut(pointer).unwrap() = bad;
        assert!(
            schema::validate(&schema, &v).is_err(),
            "{pointer} must be rejected by the schema"
        );
    }
}

#[test]
fn schema_keeps_the_shape_the_swift_validator_reads() {
    // `SessionIngestPayloadTests.SessionSchemaFile` understands `type` (a string), `anyOf`,
    // `$ref` into `$defs`, `items`, `required`, `properties`, and looks up
    // `$defs.SessionPerVideoScore` by name.
    let schema = schema::session_v1_6();
    assert_eq!(
        schema["$id"],
        json!("https://schemas.curv.org/asd/session-v1.6.json")
    );
    assert_eq!(schema["title"], json!("SessionIngestRequest"));
    assert_eq!(schema["type"], json!("object"));
    assert_eq!(schema["properties"]["durationSec"]["type"], json!("number"));
    assert!(schema["$defs"]["SessionPerVideoScore"]["properties"]["clip_id"].is_object());

    let mut required: Vec<&str> = schema["required"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    required.sort_unstable();
    assert_eq!(
        required,
        [
            "captureSource",
            "durationSec",
            "participantToken",
            "qualityVerdict",
            "rawPayload",
            "schemaVersion",
            "sessionId",
            "socialPreferenceScore",
            "validRatio",
        ]
    );

    // Nullable fields are `anyOf [.., {type: null}]` like the Pydantic file, never a
    // `type` array; and no field carries a server-side default (CON-14).
    fn walk(v: &Value, path: &str) {
        match v {
            Value::Object(m) => {
                if let Some(t) = m.get("type") {
                    assert!(t.is_string(), "{path}: `type` must be a string, got {t}");
                }
                assert!(
                    !m.contains_key("default") || path.ends_with("/properties"),
                    "{path}: no defaults in the contract"
                );
                for (k, child) in m {
                    walk(child, &format!("{path}/{k}"));
                }
            }
            Value::Array(a) => a
                .iter()
                .enumerate()
                .for_each(|(i, x)| walk(x, &format!("{path}/{i}"))),
            _ => {}
        }
    }
    walk(&schema, "#");

    assert_eq!(
        schema["properties"]["attentionFlag"]["anyOf"][1],
        json!({"type": "null"})
    );
    assert_eq!(
        schema["properties"]["battery"]["anyOf"],
        json!([{"type": "string"}, {"type": "null"}])
    );
}

#[test]
fn every_key_the_ios_payload_sends_is_declared() {
    // Port of `testPayloadSendsOnlyKeysTheSchemaDeclares`.
    let schema = schema::session_v1_6();
    let payload = ios_fixture();
    let props = schema["properties"].as_object().unwrap();
    for key in payload.as_object().unwrap().keys() {
        assert!(props.contains_key(key), "undeclared top-level key {key}");
    }
    let row_props = schema["$defs"]["SessionPerVideoScore"]["properties"]
        .as_object()
        .unwrap();
    for clip in payload["perVideo"].as_array().unwrap() {
        for key in clip.as_object().unwrap().keys() {
            assert!(
                row_props.contains_key(key),
                "undeclared per-video key {key}"
            );
        }
    }
}
