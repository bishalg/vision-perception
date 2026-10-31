//! owner: WP-G.11
//!
//! SCH-4: the `/v1` tracking-quality view is the QS-1 record with snake_case keys, values
//! unchanged, unmeasured values `null`; it validates against its `api-v1.json` definition;
//! the list item and the review reason carry the new members.

use asd_contract::api_v1::{ReviewReason, SessionDetail, SessionSummary};
use asd_contract::schema;
use asd_contract::session_v1_7::SessionIngestRequest;
use asd_contract::tracking_api::SessionTrackingQuality;
use serde_json::{json, Value};

fn golden_1_7() -> SessionIngestRequest {
    serde_json::from_str(include_str!(
        "../../../../apps/ios_asd_gaze_app/Tests/Fixtures/expected-session-v1.7.json"
    ))
    .unwrap()
}

fn snake(key: &str) -> String {
    let mut out = String::new();
    for c in key.chars() {
        if c.is_ascii_uppercase() {
            out.push('_');
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// The record with every object key snake_cased and absent optionals as `null` is what the
/// view serialises to, except that absent optionals are explicit `null`s in the view.
fn snake_keys(v: &Value) -> Value {
    match v {
        Value::Object(m) => {
            Value::Object(m.iter().map(|(k, v)| (snake(k), snake_keys(v))).collect())
        }
        Value::Array(a) => Value::Array(a.iter().map(snake_keys).collect()),
        other => other.clone(),
    }
}

fn drop_nulls(v: &Value) -> Value {
    match v {
        Value::Object(m) => Value::Object(
            m.iter()
                .filter(|(_, v)| !v.is_null())
                .map(|(k, v)| (k.clone(), drop_nulls(v)))
                .collect(),
        ),
        Value::Array(a) => Value::Array(a.iter().map(drop_nulls).collect()),
        other => other.clone(),
    }
}

#[test]
fn view_is_the_record_with_snake_case_keys_and_unchanged_values() {
    let tq = golden_1_7().raw_payload.tracking_quality.unwrap();
    let view = SessionTrackingQuality::from(&tq);
    let out = serde_json::to_value(&view).unwrap();
    let record = serde_json::to_value(&tq).unwrap();
    assert_eq!(drop_nulls(&out), snake_keys(&record));
    // Enum values keep the record spelling; unmeasured values are explicit nulls.
    assert_eq!(out["calibration"]["model"], "screenAffine");
    assert_eq!(out["device"]["orientation"], "landscapeLeft");
    assert_eq!(out["device"]["model_identifier"], "iPad13,18");
    assert_eq!(out["verdict"], "good");
    assert_eq!(out["estimated_accuracy_deg"], 1.63);
    assert_eq!(out["drift_checks"][1]["offset_deg"], Value::Null);
    assert_eq!(out["validation"]["targets"][3]["accuracy_deg"], Value::Null);
    // Round trip.
    let back: SessionTrackingQuality = serde_json::from_value(out).unwrap();
    assert_eq!(back, view);
}

#[test]
fn view_validates_against_api_v1_json() {
    let api = schema::api_v1();
    let tq = golden_1_7().raw_payload.tracking_quality.unwrap();
    let out = serde_json::to_value(SessionTrackingQuality::from(&tq)).unwrap();
    let mut def = api.clone();
    def["$ref"] = json!("#/$defs/SessionTrackingQuality");
    schema::validate(&def, &out).expect("view validates");
    // The record itself (camelCase) does not: the API is snake_case.
    let record = serde_json::to_value(&tq).unwrap();
    assert!(schema::validate(&def, &record).is_err());
}

#[test]
fn session_dtos_carry_the_tracking_members_as_nullable() {
    let api = schema::api_v1();
    let defs = &api["$defs"];
    for (ty, field) in [
        ("SessionSummary", "tracking_verdict"),
        ("SessionSummary", "tracking_accuracy_deg"),
        ("SessionDetail", "tracking_quality"),
    ] {
        assert!(
            defs[ty]["properties"].get(field).is_some(),
            "{ty}.{field} missing"
        );
        let required = defs[ty]["required"].as_array().cloned().unwrap_or_default();
        assert!(
            !required.contains(&json!(field)),
            "{ty}.{field} must be optional"
        );
    }
    assert_eq!(
        defs["TrackingVerdict"]["enum"],
        json!(["good", "fair", "poor", "unusable"])
    );
    // Published examples: one 1.6 session (nulls) and one 1.7 session (values).
    let examples = defs["SessionDetail"]["examples"].as_array().unwrap();
    assert_eq!(examples.len(), 2);
    let decoded: Vec<SessionDetail> = examples
        .iter()
        .map(|e| serde_json::from_value(e.clone()).unwrap())
        .collect();
    assert!(decoded[0].tracking_quality.is_none());
    assert_eq!(decoded[0].summary.tracking_verdict, None);
    assert!(decoded[1].tracking_quality.is_some());
    let summary: SessionSummary =
        serde_json::from_value(defs["SessionSummary"]["examples"][1].clone()).unwrap();
    assert_eq!(summary.tracking_accuracy_deg, Some(1.63));
    // A 1.6 list item serialises the new members as null (not omitted, /v1 convention).
    let v = &defs["SessionSummary"]["examples"][0];
    assert_eq!(v["tracking_verdict"], Value::Null);
    assert_eq!(v["tracking_accuracy_deg"], Value::Null);
}

#[test]
fn review_reason_has_low_tracking_quality() {
    assert_eq!(
        serde_json::to_value(ReviewReason::LowTrackingQuality).unwrap(),
        "low_tracking_quality"
    );
    let api = schema::api_v1();
    assert!(api["$defs"]["ReviewReason"]["enum"]
        .as_array()
        .unwrap()
        .contains(&json!("low_tracking_quality")));
}

#[test]
fn api_v1_json_has_no_camel_case_tracking_record() {
    // The ingest mirrors (camelCase `TrackingQuality`) must not share a `$defs` name with the
    // /v1 view; only the view's types are in api-v1.json.
    let api = schema::api_v1();
    let defs = api["$defs"].as_object().unwrap();
    assert!(defs.contains_key("SessionTrackingQuality"));
    assert!(!defs.contains_key("TrackingQuality"));
}
