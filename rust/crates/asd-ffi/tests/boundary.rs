//! owner: WP-R.5
//!
//! FFI-1: every exported entry point, called exactly as Swift/Kotlin call it (JSON strings
//! and scalars in, JSON strings, scalars or small records out).
//! - Parity: the golden vectors of `asd-core/tests/golden` (Swift oracle output) pass through
//!   the boundary unchanged (floats to 1e-9).
//! - Robustness: malformed or wrong-shaped JSON returns `AsdError::InvalidJson`, never a panic.
//! - CON-6: `verify_bundle` accepts a signed bundle and refuses a tampered one.
//! - CON-7: the sync entry points match `asd-sync`.

use std::path::PathBuf;

use asd_contract::config::{ConfigBundle, ScoringProfile};
use asd_contract::signing;
use asd_ffi::*;
use serde_json::{json, Value};

const TOLERANCE: f64 = 1e-9;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../asd-core/tests/golden")
}

fn read_json(path: PathBuf) -> Value {
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path:?}: {e}"));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{path:?}: {e}"))
}

/// `(name, input, expected)` of a multi-vector golden file.
fn vectors(rel: &str) -> Vec<(String, Value, Value)> {
    let doc = read_json(golden_dir().join(rel));
    let list: Vec<_> = doc["vectors"]
        .as_array()
        .expect("vectors")
        .iter()
        .map(|v| {
            (
                v["name"].as_str().unwrap().to_owned(),
                v["input"].clone(),
                v["expected"].clone(),
            )
        })
        .collect();
    assert!(!list.is_empty(), "{rel}");
    list
}

/// `(case, input, expected)` of every single-case golden file in a directory.
fn cases(dir: &str) -> Vec<(String, Value, Value)> {
    let mut paths: Vec<_> = std::fs::read_dir(golden_dir().join(dir))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    paths.sort();
    assert!(!paths.is_empty(), "{dir}");
    paths
        .into_iter()
        .map(|p| {
            let doc = read_json(p.clone());
            (
                p.display().to_string(),
                doc["input"].clone(),
                doc["expected"].clone(),
            )
        })
        .collect()
}

