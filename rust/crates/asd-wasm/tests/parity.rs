//! owner: WP-R.6
//!
//! Parity of the wasm exports, run twice with the same source:
//! - natively (`cargo test -p asd-wasm`), and
//! - as wasm32 under Node (`scripts/build_asd_wasm.sh --test`, wasm-bindgen-test).
//!
//! Each target checks the exports against
//! - the Swift-oracle golden vectors of asd-core (`score_session`, `assess`, `extract`) to
//!   1e-9, and against a direct asd-core call;
//! - `tests/golden/rescore_v1.json`, the native re-score output of the repo's golden
//!   sessions. Native and wasm both matching that file is the native == wasm check.
//!   Re-bless natively with `ASD_WASM_BLESS=1 cargo test -p asd-wasm --test parity`.
//!
//! Fixtures are compiled in (`include_str!`): wasm tests have no file system.

use asd_contract::config::ScoringProfile;
use asd_core::types::{GazeFrame, NormativeBaselineSet, PerVideoMetricSet};
use asd_core::{flag, metrics, scoring};
use serde_json::{json, Value};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

const TOLERANCE: f64 = 1e-9;

macro_rules! core_golden {
    ($rel:literal) => {
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../asd-core/tests/golden/",
            $rel
        ))
    };
}

const SESSION_V1_6: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../apps/ios_asd_gaze_app/Tests/Fixtures/expected-session-v1.6.json"
));
const SESSION_V1_7: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../apps/ios_asd_gaze_app/Tests/Fixtures/expected-session-v1.7.json"
));
const BASELINES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../asd-core/tests/fixtures/normative_baselines.json"
));
const RESCORE_GOLDEN: &str = include_str!("golden/rescore_v1.json");

const EXTRACT_GOLDEN: [&str; 8] = [
    core_golden!("metrics/geopref_dwell_and_preference.json"),
    core_golden!("metrics/groups_by_stimulus_with_untagged.json"),
    core_golden!("metrics/first_entry_latency_earliest_hit.json"),
    core_golden!("metrics/fixation_split_by_saccade.json"),
    core_golden!("metrics/rich_affect.json"),
    core_golden!("metrics/media_time_preferred_over_session_time.json"),
    core_golden!("metrics/all_rejected_no_valid_frames.json"),
    core_golden!("metrics/empty_input.json"),
];

fn parse(text: &str) -> Value {
    serde_json::from_str(text).expect("JSON")
}

/// The `result` of a successful envelope.
fn ok(envelope: &str) -> Value {
    let v = parse(envelope);
    assert_eq!(v["ok"], true, "expected success: {envelope}");
    v["result"].clone()
}

/// The `error.kind` of a failed envelope.
fn err_kind(envelope: &str) -> String {
    let v = parse(envelope);
    assert_eq!(v["ok"], false, "expected failure: {envelope}");
    assert!(v["error"]["message"]
        .as_str()
        .is_some_and(|m| !m.is_empty()));
    v["error"]["kind"].as_str().expect("kind").to_owned()
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
            assert_eq!(a.len(), e.len(), "{path}: lengths differ");
            for (i, (av, ev)) in a.iter().zip(e).enumerate() {
                assert_close(av, ev, &format!("{path}[{i}]"));
            }
        }
        _ => assert_eq!(actual, expected, "{path}"),
    }
}

fn vectors(text: &str) -> Vec<(String, Value, Value)> {
    let doc = parse(text);
    doc["vectors"]
        .as_array()
        .expect("vectors")
        .iter()
        .map(|v| {
            (
                v["name"].as_str().expect("name").to_owned(),
                v["input"].clone(),
                v["expected"].clone(),
            )
        })
        .collect()
}

fn profile_json() -> String {
    serde_json::to_string(&ScoringProfile::default()).unwrap()
}

/// A stricter draft profile: the sandbox's typical "what if" input.
fn strict_profile_json() -> String {
    let mut p = ScoringProfile::default();
    p.flag.green_floor = 0.9;
    p.flag.yellow_floor = 0.8;
    p.flag.atypical_share_for_red = 0.2;
    serde_json::to_string(&p).unwrap()
}

// --- scoring primitives against the Swift oracle -----------------------------------------

