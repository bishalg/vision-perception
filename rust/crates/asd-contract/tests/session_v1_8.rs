//! owner: WP-G.12
//!
//! SCH-5 (spec `asd-gaze-robustness/technical.md`): session schema 1.8 is 1.7 plus a required
//! `scoringStatus` (`scored` | `unscorable`) and optional `unscorableReason`; the five score
//! fields are required numbers when `scored` and absent or `null` when `unscorable`. The rule
//! is enforced by the exported schema and by `scoring_violations` on the typed request.

use std::path::PathBuf;

use asd_contract::schema;
use asd_contract::session_v1_7;
use asd_contract::session_v1_8::{
    scoring_violations, upgrade_v1_7, ScoringStatus, SessionIngestRequest, UnscorableReason,
    RECORD_SCHEMA_VERSION, SCHEMA_ID, SCHEMA_VERSION, SCORE_FIELDS,
};
use serde_json::{json, Value};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn read_json(rel: &str) -> Value {
    let path = repo_root().join(rel);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).expect("json")
}

fn scored() -> Value {
    read_json("rust/crates/asd-contract/tests/fixtures/session-v1.8-scored.json")
}

fn unscorable() -> Value {
    read_json("rust/crates/asd-contract/tests/fixtures/session-v1.8-unscorable.json")
}