fn assert_close(actual: &Value, expected: &Value, path: &str) {
    match (actual, expected) {
        (Value::Number(a), Value::Number(e)) => {
            let (a, e) = (a.as_f64().unwrap(), e.as_f64().unwrap());
            assert!((a - e).abs() <= TOLERANCE, "{path}: {a} != {e}");
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

fn parse(s: &str) -> Value {
    serde_json::from_str(s).expect("FFI output is JSON")
}

fn s(v: &Value) -> String {
    v.to_string()
}

fn is_invalid_json<T: std::fmt::Debug>(r: Result<T, AsdError>) -> bool {
    matches!(r, Err(AsdError::InvalidJson { .. }))
}

const MALFORMED: [&str; 6] = ["", "{", "null", "[1,2", "\"text\"", "{\"a\":NaN}"];

// --- version -------------------------------------------------------------------------

#[test]
fn core_version_is_the_crate_version() {
    assert_eq!(core_version(), env!("CARGO_PKG_VERSION"));
}

// --- scoring -------------------------------------------------------------------------

#[test]
fn score_session_matches_golden_vectors() {
    for (name, input, expected) in vectors("scoring/score_session_v1.json") {
        let out = score_session_json(
            s(&input["metricSets"]),
            s(&input["baseline"]),
            input["ageMonths"].as_i64().unwrap(),
            s(&input["frames"]),
            input["scoredAtUtc"].as_str().unwrap().to_owned(),
        )
        .unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_close(&parse(&out), &expected, &name);
    }
}

#[test]
fn score_session_rejects_malformed_json() {
    let input = &vectors("scoring/score_session_v1.json")[0].1;
    for bad in MALFORMED {
        let call = |m: &str, b: &str, f: &str| {
            score_session_json(m.into(), b.into(), 24, f.into(), "x".into())
        };
        let (m, b, f) = (
            s(&input["metricSets"]),
            s(&input["baseline"]),
            s(&input["frames"]),
        );
        assert!(is_invalid_json(call(bad, &b, &f)), "metric sets {bad:?}");
        assert!(is_invalid_json(call(&m, bad, &f)), "baseline {bad:?}");
        assert!(is_invalid_json(call(&m, &b, bad)), "frames {bad:?}");
    }
    // Valid JSON of the wrong shape.
    let r = score_session_json("{}".into(), "[]".into(), 24, "[]".into(), "x".into());
    assert!(is_invalid_json(r));
}

#[test]
fn format_scored_at_matches_golden_vectors() {
    for (name, input, expected) in vectors("scoring/scored_at_v1.json") {
        let unix = input["unixSeconds"]
            .as_f64()
            .unwrap_or_else(|| panic!("{name}: {input}"));
        assert_eq!(
            format_scored_at(unix),
            expected["scoredAtUtc"].as_str().unwrap(),
            "{name}"
        );
    }
}

// --- flag ----------------------------------------------------------------------------

fn default_profile() -> String {
    default_scoring_profile_json()
}

#[test]
fn default_scoring_profile_is_the_contract_default() {
    let p: ScoringProfile = serde_json::from_str(&default_scoring_profile_json()).unwrap();
    assert_eq!(p, ScoringProfile::default());
}

#[test]
fn assess_flag_matches_golden_vectors_with_default_profile() {
    for (name, input, expected) in vectors("flag/assess_v1.json") {
        let out = assess_flag_json(s(&input["score"]), default_profile())
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_close(&parse(&out), &expected, &name);
    }
}

#[test]
fn assess_flag_uses_profile_thresholds() {
    let (_, input, expected) = vectors("flag/assess_v1.json").remove(0);
    assert_eq!(expected["flag"], "green");
    let mut profile: Value = parse(&default_profile());
    // A green floor above any composite turns the same score away from green.
    profile["flag"]["greenFloor"] = json!(1.5);
    let out = assess_flag_json(s(&input["score"]), s(&profile)).unwrap();
    assert_ne!(parse(&out)["flag"], "green");
}

#[test]
fn assess_flag_rejects_malformed_json() {
    let score = s(&vectors("flag/assess_v1.json")[0].1["score"]);
    for bad in MALFORMED {
        assert!(is_invalid_json(assess_flag_json(
            bad.into(),
            default_profile()
        )));
        assert!(is_invalid_json(assess_flag_json(score.clone(), bad.into())));
    }
    // A profile without `flag` is not a ScoringProfile.
    assert!(is_invalid_json(assess_flag_json(score, "{}".into())));
}

// --- metrics and aggregation ---------------------------------------------------------

#[test]
fn extract_metrics_matches_golden_cases() {
    let all = cases("metrics");
    let mut ran = 0;
    for (name, input, expected) in all {
        // Clip-level cases call extract_clip, which is not exported.
        let Some(frames) = input
            .get("frames")
            .filter(|_| input.as_object().unwrap().len() == 1)
        else {
            continue;
        };
        let out = extract_metrics_json(s(frames)).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_close(&parse(&out), &expected, &name);
        ran += 1;
    }
    assert!(ran >= 10, "only {ran} metrics cases ran");
}

#[test]
fn aggregate_session_matches_golden_cases() {
    for (name, input, expected) in cases("aggregate") {
        let out = aggregate_session_json(
            s(&input["validFrames"]),
            input["durationSec"].as_f64().unwrap(),
        )
        .unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_close(&parse(&out), &expected, &name);
    }
}

#[test]
fn metrics_and_aggregate_reject_malformed_json() {
    for bad in MALFORMED {
        assert!(is_invalid_json(extract_metrics_json(bad.into())), "{bad:?}");
        assert!(
            is_invalid_json(aggregate_session_json(bad.into(), 1.0)),
            "{bad:?}"
        );
    }
    assert!(is_invalid_json(extract_metrics_json("[{}]".into())));
    assert!(is_invalid_json(aggregate_session_json("{}".into(), 1.0)));
}

#[test]
fn non_finite_duration_does_not_panic() {
    for d in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(aggregate_session_json("[]".into(), d).is_ok());
    }
}

// --- quality -------------------------------------------------------------------------

#[test]
fn evaluate_quality_matches_golden_vectors() {
    for (name, input, expected) in vectors("quality/evaluate_sample_v1.json") {
        let out = evaluate_quality_json(s(&input["sample"]), s(&input["gate"]))
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_close(&parse(&out), &expected, &name);
    }
}

#[test]
fn evaluate_quality_accepts_the_scoring_profile_gate() {
    let (name, input, expected) = vectors("quality/evaluate_sample_v1.json").remove(0);
    let profile = parse(&default_profile());
    let out = evaluate_quality_json(s(&input["sample"]), s(&profile["qualityGate"])).unwrap();
    assert_close(&parse(&out), &expected, &name);
}

#[test]
fn evaluate_quality_rejects_malformed_json() {
    let input = &vectors("quality/evaluate_sample_v1.json")[0].1;
    for bad in MALFORMED {
        assert!(is_invalid_json(evaluate_quality_json(
            bad.into(),
            s(&input["gate"])
        )));
        assert!(is_invalid_json(evaluate_quality_json(
            s(&input["sample"]),
            bad.into()
        )));
    }
}

// --- baselines -----------------------------------------------------------------------

#[test]
fn validate_baselines_matches_golden_vectors() {
    for (name, input, expected) in vectors("scoring/validate_baselines_v1.json") {
        let ids: Vec<String> = serde_json::from_value(input["requiredVideoIds"].clone()).unwrap();
        let problems = validate_baselines_json(s(&input["baseline"]), ids)
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(json!(problems), expected["problems"], "{name}");
    }
}

#[test]
fn validate_baselines_rejects_malformed_json() {
    for bad in MALFORMED {
        assert!(is_invalid_json(validate_baselines_json(bad.into(), vec![])));
    }
}

// --- bundle verification (CON-6) -----------------------------------------------------

fn signed_bundle(seed: u8) -> (String, String) {
    let res = repo_root().join("domains/asd_screening/ios/Sources/CurvAsdGazeKit/Resources");
    let key = signing::signing_key_from_seed([seed; 32]);
    let mut bundle = ConfigBundle {
        bundle_version: "2026.10.31-1".into(),
        key_id: signing::key_id(&key.verifying_key()),
        catalog: serde_json::from_value(read_json(res.join("stimulus_catalog.json"))).unwrap(),
        baselines: serde_json::from_value(read_json(res.join("normative_baselines.json"))).unwrap(),
        scoring_profile: ScoringProfile::default(),
        feature_flags: Default::default(),
        signed_at: "2026-10-31T12:00:00.000Z".into(),
        signature: String::new(),
    };
    signing::sign_bundle(&mut bundle, &key).unwrap();
    (
        serde_json::to_string_pretty(&bundle).unwrap(),
        signing::encode_public_key(&key.verifying_key()),
    )
}

#[test]
fn verify_bundle_accepts_a_signed_bundle() {
    let (bundle, pk) = signed_bundle(3);
    assert_eq!(verify_bundle(bundle, pk), Ok(true));
}

#[test]
fn verify_bundle_accepts_standard_base64_keys() {
    let (bundle, pk) = signed_bundle(3);
    // base64url without padding -> standard base64 with padding.
    let std_key = format!("{}=", pk.replace('-', "+").replace('_', "/"));
    assert_eq!(verify_bundle(bundle, std_key), Ok(true));
}

#[test]
fn verify_bundle_refuses_tampering_wrong_keys_and_missing_signatures() {
    let (bundle, pk) = signed_bundle(3);
    let (_, other_pk) = signed_bundle(4);
    assert_eq!(verify_bundle(bundle.clone(), other_pk), Ok(false));

    let mut v = parse(&bundle);
    v["scoring_profile"]["flag"]["greenFloor"] = json!(0.1);
    assert_eq!(verify_bundle(s(&v), pk.clone()), Ok(false));

    let mut v = parse(&bundle);
    v["signature"] = json!("ed25519:AAAA");
    assert_eq!(verify_bundle(s(&v), pk.clone()), Ok(false));

    let mut v = parse(&bundle);
    v.as_object_mut().unwrap().remove("signature");
    assert_eq!(verify_bundle(s(&v), pk), Ok(false));
}

#[test]
fn verify_bundle_errors_on_bad_input() {
    let (bundle, pk) = signed_bundle(3);
    for bad in ["", "{", "[1]", "\"x\""] {
        assert!(
            is_invalid_json(verify_bundle(bad.into(), pk.clone())),
            "{bad:?}"
        );
    }
    for bad_key in ["", "not base64!", "AAAA"] {
        assert!(
            matches!(
                verify_bundle(bundle.clone(), bad_key.into()),
                Err(AsdError::InvalidArgument { .. })
            ),
            "{bad_key:?}"
        );
    }
}

// --- sync (CON-7) --------------------------------------------------------------------

fn session_fixture() -> String {
    std::fs::read_to_string(
        repo_root().join("apps/ios_asd_gaze_app/Tests/Fixtures/expected-session-v1.6.json"),
    )
    .unwrap()
}

fn client() -> AsdClientInfo {
    AsdClientInfo {
        product: "EyeGaze-iOS".into(),
        app_version: "1.0.0".into(),
        build: "3".into(),
        os_name: "iOS".into(),
        os_version: "18.0".into(),
    }
}

fn header<'a>(req: &'a AsdSyncRequest, name: &str) -> Option<&'a str> {
    req.headers
        .iter()
        .find(|h| h.name.eq_ignore_ascii_case(name))
        .map(|h| h.value.as_str())
}

