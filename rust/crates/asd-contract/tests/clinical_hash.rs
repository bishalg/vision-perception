//! owner: WP-C.9
//!
//! SIGN-4: `content_hash = SHA-256(JCS(ClinicalReviewSnapshot))`. The hash must not depend
//! on JSON key order or float spelling (`1.0` vs `1`), and must change when any field of
//! the snapshot changes. The letter snapshot (NOTE-3) follows the same rule.

use asd_contract::clinical::{
    content_hash, content_hash_hex, letter_content_hash, ClinicalReviewSnapshot, Disposition,
    LetterKind, LetterSnapshot,
};
use serde_json::{json, Value};

fn snapshot_json() -> Value {
    json!({
        "session_id": "SESS-1",
        "org_id": "6f1c1b2e-6d1a-4c55-9a43-0c6f0e0c2b11",
        "bundle_version": "2026.10.1",
        "schema_version": "1.6",
        "measurements": {
            "started_at": "2026-10-01T10:00:00.000Z",
            "duration_sec": 30.0,
            "quality_verdict": "good",
            "attention_flag": "red",
            "composite_score": 0.5,
            "composite_confidence": 1.0,
            "valid_frame_ratio": 0.9,
            "calibrated_distance_cm": 45.0,
            "social_preference": 0.25,
            "eye_dwell": 0.125,
            "mouth_dwell": 0.0625,
            "first_eye_latency_ms": 420.0
        },
        "clips": [{
            "clip_id": "clip-1",
            "order": 1,
            "rows": [{
                "segment_id": "seg-1",
                "metric": "eye_dwell_ratio",
                "target_aoi": "eyes",
                "direction": "higherBetter",
                "expected": 0.4,
                "acceptable_low": 0.3,
                "acceptable_high": 0.5,
                "recorded": 0.2,
                "deviation": -0.2,
                "status": "atypical",
                "score": 1.0,
                "imputed": false,
                "source": "measured"
            }]
        }],
        "disposition": "monitor",
        "impression": "Reduced eye dwell in clip 1.",
        "recommendations": "Re-check in 3 months.",
        "version_no": 1
    })
}

type Mutation = Box<dyn Fn(&mut Value)>;

fn snapshot(v: Value) -> ClinicalReviewSnapshot {
    serde_json::from_value(v).expect("snapshot parses")
}

/// Rebuilds every object with its keys in reverse order (serde_json without
/// `preserve_order` sorts them on output, so this is checked on the text form).
fn reversed_text(v: &Value) -> String {
    fn write(v: &Value, out: &mut String) {
        match v {
            Value::Object(m) => {
                out.push('{');
                for (i, (k, val)) in m.iter().rev().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    out.push_str(&serde_json::to_string(k).unwrap());
                    out.push(':');
                    write(val, out);
                }
                out.push('}');
            }
            Value::Array(a) => {
                out.push('[');
                for (i, val) in a.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    write(val, out);
                }
                out.push(']');
            }
            other => out.push_str(&serde_json::to_string(other).unwrap()),
        }
    }
    let mut s = String::new();
    write(v, &mut s);
    s
}

#[test]
fn hash_is_stable_across_key_order() {
    let a = snapshot(snapshot_json());
    let text = reversed_text(&snapshot_json());
    assert!(text.starts_with("{\"version_no\""), "{text}");
    let b: ClinicalReviewSnapshot = serde_json::from_str(&text).unwrap();
    assert_eq!(content_hash(&a), content_hash(&b));
}

#[test]
fn hash_is_stable_across_float_spelling() {
    let text = serde_json::to_string(&snapshot_json()).unwrap();
    // `30.0` vs `30`, `1.0` vs `1`, `0.0625` vs `6.25e-2`.
    let respelled = text
        .replace("\"duration_sec\":30.0", "\"duration_sec\":30")
        .replace("\"composite_confidence\":1.0", "\"composite_confidence\":1")
        .replace("\"mouth_dwell\":0.0625", "\"mouth_dwell\":6.25e-2");
    assert_ne!(text, respelled);
    let a: ClinicalReviewSnapshot = serde_json::from_str(&text).unwrap();
    let b: ClinicalReviewSnapshot = serde_json::from_str(&respelled).unwrap();
    assert_eq!(content_hash(&a), content_hash(&b));
}