#[test]
fn score_session_matches_swift_golden_and_core() {
    for (name, input, expected) in vectors(core_golden!("scoring/score_session_v1.json")) {
        let age = input["ageMonths"].as_i64().unwrap();
        let scored_at = input["scoredAtUtc"].as_str().unwrap();
        let out = ok(&asd_wasm::score_session(
            &input["metricSets"].to_string(),
            &input["baseline"].to_string(),
            i32::try_from(age).unwrap(),
            &input["frames"].to_string(),
            scored_at,
        ));
        assert_close(&out, &expected, &name);

        let sets: Vec<PerVideoMetricSet> =
            serde_json::from_value(input["metricSets"].clone()).expect("metric sets");
        let baseline: NormativeBaselineSet =
            serde_json::from_value(input["baseline"].clone()).expect("baseline");
        let frames: Vec<GazeFrame> =
            serde_json::from_value(input["frames"].clone()).expect("frames");
        let core = scoring::score_session(&sets, &baseline, age, &frames, scored_at);
        assert_eq!(out, serde_json::to_value(core).unwrap(), "{name}: core");
    }
}

#[test]
fn assess_flag_matches_swift_golden_and_core() {
    let profile = profile_json();
    for (name, input, expected) in vectors(core_golden!("flag/assess_v1.json")) {
        let out = ok(&asd_wasm::assess_flag(
            &input["score"].to_string(),
            &profile,
        ));
        assert_close(&out, &expected, &name);
        let score = serde_json::from_value(input["score"].clone()).expect("score");
        let core = flag::assess(&score, &flag::FlagThresholds::default());
        assert_eq!(out, serde_json::to_value(core).unwrap(), "{name}: core");
    }
}

#[test]
fn extract_metrics_matches_swift_golden_and_core() {
    for text in EXTRACT_GOLDEN {
        let doc = parse(text);
        let name = doc["case"].as_str().unwrap_or("extract").to_owned();
        assert_eq!(doc["function"], "extract", "{name}");
        let frames_json = doc["input"]["frames"].to_string();
        let out = ok(&asd_wasm::extract_metrics(&frames_json));
        assert_close(&out, &doc["expected"], &name);
        let frames: Vec<GazeFrame> = serde_json::from_str(&frames_json).unwrap();
        let core = metrics::extract(&frames);
        assert_eq!(out, serde_json::to_value(core).unwrap(), "{name}: core");
    }
}

// --- sandbox re-score on the golden sessions ---------------------------------------------

struct Case {
    name: &'static str,
    session: String,
    profile: String,
    baselines: String,
    options: Value,
    frames: Option<String>,
}

/// `rawPayload` alone, as the backend stores it in `sessions.summary`.
fn record_of(request: &str) -> String {
    parse(request)["rawPayload"].to_string()
}

fn cases() -> Vec<Case> {
    let frames_vector = vectors(core_golden!("scoring/score_session_v1.json"))
        .into_iter()
        .find(|(n, _, _)| n == "frames_grouped_by_stimulus_and_untagged")
        .expect("frames vector");
    let options = json!({"ageMonths": 28, "scoredAtUtc": "2026-11-01T00:00:00Z"});
    vec![
        Case {
            name: "v1_6_request_default_profile",
            session: SESSION_V1_6.into(),
            profile: profile_json(),
            baselines: BASELINES.into(),
            options: options.clone(),
            frames: None,
        },
        Case {
            name: "v1_7_request_default_profile",
            session: SESSION_V1_7.into(),
            profile: profile_json(),
            baselines: BASELINES.into(),
            options: options.clone(),
            frames: None,
        },
        Case {
            name: "v1_7_record_default_profile",
            session: record_of(SESSION_V1_7),
            profile: profile_json(),
            baselines: BASELINES.into(),
            options: options.clone(),
            frames: None,
        },
        Case {
            name: "v1_6_record_strict_profile_younger_bracket",
            session: record_of(SESSION_V1_6),
            profile: strict_profile_json(),
            baselines: BASELINES.into(),
            options: json!({"ageMonths": 14}),
            frames: None,
        },
        Case {
            name: "v1_7_request_with_frames",
            session: SESSION_V1_7.into(),
            profile: strict_profile_json(),
            baselines: frames_vector.1["baseline"].to_string(),
            options: json!({"ageMonths": frames_vector.1["ageMonths"]}),
            frames: Some(frames_vector.1["frames"].to_string()),
        },
    ]
}

fn run(case: &Case) -> String {
    asd_wasm::rescore_session(
        &case.session,
        &case.profile,
        &case.baselines,
        &case.options.to_string(),
        case.frames.clone(),
    )
}