#[test]
fn build_session_request_matches_asd_sync() {
    let payload = session_fixture();
    let req = sync_build_session_request_json(payload.clone(), "dvc_tok".into(), client()).unwrap();
    let expected = asd_sync::build_session_request_json(
        payload.as_bytes(),
        "dvc_tok",
        &asd_sync::ClientInfo::ios("1.0.0", "3", "18.0"),
    )
    .unwrap();
    assert_eq!(req.method, "POST");
    assert_eq!(req.path, asd_sync::SESSIONS_PATH);
    assert_eq!(req.body, expected.body);
    assert_eq!(req.timeout_secs, expected.timeout_secs);
    let pairs: Vec<(String, String)> = req
        .headers
        .iter()
        .map(|h| (h.name.clone(), h.value.clone()))
        .collect();
    assert_eq!(pairs, expected.headers);
    assert_eq!(header(&req, "Authorization"), Some("Bearer dvc_tok"));
    let session_id = parse(&payload)["sessionId"].as_str().unwrap().to_owned();
    assert_eq!(header(&req, "X-Idempotency-Key"), Some(session_id.as_str()));
    assert_eq!(
        header(&req, "User-Agent"),
        Some("EyeGaze-iOS/1.0.0 (3; iOS 18.0)")
    );
}

#[test]
fn build_session_request_maps_refusals() {
    let payload = session_fixture();
    assert!(matches!(
        sync_build_session_request_json(payload.clone(), "  ".into(), client()),
        Err(AsdError::NotRegistered { .. })
    ));
    let mut v = parse(&payload);
    v["captureSource"] = json!("simulated");
    assert!(matches!(
        sync_build_session_request_json(s(&v), "t".into(), client()),
        Err(AsdError::UploadRefused { .. })
    ));
    let mut v = parse(&payload);
    v["schemaVersion"] = json!("1.5");
    assert!(matches!(
        sync_build_session_request_json(s(&v), "t".into(), client()),
        Err(AsdError::UploadRefused { .. })
    ));
    for bad in MALFORMED {
        assert!(is_invalid_json(sync_build_session_request_json(
            bad.into(),
            "t".into(),
            client()
        )));
    }
}