#[test]
fn hash_is_sha256_of_rfc8785_bytes() {
    let s = snapshot(snapshot_json());
    let hex = content_hash_hex(&s);
    assert_eq!(hex.len(), 64);
    assert!(hex
        .bytes()
        .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()));
    let bytes = content_hash(&s);
    let again: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(hex, again);

    // Exactly SHA-256 over the RFC 8785 bytes (ECMAScript numbers: `30`, not `30.0`), not
    // over `serde_json::to_vec` (trap 7).
    let jcs = asd_contract::signing::canonical_bytes(&s).unwrap();
    let text = String::from_utf8(jcs.clone()).unwrap();
    assert!(text.contains(r#""duration_sec":30,"#), "{text}");
    assert!(text.contains(r#""score":1,"#), "{text}");
    use sha2::{Digest, Sha256};
    let want: [u8; 32] = Sha256::digest(&jcs).into();
    assert_eq!(bytes, want);
    let serde_bytes: [u8; 32] = Sha256::digest(serde_json::to_vec(&s).unwrap()).into();
    assert_ne!(
        bytes, serde_bytes,
        "the fixture must tell JCS and serde_json apart"
    );
}

/// Golden: signatures made by one release must verify in the next, so this value only
/// changes together with a deliberate, versioned change of the snapshot type.
#[test]
fn golden_hash() {
    assert_eq!(
        content_hash_hex(&snapshot(snapshot_json())),
        "c4875982d5d7e3902d82881cc4868e5da599ceb1309a0703684343e74d5082a5"
    );
}

#[test]
fn hash_changes_when_any_field_changes() {
    let base = content_hash(&snapshot(snapshot_json()));
    let mutations: Vec<(&str, Mutation)> = vec![
        (
            "session_id",
            Box::new(|v| v["session_id"] = json!("SESS-2")),
        ),
        (
            "org_id",
            Box::new(|v| v["org_id"] = json!("00000000-0000-0000-0000-000000000000")),
        ),
        (
            "bundle_version",
            Box::new(|v| v["bundle_version"] = json!("x")),
        ),
        (
            "bundle_version null",
            Box::new(|v| v["bundle_version"] = Value::Null),
        ),
        (
            "schema_version",
            Box::new(|v| v["schema_version"] = json!("1.5")),
        ),
        (
            "measurements.eye_dwell",
            Box::new(|v| v["measurements"]["eye_dwell"] = json!(0.126)),
        ),
        (
            "measurements.attention_flag",
            Box::new(|v| v["measurements"]["attention_flag"] = json!("green")),
        ),
        (
            "measurements.started_at",
            Box::new(|v| v["measurements"]["started_at"] = json!("2026-10-01T10:00:01.000Z")),
        ),
        (
            "clip row recorded",
            Box::new(|v| v["clips"][0]["rows"][0]["recorded"] = json!(0.21)),
        ),
        (
            "clip row imputed",
            Box::new(|v| v["clips"][0]["rows"][0]["imputed"] = json!(true)),
        ),
        (
            "clip order",
            Box::new(|v| v["clips"][0]["order"] = json!(2)),
        ),
        ("clips empty", Box::new(|v| v["clips"] = json!([]))),
        (
            "disposition",
            Box::new(|v| v["disposition"] = json!("refer_for_evaluation")),
        ),
        (
            "disposition null",
            Box::new(|v| v["disposition"] = Value::Null),
        ),
        (
            "impression",
            Box::new(|v| v["impression"] = json!("Reduced eye dwell in clip 1")),
        ),
        (
            "recommendations",
            Box::new(|v| v["recommendations"] = Value::Null),
        ),
        ("version_no", Box::new(|v| v["version_no"] = json!(2))),
    ];
    for (name, mutate) in mutations {
        let mut v = snapshot_json();
        mutate(&mut v);
        assert_ne!(
            content_hash(&snapshot(v)),
            base,
            "{name} did not change the hash"
        );
    }
}

#[test]
fn snapshot_round_trips_its_typed_fields() {
    let s = snapshot(snapshot_json());
    assert_eq!(s.disposition, Some(Disposition::Monitor));
    assert_eq!(s.version_no, 1);
    assert_eq!(s.clips[0].rows.len(), 1);
}

#[test]
fn letter_hash_changes_with_body_and_recipients() {
    let letter = LetterSnapshot {
        letter_id: "6f1c1b2e-6d1a-4c55-9a43-0c6f0e0c2b12".into(),
        org_id: "6f1c1b2e-6d1a-4c55-9a43-0c6f0e0c2b11".into(),
        participant_id: "6f1c1b2e-6d1a-4c55-9a43-0c6f0e0c2b13".into(),
        kind: LetterKind::Referral,
        session_ids: vec!["SESS-1".into()],
        recipient: "Dr. A, Child Development Clinic".into(),
        body: "Dear colleague,".into(),
        row_version: 3,
    };
    let base = letter_content_hash(&letter);
    assert_eq!(base, letter_content_hash(&letter.clone()));
    let mut b = letter.clone();
    b.body.push('!');
    assert_ne!(letter_content_hash(&b), base);
    let mut r = letter.clone();
    r.recipient.push('.');
    assert_ne!(letter_content_hash(&r), base);
    let mut s = letter;
    s.session_ids.push("SESS-2".into());
    assert_ne!(letter_content_hash(&s), base);
}

#[test]
fn ctr2_clinical_slugs_are_published() {
    use asd_contract::problem::SLUGS;
    for slug in [
        "clinical-crypto-unavailable",
        "patient-ref-disabled",
        "stale-version",
        "content-changed",
        "reauthentication-failed",
        "cosigner-must-differ",
        "signer-profile-incomplete",
        "export-expired",
        "export-too-large",
        "consent-purpose-mismatch",
        "host-key-mismatch",
    ] {
        assert!(SLUGS.contains(&slug), "{slug}");
    }
    let api = asd_contract::schema::api_v1();
    let defs = &api["$defs"];
    for name in [
        "ClinicalReviewSnapshot",
        "ClinicalReview",
        "SignRequest",
        "WorklistPage",
        "PatientRef",
        "ClinicalNote",
        "ReferralLetter",
        "TimelinePage",
        "UserProfile",
    ] {
        assert!(defs.get(name).is_some(), "{name} not exported");
    }
    assert!(defs["OrgUser"]["properties"].get("display_name").is_some());
    let slugs = defs["ProblemSlug"]["enum"].as_array().unwrap();
    assert!(slugs.iter().any(|s| s == "content-changed"));
}
