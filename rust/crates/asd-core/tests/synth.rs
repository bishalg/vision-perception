//! owner: WP-R.4
//!
//! Ports the 7 cases of `tools/asd_synth/test_asd_synth.py`, adjusted to the CON-13 contract
//! (ratios, three statuses), plus a golden payload and a norms-pipeline round trip.
#![cfg(feature = "synth")]

use std::collections::BTreeSet;

use asd_core::norms::{aggregate, AggregateParams};
use asd_core::synth::{
    age_bands, generate_session, generate_sessions, is_synthetic, Profile, SynthError, PROFILES,
};
use serde_json::{json, Value};

/// Minimal JSON-Schema checker for the subset `session-v1.6.json` uses
/// (`type`, `required`, `properties`, `items`, `anyOf`, `enum`, `minimum`, `maximum`,
/// `$ref` into `$defs`). `asd-cli synth` also checks every payload with the full validator.
fn validate(schema: &Value, root: &Value, v: &Value, path: &str, errs: &mut Vec<String>) {
    if let Some(r) = schema.get("$ref").and_then(Value::as_str) {
        let name = r.rsplit('/').next().unwrap();
        return validate(&root["$defs"][name], root, v, path, errs);
    }
    if let Some(any) = schema.get("anyOf").and_then(Value::as_array) {
        let ok = any.iter().any(|s| {
            let mut e = Vec::new();
            validate(s, root, v, path, &mut e);
            e.is_empty()
        });
        if !ok {
            errs.push(format!("{path}: no anyOf branch matches {v}"));
        }
        return;
    }
    if let Some(allowed) = schema.get("enum").and_then(Value::as_array) {
        if !allowed.contains(v) {
            errs.push(format!("{path}: {v} not in {allowed:?}"));
        }
    }
    if let Some(x) = v.as_f64() {
        if schema
            .get("minimum")
            .and_then(Value::as_f64)
            .is_some_and(|m| x < m)
        {
            errs.push(format!("{path}: {x} below minimum"));
        }
        if schema
            .get("maximum")
            .and_then(Value::as_f64)
            .is_some_and(|m| x > m)
        {
            errs.push(format!("{path}: {x} above maximum"));
        }
    }
    match schema.get("type").and_then(Value::as_str) {
        Some("string") if !v.is_string() => errs.push(format!("{path}: not a string")),
        Some("number") if !v.is_number() => errs.push(format!("{path}: not a number")),
        Some("integer") if !(v.is_i64() || v.is_u64()) => errs.push(format!("{path}: not integer")),
        Some("null") if !v.is_null() => errs.push(format!("{path}: not null")),
        Some("array") => match v.as_array() {
            None => errs.push(format!("{path}: not an array")),
            Some(a) => {
                for (i, x) in a.iter().enumerate() {
                    validate(&schema["items"], root, x, &format!("{path}[{i}]"), errs);
                }
            }
        },
        Some("object") => match v.as_object() {
            None => errs.push(format!("{path}: not an object")),
            Some(o) => {
                for req in schema["required"].as_array().into_iter().flatten() {
                    if !o.contains_key(req.as_str().unwrap()) {
                        errs.push(format!("{path}: missing {req}"));
                    }
                }
                for (k, sub) in schema["properties"].as_object().into_iter().flatten() {
                    if let Some(x) = o.get(k) {
                        validate(sub, root, x, &format!("{path}.{k}"), errs);
                    }
                }
            }
        },
        _ => {}
    }
}

fn schema() -> Value {
    let p = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../docs/spec/schemas/asd/session-v1.6.json"
    );
    serde_json::from_str(&std::fs::read_to_string(p).expect("schema file")).unwrap()
}

#[test]
fn all_outputs_schema_valid() {
    let s = schema();
    for p in generate_sessions(200, 3) {
        let mut errs = Vec::new();
        validate(&s, &s, &p, "$", &mut errs);
        assert!(errs.is_empty(), "{errs:?}");
    }
}

#[test]
fn validator_rejects_bad_payload() {
    let s = schema();
    let mut p = generate_session(1, 0, None);
    p["perVideo"][0]["order"] = json!("x");
    let mut errs = Vec::new();
    validate(&s, &s, &p, "$", &mut errs);
    assert!(!errs.is_empty());

    // CON-2 specifics the old generator violated.
    for (pointer, bad) in [
        ("/qualityVerdict", json!("insufficient")),
        ("/validRatio", json!(1.5)),
        ("/perVideo/0/dwell_social", json!(2.0)),
        ("/perVideo/0/details/dwellUnits", json!("seconds")),
        ("/captureSource", json!("webcam")),
    ] {
        let mut p = generate_session(1, 0, None);
        *p.pointer_mut(pointer).unwrap() = bad;
        let mut errs = Vec::new();
        validate(&s, &s, &p, "$", &mut errs);
        assert!(!errs.is_empty(), "{pointer} accepted");
    }
    for key in ["rawPayload", "qualityVerdict"] {
        let mut p = generate_session(1, 0, None);
        p.as_object_mut().unwrap().remove(key);
        let mut errs = Vec::new();
        validate(&s, &s, &p, "$", &mut errs);
        assert!(!errs.is_empty(), "missing {key} accepted");
    }
}

#[test]
fn deterministic_and_seed_sensitive() {
    assert_eq!(generate_sessions(30, 5), generate_sessions(30, 5));
    assert_ne!(generate_session(5, 1, None), generate_session(6, 1, None));
    assert_eq!(generate_session(5, 2, None), generate_sessions(3, 5)[2]);
}