#[test]
fn build_register_request_sends_device_id() {
    let req = sync_build_register_request(
        " CODE-1 ".into(),
        Some("dev_1".into()),
        "idem-1".into(),
        client(),
    )
    .unwrap();
    assert_eq!(req.method, "POST");
    assert_eq!(req.path, asd_sync::REGISTER_PATH);
    assert_eq!(header(&req, "X-Idempotency-Key"), Some("idem-1"));
    assert_eq!(header(&req, "Authorization"), None);
    let body: Value = serde_json::from_slice(&req.body).unwrap();
    assert_eq!(body["enrol_code"], "CODE-1");
    assert_eq!(body["device_id"], "dev_1");
    assert!(matches!(
        sync_build_register_request("  ".into(), None, "i".into(), client()),
        Err(AsdError::InvalidArgument { .. })
    ));
}

fn h(name: &str, value: &str) -> AsdHttpHeader {
    AsdHttpHeader {
        name: name.into(),
        value: value.into(),
    }
}

#[test]
fn classify_response_covers_every_outcome() {
    let body = br#"{"sessionId":"s1","status":"accepted","duplicate":true,"frames_upload_url":"https://x.example/f"}"#;
    match sync_classify_response(200, vec![], body.to_vec()) {
        AsdSyncOutcome::Accepted { ack } => {
            assert_eq!(ack.session_id.as_deref(), Some("s1"));
            assert_eq!(ack.status.as_deref(), Some("accepted"));
            assert_eq!(ack.duplicate, Some(true));
            assert_eq!(
                ack.frames_upload_url.as_deref(),
                Some("https://x.example/f")
            );
            assert_eq!(ack.report_upload_url, None);
        }
        other => panic!("{other:?}"),
    }
    // A garbage 2xx body is still accepted.
    assert!(matches!(
        sync_classify_response(201, vec![], b"<html>".to_vec()),
        AsdSyncOutcome::Accepted { .. }
    ));
    assert_eq!(
        sync_classify_response(401, vec![], vec![]),
        AsdSyncOutcome::Reauthenticate
    );
    assert_eq!(
        sync_classify_response(409, vec![], vec![]),
        AsdSyncOutcome::Duplicate
    );
    assert_eq!(
        sync_classify_response(503, vec![h("retry-after", "99999")], vec![]),
        AsdSyncOutcome::RetryAfter {
            seconds: Some(asd_sync::MAX_RETRY_AFTER_SECS)
        }
    );
    assert_eq!(
        sync_classify_response(429, vec![], vec![]),
        AsdSyncOutcome::RetryAfter { seconds: None }
    );
    assert_eq!(
        sync_classify_response(422, vec![], vec![]),
        AsdSyncOutcome::Fatal { status: 422 }
    );
}