fn fixture_v1_7() -> Value {
    read_json("apps/ios_asd_gaze_app/Tests/Fixtures/expected-session-v1.7.json")
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

fn s18() -> Value {
    schema::session_v1_8()
}

fn pointers(req: &SessionIngestRequest) -> Vec<String> {
    scoring_violations(req)
        .into_iter()
        .map(|v| v.pointer)
        .collect()
}

// ---- golden examples ----------------------------------------------------------------------

#[test]
fn scored_golden_roundtrips_and_validates_strictly() {
    let original = scored();
    let typed = decode(&original).expect("decode scored golden");
    assert_eq!(
        jcs(&typed),
        jcs(&original),
        "JCS bytes differ after round trip"
    );
    assert_eq!(typed.schema_version, SCHEMA_VERSION);
    assert_eq!(typed.raw_payload.schema_version, RECORD_SCHEMA_VERSION);
    assert_eq!(typed.scoring_status, ScoringStatus::Scored);
    assert_eq!(typed.raw_payload.scoring_status, ScoringStatus::Scored);
    assert_eq!(typed.social_preference_score, Some(0.62));
    assert_eq!(typed.raw_payload.flat_affect_score, Some(0.12));
    assert!(
        typed.raw_payload.tracking_quality.is_some(),
        "1.7 parts carried"
    );
    assert!(!typed.raw_payload.extra.contains_key("scoringStatus"));
    assert!(scoring_violations(&typed).is_empty());

    schema::validate(&strict(s18()), &original).expect("every key declared");
    schema::validate_session_v1_8(&original).expect("validate_session_v1_8");
    let encoded = serde_json::to_value(&typed).unwrap();
    schema::validate(&strict(s18()), &encoded).expect("re-encoded validates");
}

#[test]
fn unscorable_golden_has_no_score_numbers_and_validates_strictly() {
    let original = unscorable();
    let typed = decode(&original).expect("decode unscorable golden");
    assert_eq!(jcs(&typed), jcs(&original));
    assert!(typed.is_unscorable());
    assert_eq!(
        typed.unscorable_reason,
        Some(UnscorableReason::AggregationFailed)
    );
    assert_eq!(
        typed.raw_payload.unscorable_reason,
        Some(UnscorableReason::AggregationFailed)
    );
    assert_eq!(typed.social_preference_score, None);
    let r = &typed.raw_payload;
    for v in [
        r.social_preference_score,
        r.social_smile_intensity,
        r.smile_reactivity_index,
        r.flat_affect_score,
        r.surprise_score_mean,
    ] {
        assert_eq!(v, None);
    }
    assert_eq!(typed.attention_flag, Some(None), "explicit null flag");
    assert!(scoring_violations(&typed).is_empty());
    schema::validate(&strict(s18()), &original).expect("unscorable validates");

    // Absent, never null, on re-encode.
    let out = serde_json::to_value(&typed).unwrap();
    assert!(out.get("socialPreferenceScore").is_none());
    for key in SCORE_FIELDS {
        assert!(out["rawPayload"].get(key).is_none(), "{key}");
    }
}

#[test]
fn unscorable_score_fields_may_be_explicit_null() {
    let mut v = unscorable();
    v["socialPreferenceScore"] = Value::Null;
    for key in SCORE_FIELDS {
        v["rawPayload"][key] = Value::Null;
    }
    schema::validate(&strict(s18()), &v).expect("null is allowed when unscorable");
    let typed = decode(&v).unwrap();
    assert!(scoring_violations(&typed).is_empty());
    assert_eq!(typed.raw_payload.flat_affect_score, None);
}

#[test]
fn unscorable_reason_is_optional() {
    let mut v = unscorable();
    v.as_object_mut().unwrap().remove("unscorableReason");
    v["rawPayload"]
        .as_object_mut()
        .unwrap()
        .remove("unscorableReason");
    schema::validate(&s18(), &v).expect("valid without a reason");
    assert!(scoring_violations(&decode(&v).unwrap()).is_empty());
}

#[test]
fn committed_v1_8_schema_is_the_fresh_export() {
    let committed = read_json("docs/spec/schemas/asd/session-v1.8.json");
    assert_eq!(committed, s18());
    assert_eq!(committed["$id"], json!(SCHEMA_ID));
    assert_eq!(SCHEMA_ID, "https://schemas.curv.org/asd/session-v1.8.json");
    assert_eq!(committed["title"], json!("SessionIngestRequest"));
    assert_eq!(
        committed["$defs"]["ScoringStatus"]["enum"],
        json!(["scored", "unscorable"])
    );
    let reasons: Vec<&str> = UnscorableReason::ALL.iter().map(|r| r.as_str()).collect();
    assert_eq!(
        committed["$defs"]["UnscorableReason"]["enum"],
        json!(reasons)
    );
    for (i, r) in UnscorableReason::ALL.iter().enumerate() {
        assert_eq!(serde_json::to_value(r).unwrap(), json!(reasons[i]));
    }
    let files: Vec<&str> = schema::documents().iter().map(|d| d.file_name).collect();
    for f in [
        "session-v1.6.json",
        "session-v1.7.json",
        "session-v1.8.json",
    ] {
        assert!(files.contains(&f), "{f} exported and checked");
    }
    // 1.6 and 1.7 are unchanged.
    assert_eq!(
        read_json("docs/spec/schemas/asd/session-v1.7.json"),
        schema::session_v1_7()
    );
    assert_eq!(
        read_json("docs/spec/schemas/asd/session-v1.6.json"),
        schema::session_v1_6()
    );
}

#[test]
fn required_keys_differ_from_1_7_only_by_the_scoring_rule() {
    let v17 = schema::session_v1_7();
    let v18 = s18();
    let set = |s: &Value, ptr: &str| -> Vec<String> {
        let mut v: Vec<String> = serde_json::from_value(s.pointer(ptr).unwrap().clone()).unwrap();
        v.sort();
        v
    };
    let mut top17 = set(&v17, "/required");
    top17.retain(|k| k != "socialPreferenceScore");
    top17.push("scoringStatus".into());
    top17.sort();
    assert_eq!(top17, set(&v18, "/required"));
    let mut raw17 = set(&v17, "/$defs/GazeSessionRecord/required");
    raw17.retain(|k| !SCORE_FIELDS.contains(&k.as_str()));
    raw17.push("scoringStatus".into());
    raw17.sort();
    assert_eq!(raw17, set(&v18, "/$defs/GazeSessionRecord/required"));
    assert_eq!(
        v17["$defs"]["SessionPerVideoScore"],
        v18["$defs"]["SessionPerVideoScore"]
    );
}

// ---- the scoring rule ---------------------------------------------------------------------

#[test]
fn scored_without_a_score_field_is_rejected_by_schema_and_rule() {
    let mut cases: Vec<String> = vec!["/socialPreferenceScore".into()];
    cases.extend(SCORE_FIELDS.iter().map(|k| format!("/rawPayload/{k}")));
    for pointer in cases {
        for null in [false, true] {
            let mut v = scored();
            let (parent, key) = pointer.rsplit_once('/').unwrap();
            let parent = if parent.is_empty() { "" } else { parent };
            let obj = v.pointer_mut(parent).unwrap().as_object_mut().unwrap();
            if null {
                obj.insert(key.into(), Value::Null);
            } else {
                obj.remove(key);
            }
            assert!(
                schema::validate(&s18(), &v).is_err(),
                "{pointer} (null={null}) must be rejected by the schema"
            );
            let typed = decode(&v).expect("still decodes: the rule is not a decode error");
            assert_eq!(
                pointers(&typed),
                std::slice::from_ref(&pointer),
                "null={null}"
            );
        }
    }
}

#[test]
fn unscorable_with_a_score_number_is_rejected_by_schema_and_rule() {
    let mut cases: Vec<String> = vec!["/socialPreferenceScore".into()];
    cases.extend(SCORE_FIELDS.iter().map(|k| format!("/rawPayload/{k}")));
    for pointer in cases {
        let mut v = unscorable();
        let (parent, key) = pointer.rsplit_once('/').unwrap();
        v.pointer_mut(parent)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert(key.into(), json!(0.0));
        assert!(
            schema::validate(&s18(), &v).is_err(),
            "{pointer} must be rejected by the schema"
        );
        assert_eq!(pointers(&decode(&v).unwrap()), [pointer]);
    }
}

#[test]
fn reason_on_a_scored_session_and_status_mismatch_are_rejected() {
    let mut v = scored();
    v["unscorableReason"] = json!("other");
    v["rawPayload"]["unscorableReason"] = json!("other");
    assert!(schema::validate(&s18(), &v).is_err());
    assert_eq!(pointers(&decode(&v).unwrap()), ["/unscorableReason"]);

    let mut v = scored();
    v["rawPayload"]["scoringStatus"] = json!("unscorable");
    assert!(
        schema::validate(&s18(), &v).is_err(),
        "record says unscorable but has scores"
    );
    assert!(pointers(&decode(&v).unwrap()).contains(&"/rawPayload/scoringStatus".to_string()));

    let mut v = unscorable();
    v["rawPayload"]["unscorableReason"] = json!("other");
    assert_eq!(
        pointers(&decode(&v).unwrap()),
        ["/rawPayload/unscorableReason"]
    );
}

#[test]
fn unscorable_with_a_device_flag_is_rejected_by_the_rule() {
    let mut v = unscorable();
    v["attentionFlag"] = json!("red");
    assert_eq!(pointers(&decode(&v).unwrap()), ["/attentionFlag"]);
    v.as_object_mut().unwrap().remove("attentionFlag");
    assert!(pointers(&decode(&v).unwrap()).is_empty(), "absent is fine");
}

#[test]
fn scoring_status_is_required_and_typed() {
    for remove_from in ["", "/rawPayload"] {
        let mut v = scored();
        v.pointer_mut(remove_from)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .remove("scoringStatus");
        assert!(schema::validate(&s18(), &v).is_err(), "{remove_from}");
        assert!(decode(&v).is_err(), "{remove_from}");
    }
    let mut v = scored();
    v["scoringStatus"] = json!("failed");
    assert!(schema::validate(&s18(), &v).is_err());
    assert!(decode(&v).is_err());
    let mut v = unscorable();
    v["unscorableReason"] = json!("bored");
    assert!(schema::validate(&s18(), &v).is_err());
    assert!(decode(&v).is_err());
}

// ---- 1.7 relation -------------------------------------------------------------------------

#[test]
fn a_1_7_body_is_not_a_1_8_body() {
    let v = fixture_v1_7();
    assert!(
        schema::validate(&s18(), &v).is_err(),
        "scoringStatus is required"
    );
    assert!(decode(&v).is_err());
}

#[test]
fn upgrading_1_7_is_lossless_and_scored() {
    let v17: session_v1_7::SessionIngestRequest = serde_json::from_value(fixture_v1_7()).unwrap();
    let up = upgrade_v1_7(v17.clone());
    assert_eq!(up.scoring_status, ScoringStatus::Scored);
    assert_eq!(up.raw_payload.scoring_status, ScoringStatus::Scored);
    assert_eq!(up.schema_version, "1.7", "the sent version is kept");
    assert!(scoring_violations(&up).is_empty());
    assert_eq!(
        up.social_preference_score,
        Some(v17.social_preference_score)
    );
    assert_eq!(
        up.raw_payload.surprise_score_mean,
        Some(v17.raw_payload.surprise_score_mean)
    );
    // Same JSON as the 1.7 document plus the two status keys.
    let mut expected = fixture_v1_7();
    expected["scoringStatus"] = json!("scored");
    expected["rawPayload"]["scoringStatus"] = json!("scored");
    assert_eq!(jcs(&up), jcs(&expected));
}

// ---- /v1 DTOs -----------------------------------------------------------------------------

#[test]
fn session_dtos_carry_the_scoring_status_as_nullable() {
    use asd_contract::api_v1::{ReviewReason, SessionDetail, SessionSummary};
    let api = schema::api_v1();
    let defs = &api["$defs"];
    for field in ["scoring_status", "unscorable_reason"] {
        assert!(
            defs["SessionSummary"]["properties"].get(field).is_some(),
            "{field}"
        );
        let required = defs["SessionSummary"]["required"].as_array().unwrap();
        assert!(
            !required.contains(&json!(field)),
            "{field} must be optional"
        );
    }
    assert_eq!(
        defs["ScoringStatus"]["enum"],
        json!(["scored", "unscorable"])
    );
    assert!(defs["UnscorableReason"]["enum"]
        .as_array()
        .unwrap()
        .contains(&json!("aggregation_failed")));
    assert!(defs["ReviewReason"]["enum"]
        .as_array()
        .unwrap()
        .contains(&json!("unscorable")));
    assert_eq!(
        serde_json::to_value(ReviewReason::Unscorable).unwrap(),
        "unscorable"
    );

    // 1.6 example: null status. Unscorable 1.8 example: no score numbers.
    let first: SessionSummary =
        serde_json::from_value(defs["SessionSummary"]["examples"][0].clone()).unwrap();
    assert_eq!(first.scoring_status, None);
    assert_eq!(
        defs["SessionSummary"]["examples"][0]["scoring_status"],
        Value::Null
    );
    let detail: SessionDetail =
        serde_json::from_value(defs["SessionDetail"]["examples"][2].clone()).unwrap();
    assert_eq!(
        detail.summary.scoring_status,
        Some(ScoringStatus::Unscorable)
    );
    assert_eq!(
        detail.summary.unscorable_reason,
        Some(UnscorableReason::AggregationFailed)
    );
    assert_eq!(detail.social_preference, None);
    assert_eq!(detail.schema_version.as_deref(), Some("1.8"));
}