#[test]
fn rescore_matches_blessed_native_output() {
    let golden = parse(RESCORE_GOLDEN);
    assert_eq!(golden["function"], "rescore_session");
    let expected = golden["vectors"].as_object().expect("vectors object");
    let cases = cases();
    assert_eq!(
        expected.len(),
        cases.len(),
        "re-bless after changing cases()"
    );
    for case in &cases {
        let out = ok(&run(case));
        assert_close(&out, &expected[case.name], case.name);
    }
}

/// Writes `tests/golden/rescore_v1.json` from the native build when `ASD_WASM_BLESS=1`.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn bless_rescore_golden() {
    if std::env::var("ASD_WASM_BLESS").as_deref() != Ok("1") {
        return;
    }
    let vectors: serde_json::Map<String, Value> = cases()
        .iter()
        .map(|c| (c.name.to_owned(), ok(&run(c))))
        .collect();
    let doc = json!({
        "schema": "asd-golden/1",
        "function": "rescore_session",
        "derivation": "Native asd-wasm::api::rescore_session output for the cases in \
                       tests/parity.rs (golden sessions of apps/ios_asd_gaze_app/Tests/Fixtures, \
                       asd-core fixture baselines). The wasm build must reproduce it.",
        "vectors": vectors,
    });
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden/rescore_v1.json");
    std::fs::write(path, serde_json::to_string_pretty(&doc).unwrap() + "\n").unwrap();
}

/// The console's form of a stored session: `SessionDetail` + `GET /sessions/{id}/clips`
/// items, built here from the golden request the way the backend stores it.
fn api_form_of(request: &str) -> (String, String) {
    let req = parse(request);
    let raw = &req["rawPayload"];
    let session = json!({
        "id": req["sessionId"],
        "schema_version": req["schemaVersion"],
        "composite_score": raw["perVideoComposite"],
        "attention_flag": req["attentionFlag"],
        "valid_frame_ratio": 0.8,
    });
    let clips: Vec<Value> = raw["perVideo"]
        .as_array()
        .expect("perVideo")
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let report = if v.get("report").is_some() {
                &v["report"]
            } else {
                v
            };
            let rows: Vec<Value> = report["rows"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| {
                    json!({
                        "segment_id": r["segmentId"], "metric": r["biomarker"],
                        "target_aoi": r.get("targetAOI").cloned().unwrap_or(Value::Null),
                        "direction": r["direction"], "expected": r.get("expected"),
                        "acceptable_low": r.get("acceptableLow"),
                        "acceptable_high": r.get("acceptableHigh"),
                        "recorded": r["observed"], "deviation": null, "status": r["status"],
                        "score": r["score"], "imputed": r["imputed"], "source": r["source"],
                    })
                })
                .collect();
            json!({"clip_id": report["stimulusId"], "order": i, "weight": report["weight"],
                   "score": report["subscore"], "rows": rows})
        })
        .collect();
    (session.to_string(), Value::Array(clips).to_string())
}