#[test]
fn classify_register_response_covers_every_outcome() {
    let body = json!({
        "status": "registered",
        "device_id": "dev_1",
        "device_token": "dvc_x",
        "org_id": "org_1",
        "config_bundle_version": null
    });
    match sync_classify_register_response(201, s(&body).into_bytes()) {
        AsdRegisterOutcome::Registered {
            status,
            device_id,
            device_token,
            org_id,
            config_bundle_version,
        } => {
            assert_eq!(status, "registered");
            assert_eq!(device_id, "dev_1");
            assert_eq!(device_token, "dvc_x");
            assert_eq!(org_id, "org_1");
            assert_eq!(config_bundle_version, None);
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(
        sync_classify_register_response(410, vec![]),
        AsdRegisterOutcome::Rejected { status: 410 }
    );
    assert_eq!(
        sync_classify_register_response(200, b"{}".to_vec()),
        AsdRegisterOutcome::Invalid
    );
}

#[test]
fn url_rules_match_asd_sync() {
    let base = "https://api.example.org/";
    let t =
        sync_validate_upload_url("https://api.example.org/v1/blob/1".into(), base.into()).unwrap();
    assert!(t.attach_bearer);
    let t =
        sync_validate_upload_url("https://s3.example.com/b?X-Sig=1".into(), base.into()).unwrap();
    assert!(!t.attach_bearer);
    assert_eq!(t.url, "https://s3.example.com/b?X-Sig=1");
    for bad in [
        "http://s3.example.com/b",
        "https://127.0.0.1/b",
        "https://localhost/b",
    ] {
        assert!(
            matches!(
                sync_validate_upload_url(bad.into(), base.into()),
                Err(AsdError::InvalidUrl { .. })
            ),
            "{bad}"
        );
    }
    assert!(sync_validate_ingest_url(base.into()).is_ok());
    assert!(matches!(
        sync_validate_ingest_url("http://api.example.org".into()),
        Err(AsdError::InvalidUrl { .. })
    ));
    assert_eq!(
        sync_endpoint_url(base.into(), "v1/sessions".into()).unwrap(),
        asd_sync::endpoint_url(base, "v1/sessions").unwrap()
    );
}

#[test]
fn retry_schedule_matches_asd_sync() {
    for attempt in 0..10 {
        assert_eq!(
            sync_backoff_schedule(attempt),
            asd_sync::backoff_schedule(attempt)
        );
        for ra in [None, Some(0.5), Some(7200.0)] {
            assert_eq!(
                sync_next_retry_delay(attempt, ra),
                asd_sync::next_retry_delay(attempt, ra)
            );
        }
    }
    assert_eq!(sync_max_attempts(), asd_sync::MAX_ATTEMPTS);
}