#[test]
fn clearly_synthetic() {
    for p in generate_sessions(50, 0) {
        assert!(is_synthetic(&p));
        assert!(p["participantToken"]
            .as_str()
            .unwrap()
            .starts_with("synth-"));
        assert!(p["rawPayload"]["notice"]
            .as_str()
            .unwrap()
            .contains("SYNTHETIC"));
    }
}

#[test]
fn variety_and_edge_cases() {
    let ps = generate_sessions(100, 1);
    let bands: BTreeSet<_> = ps
        .iter()
        .map(|p| p["rawPayload"]["ageBand"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(bands, age_bands().into_iter().map(|b| b.id).collect());
    let profiles: BTreeSet<_> = ps
        .iter()
        .map(|p| p["rawPayload"]["profile"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(
        profiles,
        PROFILES.iter().map(|p| p.as_str().to_string()).collect()
    );
    let batteries: BTreeSet<_> = ps.iter().map(|p| p["battery"].as_str().unwrap()).collect();
    assert_eq!(
        batteries,
        BTreeSet::from(["comprehensive_60s", "express_30s"])
    );
    let clips: BTreeSet<_> = ps
        .iter()
        .flat_map(|p| p["perVideo"].as_array().unwrap())
        .map(|c| c["clip_id"].as_str().unwrap())
        .collect();
    assert!(clips.len() >= 6);
    let low: Vec<_> = ps
        .iter()
        .filter(|p| p["rawPayload"]["profile"] == "low_quality")
        .collect();
    assert!(!low.is_empty());
    assert!(low.iter().all(|p| p["qualityVerdict"] == "rejected"
        && p["rawPayload"]["qualityVerdict"] == "rejected"
        && p["validRatio"].as_f64().unwrap() < 0.55));
}

#[test]
fn value_ranges_and_unique_ids() {
    let ps = generate_sessions(100, 0);
    assert_eq!(
        ps.iter()
            .map(|p| p["sessionId"].as_str().unwrap())
            .collect::<BTreeSet<_>>()
            .len(),
        100
    );
    for p in &ps {
        for k in ["validRatio", "socialPreferenceScore"] {
            assert!((0.0..=1.0).contains(&p[k].as_f64().unwrap()));
        }
        for c in p["perVideo"].as_array().unwrap() {
            assert!(c["fixation_count"].as_i64().unwrap() >= 1);
        }
    }
}

#[test]
fn unknown_profile_rejected() {
    assert_eq!(
        "bogus".parse::<Profile>(),
        Err(SynthError::UnknownProfile("bogus".into()))
    );
    assert_eq!("short".parse::<Profile>(), Ok(Profile::Short));
}

/// CON-13: ratios not seconds, three statuses, never `simulated`; CON-2 record fields.
#[test]
fn con13_ratios_statuses_and_capture_source() {
    let mut statuses = BTreeSet::new();
    for p in generate_sessions(300, 11) {
        assert_eq!(p["captureSource"], "synthetic");
        for c in p["perVideo"].as_array().unwrap() {
            for k in [
                "dwell_social",
                "dwell_geometric",
                "dwell_eye",
                "dwell_mouth",
            ] {
                assert!(
                    (0.0..=1.0).contains(&c[k].as_f64().unwrap()),
                    "{k} not a ratio"
                );
            }
            let total =
                c["dwell_social"].as_f64().unwrap() + c["dwell_geometric"].as_f64().unwrap();
            assert!(total <= 1.0 + 1e-9);
            statuses.insert(c["status"].as_str().unwrap().to_string());
            assert_eq!(c["details"]["dwellUnits"], "ratio");
            assert!(!c["details"]["rows"].as_array().unwrap().is_empty());
        }
        let raw = &p["rawPayload"];
        assert_eq!(raw["sessionId"], p["sessionId"]);
        assert_eq!(raw["participantId"], p["participantToken"]);
        assert_eq!(raw["captureSource"], "synthetic");
        assert_eq!(raw["schemaVersion"], "v1.6");
        assert_eq!(p["schemaVersion"], "1.6");
        assert_eq!(raw["zeroRawVideoEnforced"], true);
        assert!(raw["startedAt"].as_str().unwrap() < raw["endedAt"].as_str().unwrap());
    }
    assert!(
        statuses
            .iter()
            .all(|s| ["onTarget", "monitor", "atypical"].contains(&s.as_str())),
        "{statuses:?}"
    );
    assert!(statuses.contains("onTarget"));
}

#[test]
fn golden_payloads() {
    let sessions: Vec<Value> = (0..5).map(|i| generate_session(7, i, None)).collect();
    let actual = serde_json::to_string_pretty(&sessions).unwrap() + "\n";
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/golden/synth/seed7_count5.json"
    );
    if std::env::var("ASD_WRITE_GOLDEN").as_deref() == Ok("1") {
        std::fs::write(path, &actual).unwrap();
    }
    let expected =
        std::fs::read_to_string(path).expect("golden file (run with ASD_WRITE_GOLDEN=1)");
    assert_eq!(actual, expected);
}

/// Acceptance 13: 30 synthetic vault exports -> candidate with every n<12 cell withheld.
#[test]
fn synthetic_sessions_feed_the_norm_aggregator() {
    let records: Vec<Value> = generate_sessions(30, 7)
        .iter()
        .map(|p| {
            let raw = &p["rawPayload"];
            json!({"type": "per_video_score",
                   "score": {"ageMonths": raw["ageMonths"], "videos": raw["perVideo"]}})
        })
        .collect();
    let set = aggregate(&records, &AggregateParams::default());
    assert_eq!(set.status, "candidate-collected");
    assert!(!set.videos.is_empty());
    for v in set.videos.values() {
        for e in &v.segments[0].expectations {
            for cell in e.by_age.values().flatten() {
                assert!(cell.n >= 12);
            }
        }
    }
}