#[test]
fn api_session_rescore_scores_the_same_rows_as_the_stored_session() {
    for request in [SESSION_V1_6, SESSION_V1_7] {
        let (session, clips) = api_form_of(request);
        let options = r#"{"ageMonths":28,"scoredAtUtc":"2026-11-01T00:00:00Z"}"#;
        let api = ok(&asd_wasm::rescore_api_session(
            &session,
            &clips,
            &profile_json(),
            BASELINES,
            options,
        ));
        let stored = ok(&asd_wasm::rescore_session(
            request,
            &profile_json(),
            BASELINES,
            options,
            None,
        ));
        assert_eq!(api["metricSource"], "sessionReport");
        assert_eq!(api["sessionId"], stored["sessionId"]);
        assert_eq!(api["device"], stored["device"]);
        let videos = |v: &Value| -> Vec<(Value, Value)> {
            v["score"]["videos"]
                .as_array()
                .unwrap()
                .iter()
                .map(|x| (x["stimulusId"].clone(), x["rows"].clone()))
                .collect()
        };
        assert_eq!(videos(&api), videos(&stored));
        assert!(api["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w.as_str().unwrap().contains("valid frame ratio 0.8")));
    }

    let (session, clips) = api_form_of(SESSION_V1_6);
    let kind = |o: &str| {
        err_kind(&asd_wasm::rescore_api_session(
            &session,
            &clips,
            &profile_json(),
            BASELINES,
            o,
        ))
    };
    assert_eq!(kind(r#"{"ageMonths":-1}"#), "invalidArgument");
    assert_eq!(
        kind(r#"{"ageMonths":20,"validSampleRatio":1.5}"#),
        "invalidArgument"
    );
    assert_eq!(kind(r#"{"ageMonths":20,"typo":1}"#), "invalidJson");
}

#[test]
fn rescore_with_frames_is_the_device_path() {
    let case = cases().pop().expect("frames case");
    let out = ok(&run(&case));
    assert_eq!(out["metricSource"], "frames");
    let frames: Vec<GazeFrame> = serde_json::from_str(case.frames.as_deref().unwrap()).unwrap();
    let baseline: NormativeBaselineSet = serde_json::from_str(&case.baselines).unwrap();
    let age = case.options["ageMonths"].as_i64().unwrap();
    let core = scoring::score_session(&metrics::extract(&frames), &baseline, age, &frames, "");
    assert_eq!(out["score"], serde_json::to_value(&core).unwrap());
    let profile: ScoringProfile = serde_json::from_str(&case.profile).unwrap();
    let t = profile.flag;
    let thresholds = flag::FlagThresholds {
        green_floor: t.green_floor,
        yellow_floor: t.yellow_floor,
        low_confidence_share: t.low_confidence_share,
        atypical_share_for_red: t.atypical_share_for_red,
    };
    assert_eq!(
        out["flag"],
        serde_json::to_value(flag::assess(&core, &thresholds)).unwrap()
    );
}

#[test]
fn rescore_reports_the_session_and_device_values() {
    let out = ok(&run(&cases()[1]));
    assert_eq!(out["sessionId"], "SESS-GOLDEN-0001");
    assert_eq!(out["schemaVersion"], "1.7");
    assert_eq!(out["metricSource"], "sessionReport");
    assert_eq!(out["device"]["attentionFlag"], "yellow");
    assert_eq!(out["device"]["compositeScore"], 0.74);
    assert_eq!(out["score"]["ageMonths"], 28);
    assert_eq!(out["score"]["scoredAtUtc"], "2026-11-01T00:00:00Z");
    // A bare record has no request-level flag.
    let record = ok(&run(&cases()[2]));
    assert_eq!(record["device"]["attentionFlag"], Value::Null);
    assert_eq!(record["score"], out["score"]);
}

#[test]
fn rescore_profile_changes_only_the_flag() {
    let mut strict = cases().remove(1);
    let default = ok(&run(&strict));
    strict.profile = strict_profile_json();
    let out = ok(&run(&strict));
    assert_eq!(out["score"], default["score"]);
    assert_eq!(
        out["flag"],
        ok(&asd_wasm::assess_flag(
            &out["score"].to_string(),
            &strict.profile
        ))
    );
}

// --- errors are envelopes, never panics ----------------------------------------------------

#[test]
fn bad_input_is_a_structured_error() {
    let p = profile_json();
    let opts = r#"{"ageMonths":28}"#;
    let rescore = |s: &str, p: &str, b: &str, o: &str, f: Option<&str>| {
        asd_wasm::rescore_session(s, p, b, o, f.map(str::to_owned))
    };
    assert_eq!(
        err_kind(&rescore("", &p, BASELINES, opts, None)),
        "invalidJson"
    );
    assert_eq!(
        err_kind(&rescore("[]", &p, BASELINES, opts, None)),
        "invalidJson"
    );
    assert_eq!(
        err_kind(&rescore(
            r#"{"schemaVersion":"1.5","rawPayload":{}}"#,
            &p,
            BASELINES,
            opts,
            None
        )),
        "unsupportedSchemaVersion"
    );
    assert_eq!(
        err_kind(&rescore(
            r#"{"schemaVersion":"v2"}"#,
            &p,
            BASELINES,
            opts,
            None
        )),
        "unsupportedSchemaVersion"
    );
    assert_eq!(
        err_kind(&rescore(
            r#"{"schemaVersion":"1.7","rawPayload":{}}"#,
            &p,
            BASELINES,
            opts,
            None
        )),
        "invalidJson"
    );
    assert_eq!(
        err_kind(&rescore(SESSION_V1_7, "{}", BASELINES, opts, None)),
        "invalidJson"
    );
    assert_eq!(
        err_kind(&rescore(SESSION_V1_7, &p, "null", opts, None)),
        "invalidJson"
    );
    assert_eq!(
        err_kind(&rescore(SESSION_V1_7, &p, BASELINES, "{}", None)),
        "invalidJson"
    );
    assert_eq!(
        err_kind(&rescore(
            SESSION_V1_7,
            &p,
            BASELINES,
            r#"{"ageMonths":28,"age":1}"#,
            None
        )),
        "invalidJson"
    );
    assert_eq!(
        err_kind(&rescore(
            SESSION_V1_7,
            &p,
            BASELINES,
            r#"{"ageMonths":-1}"#,
            None
        )),
        "invalidArgument"
    );
    assert_eq!(
        err_kind(&rescore(SESSION_V1_7, &p, BASELINES, opts, Some("{"))),
        "invalidJson"
    );
    assert_eq!(
        err_kind(&asd_wasm::score_session("x", BASELINES, 1, "[]", "")),
        "invalidJson"
    );
    assert_eq!(err_kind(&asd_wasm::assess_flag("{}", &p)), "invalidJson");
    assert_eq!(err_kind(&asd_wasm::extract_metrics("{}")), "invalidJson");
    assert_eq!(
        err_kind(&asd_wasm::validate_baselines(BASELINES, Some("{".into()))),
        "invalidJson"
    );
}

#[test]
fn hostile_numbers_do_not_panic() {
    // Non-finite floats cannot be written in JSON; huge ones and odd shapes still score.
    let sets = r#"[{"stimulusId":"stim_05_geopref_split_10s","clipFrameCount":0,
        "validFrameCount":0,"validSampleRatio":1e308,"observedDurationSec":-1e308,
        "aoiDwellRatios":{},"metrics":{"social_dwell_ratio":1e308}}]"#;
    ok(&asd_wasm::score_session(
        sets,
        BASELINES,
        i32::MIN,
        "[]",
        "",
    ));
    ok(&asd_wasm::score_session(
        sets,
        BASELINES,
        i32::MAX,
        "[]",
        "",
    ));
    let empty =
        r#"{"schemaVersion":"x","status":"x","provenance":"x","ageBrackets":[],"videos":{}}"#;
    ok(&asd_wasm::rescore_session(
        SESSION_V1_6,
        &profile_json(),
        empty,
        r#"{"ageMonths":9223372036854775807}"#,
        Some("[]".into()),
    ));
}

// --- version, defaults and document checks ----------------------------------------------------

#[test]
fn contract_info_and_default_profile() {
    let info = ok(&asd_wasm::contract_info());
    assert_eq!(info["coreVersion"], env!("CARGO_PKG_VERSION"));
    assert_eq!(info["sessionSchemaVersions"], json!(["1.6", "1.7", "1.8"]));
    assert_eq!(
        info["scoringProfileSchemaVersion"],
        asd_contract::config::SCORING_PROFILE_SCHEMA_VERSION
    );
    assert_eq!(info["lowConfidenceValidRatio"], 0.6);
    let profile = ok(&asd_wasm::default_scoring_profile());
    assert_eq!(
        profile,
        serde_json::to_value(ScoringProfile::default()).unwrap()
    );
}

#[test]
fn validation_reports_problems_without_failing() {
    let good = ok(&asd_wasm::validate_scoring_profile(&profile_json()));
    assert_eq!(
        good,
        json!({"valid": true, "problems": [], "fieldProblems": []})
    );
    let bad = ok(&asd_wasm::validate_scoring_profile(r#"{"flag":{}}"#));
    assert_eq!(bad["valid"], false);
    assert_eq!(bad["problems"].as_array().unwrap().len(), 1);
    assert_eq!(bad["fieldProblems"], json!([]));

    // Range checks: the same rules and messages as the backend (asd-contract profile_checks).
    let mut out_of_range = ScoringProfile::default();
    out_of_range.flag.green_floor = 1.5;
    out_of_range.quality_gate.min_distance_cm = 80.0;
    let ranged = ok(&asd_wasm::validate_scoring_profile(
        &serde_json::to_string(&out_of_range).unwrap(),
    ));
    assert_eq!(ranged["valid"], false);
    assert_eq!(
        ranged["fieldProblems"],
        json!([
            {"path": "flag.greenFloor", "message": "flag.greenFloor must be in [0, 1]"},
            {"path": "qualityGate.maxDistanceCm",
             "message": "qualityGate needs 0 < minDistanceCm < maxDistanceCm"}
        ])
    );
    assert_eq!(
        ranged["problems"],
        json!(asd_contract::profile_checks::scoring_profile_problem_messages(&out_of_range))
    );

    assert_eq!(
        ok(&asd_wasm::validate_baselines(BASELINES, None))["valid"],
        true
    );
    let missing = ok(&asd_wasm::validate_baselines(
        BASELINES,
        Some(r#"["stim_99_unknown"]"#.into()),
    ));
    assert_eq!(
        missing["problems"],
        json!(["missing video profile: stim_99_unknown"])
    );
    let not_a_set = ok(&asd_wasm::validate_baselines("[]", None));
    assert_eq!(not_a_set["valid"], false);
}

#[test]
fn validate_catalog_is_the_backend_check() {
    let raw = include_str!("../../asd-backend/seed/stimulus_catalog.json");
    assert_eq!(
        ok(&asd_wasm::validate_catalog(raw)),
        json!({"valid": true, "problems": [], "fieldProblems": []})
    );

    let mut bad: serde_json::Value = serde_json::from_str(raw).unwrap();
    bad["clips"][2]["areas"][0]["xMin"] = json!(1.4);
    bad["fallbackLanguage"] = json!("fr");
    let r = ok(&asd_wasm::validate_catalog(&bad.to_string()));
    assert_eq!(r["valid"], false);
    let paths: Vec<&str> = r["fieldProblems"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["path"].as_str().unwrap())
        .collect();
    assert_eq!(
        paths,
        [
            "fallbackLanguage",
            "clips[2].areas[0].xMin",
            "clips[2].areas[0].xMax"
        ]
    );
    // The same list as the one the backend and asd-contract produce.
    let catalog: asd_core::types::StimulusCatalog = serde_json::from_value(bad).unwrap();
    assert_eq!(
        r["problems"],
        json!(asd_contract::catalog_checks::catalog_problem_messages(
            &catalog
        ))
    );

    // A shape failure is a problem, not an error envelope.
    let shape = ok(&asd_wasm::validate_catalog(r#"{"clips":[]}"#));
    assert_eq!(shape["valid"], false);
    assert_eq!(shape["problems"].as_array().unwrap().len(), 1);
    assert_eq!(shape["fieldProblems"], json!([]));
}

/// Round trip: a session carrying the Swift-scored reports of a golden vector re-scores to
/// the same reports with the same baseline and age (vectors without frames: segment windows
/// need frames).
#[test]
fn rescore_from_reports_reproduces_the_device_score() {
    let mut checked = 0;
    for (name, input, expected) in vectors(core_golden!("scoring/score_session_v1.json")) {
        if !input["frames"].as_array().unwrap().is_empty() {
            continue;
        }
        let mut record = parse(SESSION_V1_6)["rawPayload"].clone();
        record["perVideo"] = expected["videos"].clone();
        record["perVideoComposite"] = expected["compositeScore"].clone();
        let options = json!({"ageMonths": input["ageMonths"], "scoredAtUtc": input["scoredAtUtc"]});
        let out = ok(&asd_wasm::rescore_session(
            &record.to_string(),
            &profile_json(),
            &input["baseline"].to_string(),
            &options.to_string(),
            None,
        ));
        assert_close(&out["score"], &expected, &name);
        checked += 1;
    }
    assert!(checked >= 5, "only {checked} vectors without frames");
}

/// 1.8 keeps the 1.7 per-clip report, so a 1.8 copy of the 1.7 fixture re-scores the same,
/// as a request and as the bare record the backend stores.
#[test]
fn rescore_reads_v1_8_requests_and_records() {
    let mut req = parse(SESSION_V1_7);
    req["schemaVersion"] = json!("1.8");
    req["scoringStatus"] = json!("scored");
    req["rawPayload"]["schemaVersion"] = json!("v1.8");
    req["rawPayload"]["scoringStatus"] = json!("scored");
    let base = cases();
    let v17 = ok(&run(&base[1]));
    for session in [req.to_string(), record_of(&req.to_string())] {
        let case = Case {
            session,
            profile: base[1].profile.clone(),
            baselines: base[1].baselines.clone(),
            options: base[1].options.clone(),
            frames: None,
            name: "v1_8",
        };
        let out = ok(&run(&case));
        assert_eq!(out["schemaVersion"], "1.8");
        assert_eq!(out["score"], v17["score"]);
        assert_eq!(out["flag"], v17["flag"]);
    }
}
