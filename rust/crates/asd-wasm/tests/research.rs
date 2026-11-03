//! owner: WP-X.4
//!
//! RPX-4: `paramRegistry`, `reprocessSession` and the honest `rescoreSession`, run twice with
//! the same source:
//! - natively (`cargo test -p asd-wasm`), and
//! - as wasm32 under Node (`scripts/build_asd_wasm.sh --test`, wasm-bindgen-test).
//!
//! Native == wasm32: `tests/golden/research_v1.json` holds, per case, the SHA-256 of the
//! export's `result` JSON bytes (plus a few readable fields) as the native build writes it.
//! Both targets must reproduce every hash, so the two give byte-identical JSON for every
//! synthetic tracking scenario at `frames` level and every synthetic session profile at
//! `score` level: `score`, `flag`, `qualityStatus`, `validRatio`, `metrics`, `fixations`,
//! `trace` and `warnings`. The one exception is the replay's `tracking` record (validation
//! and drift diagnostics in degrees): it goes through `atan2`/`acos`, which come from the
//! platform libm natively and from Rust's libm port on wasm32 and differ in the last bit, so
//! it is kept in full in the golden file and compared to a relative 1e-12. Making it
//! byte-identical needs asd-core to use one libm on every target (not this package).
//! Re-bless natively with `ASD_WASM_BLESS=1 cargo test -p asd-wasm --test research
//! bless_research_golden`.
//!
//! Fixtures are compiled in or generated (`synth_tracking`, `synth`): wasm tests have no
//! file system.

use asd_contract::config::ScoringProfile;
use asd_core::analysis::{evaluate, AnalysisProfile, LevelRequest, ResearchInput, SessionInput};
use asd_core::gaze::raw_log;
use asd_core::gaze::reprocess::ReprocessOptions;
use asd_core::synth::{generate_session, PROFILES};
use asd_core::synth_tracking::{generate, Scenario, SCENARIOS};
use asd_core::types::{NormativeBaselineSet, StimulusCatalog};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

const REGISTRY_FILE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../docs/spec/schemas/asd/param-registry-v1.json"
));
const CATALOG: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../asd-core/tests/fixtures/stimulus_catalog.json"
));
const BASELINES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../asd-core/tests/fixtures/normative_baselines.json"
));
const SESSION_V1_6: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../apps/ios_asd_gaze_app/Tests/Fixtures/expected-session-v1.6.json"
));
const GOLDEN: &str = include_str!("golden/research_v1.json");
const SEED: u64 = 7;
const OPTIONS: &str = r#"{"ageMonths":30,"scoredAtUtc":"2026-11-01T00:00:00Z"}"#;

/// The catalog clips the synthetic clips play as (`synth_clip_<k>` -> the k-th id), so the
/// AOIs come from the catalog (the header lists no clips) and the baselines apply.
const CLIP_IDS: [&str; 3] = [
    "stim_05_geopref_split_10s",
    "stim_01_social_peekaboo_8s",
    "stim_02_gaze_cue_right_8s",
];

/// A synthetic research frame log (`synth_tracking`, seed 7) as JSON lines, its clips renamed
/// to catalog clips.
fn synth_log(scenario: Scenario) -> String {
    let mut log = generate(scenario, SEED);
    for s in &mut log.samples {
        if let Some(id) = &s.stimulus_id {
            let k: usize = id
                .strip_prefix("synth_clip_")
                .and_then(|n| n.parse().ok())
                .expect("synth clip id");
            s.stimulus_id = Some(CLIP_IDS[(k - 1) % CLIP_IDS.len()].to_owned());
        }
    }
    raw_log::to_jsonl(&log).expect("log serialises")
}

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

fn profile_with_ratio(ratio: f64) -> String {
    let p = ScoringProfile {
        low_confidence_valid_ratio: ratio,
        ..ScoringProfile::default()
    };
    serde_json::to_string(&p).unwrap()
}

fn default_profile() -> String {
    serde_json::to_string(&ScoringProfile::default()).unwrap()
}

fn reprocess(log: &str, options: &str) -> String {
    asd_wasm::reprocess_session(log, CATALOG, &default_profile(), BASELINES, options)
}

fn low_confidence_clips(result: &Value) -> usize {
    result["score"]["videos"]
        .as_array()
        .expect("videos")
        .iter()
        .filter(|v| v["lowConfidence"] == true)
        .count()
}

// --- paramRegistry ---------------------------------------------------------------------------

#[test]
fn param_registry_equals_the_exported_file() {
    assert_eq!(ok(&asd_wasm::param_registry()), parse(REGISTRY_FILE));
}

// --- reprocessSession ------------------------------------------------------------------------

/// `asd-cli research reprocess` (RPX-5) prints `analysis::evaluate` at `frames` level over
/// the parsed log; the export must give the same bytes.
#[test]
fn reprocess_ideal_equals_the_core_evaluation() {
    let log = synth_log(Scenario::Ideal);
    for trace_hz in [None, Some(10)] {
        let options = match trace_hz {
            Some(hz) => format!(r#"{{"ageMonths":30,"traceHz":{hz}}}"#),
            None => r#"{"ageMonths":30}"#.to_owned(),
        };
        let out = ok(&reprocess(&log, &options));
        let profile: AnalysisProfile = serde_json::from_str(&default_profile()).unwrap();
        let baselines: NormativeBaselineSet = serde_json::from_str(BASELINES).unwrap();
        let catalog: StimulusCatalog = serde_json::from_str(CATALOG).unwrap();
        let input = SessionInput {
            age_months: Some(30),
            research: Some(ResearchInput {
                log: raw_log::parse(&log).unwrap(),
                catalog,
                options: ReprocessOptions {
                    trace_hz,
                    ..ReprocessOptions::default()
                },
            }),
            ..SessionInput::default()
        };
        let core = evaluate(&input, &profile, &baselines, LevelRequest::Frames);
        assert_eq!(
            out.to_string(),
            serde_json::to_value(&core).unwrap().to_string()
        );
        assert_eq!(out["level"], "frames");
        assert!(out["score"].is_object() && out["flag"].is_object());
        assert_eq!(out["trace"].is_array(), trace_hz.is_some());
    }
}

#[test]
fn reprocess_takes_the_profile_metrics_section() {
    let log = synth_log(Scenario::Ideal);
    let base = ok(&reprocess(&log, OPTIONS));
    let mut p = ScoringProfile::default();
    p.metrics.ivt_velocity_units_per_sec = 2.0;
    let tuned = ok(&asd_wasm::reprocess_session(
        &log,
        CATALOG,
        &serde_json::to_string(&p).unwrap(),
        BASELINES,
        OPTIONS,
    ));
    assert_ne!(base["metrics"], tuned["metrics"]);
    // Pure: the same inputs give the same bytes (RPX-6).
    assert_eq!(base.to_string(), ok(&reprocess(&log, OPTIONS)).to_string());
}

#[test]
fn reprocess_bad_input_is_an_envelope() {
    let log = synth_log(Scenario::Ideal);
    assert_eq!(err_kind(&reprocess("", OPTIONS)), "invalidArgument");
    assert_eq!(err_kind(&reprocess("{not json", OPTIONS)), "invalidJson");
    assert_eq!(
        err_kind(&reprocess(&log, r#"{"ageMonths":-1}"#)),
        "invalidArgument"
    );
    assert_eq!(
        err_kind(&reprocess(&log, r#"{"ageMonths":30,"trace":1}"#)),
        "invalidJson"
    );
    assert_eq!(
        err_kind(&asd_wasm::reprocess_session(
            &log,
            CATALOG,
            r#"{"flag":{}}"#,
            BASELINES,
            OPTIONS
        )),
        "invalidJson"
    );
    // No catalog: allowed (the header may carry the AOIs); clips without AOIs are warned.
    let out = ok(&asd_wasm::reprocess_session(
        &log,
        "null",
        &default_profile(),
        BASELINES,
        OPTIONS,
    ));
    assert_eq!(out["level"], "frames");
}

// --- rescoreSession: the profile's lowConfidenceValidRatio applies -----------------------------

#[test]
fn rescore_honours_low_confidence_valid_ratio() {
    // The golden 1.6 session with its first clip at a valid-sample ratio of 0.75: reliable
    // under 0.6, low-confidence under 0.9 (before WP-X.4 the 0.60 constant was used for both).
    let mut session = parse(SESSION_V1_6);
    session["rawPayload"]["perVideo"][0]["validSampleRatio"] = json!(0.75);
    let session = session.to_string();
    let run = |ratio: f64| {
        ok(&asd_wasm::rescore_session(
            &session,
            &profile_with_ratio(ratio),
            BASELINES,
            OPTIONS,
            None,
        ))
    };
    let (lenient, strict) = (run(0.6), run(0.9));
    assert_eq!(strict["metricSource"], "sessionReport");
    assert_eq!(lenient["score"]["videos"][0]["lowConfidence"], false);
    assert_eq!(strict["score"]["videos"][0]["lowConfidence"], true);
    assert!(low_confidence_clips(&strict) > low_confidence_clips(&lenient));
    // A behaviour fix, not an approximation: no warning about it.
    assert_eq!(strict["warnings"], lenient["warnings"]);
}

// --- native == wasm32 on the synthetic scenarios, both levels -----------------------------------

fn cases() -> Vec<(String, String)> {
    let mut out = Vec::new();
    for s in SCENARIOS {
        let log = synth_log(s);
        out.push((format!("frames/{}", s.name()), reprocess(&log, OPTIONS)));
    }
    let ideal = synth_log(Scenario::Ideal);
    out.push((
        "frames/ideal_trace_10hz".to_owned(),
        reprocess(&ideal, r#"{"ageMonths":30,"traceHz":10}"#),
    ));
    for (i, p) in PROFILES.into_iter().enumerate() {
        let session = generate_session(SEED, i as u64, Some(p)).to_string();
        for ratio in [0.6, 0.9] {
            out.push((
                format!("score/{p:?}/ratio_{ratio}").to_lowercase(),
                asd_wasm::rescore_session(
                    &session,
                    &profile_with_ratio(ratio),
                    BASELINES,
                    OPTIONS,
                    None,
                ),
            ));
        }
    }
    out
}

/// Per case: the SHA-256 of the result's JSON bytes without `tracking`, a few readable
/// fields, and `tracking` itself (frames level), which is compared to [`TRACKING_TOLERANCE`]
/// (see the module docs).
fn digest(envelope: &str) -> Value {
    let mut result = ok(envelope);
    let tracking = result
        .as_object_mut()
        .and_then(|m| m.remove("tracking"))
        .unwrap_or(Value::Null);
    let bytes = result.to_string();
    let sha: String = Sha256::digest(bytes.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    json!({
        "sha256": sha,
        "bytes": bytes.len(),
        "level": result.get("level").cloned().unwrap_or(json!("score")),
        "flag": result["flag"]["flag"],
        "compositeScore": result["score"]["compositeScore"],
        "tracking": tracking,
    })
}

/// Relative tolerance for the replay's tracking diagnostics across targets.
const TRACKING_TOLERANCE: f64 = 1e-12;

fn assert_close(actual: &Value, expected: &Value, path: &str) {
    match (actual, expected) {
        (Value::Number(a), Value::Number(e)) => {
            let (a, e) = (a.as_f64().unwrap(), e.as_f64().unwrap());
            let tol = TRACKING_TOLERANCE * e.abs().max(1.0);
            assert!((a - e).abs() <= tol, "{path}: {a} != {e}");
        }
        (Value::Object(a), Value::Object(e)) => {
            assert_eq!(
                a.keys().collect::<Vec<_>>(),
                e.keys().collect::<Vec<_>>(),
                "{path}: key sets differ"
            );
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

#[test]
fn synth_scenarios_match_the_blessed_native_bytes() {
    let golden = parse(GOLDEN);
    assert_eq!(golden["function"], "reprocess_session+rescore_session");
    let expected = golden["vectors"].as_object().expect("vectors object");
    let cases = cases();
    assert_eq!(
        expected.len(),
        cases.len(),
        "re-bless after changing cases()"
    );
    for (name, envelope) in &cases {
        let mut got = digest(envelope);
        let mut want = expected[name].clone();
        let (got_t, want_t) = (got["tracking"].take(), want["tracking"].take());
        assert_eq!(got, want, "{name}: bytes without tracking");
        assert_close(&got_t, &want_t, &format!("{name}.tracking"));
    }
}

/// Writes `tests/golden/research_v1.json` from the native build when `ASD_WASM_BLESS=1`.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn bless_research_golden() {
    if std::env::var("ASD_WASM_BLESS").as_deref() != Ok("1") {
        return;
    }
    let vectors: serde_json::Map<String, Value> = cases()
        .iter()
        .map(|(n, e)| (n.clone(), digest(e)))
        .collect();
    let doc = json!({
        "schema": "asd-golden/1",
        "function": "reprocess_session+rescore_session",
        "derivation": "Native asd-wasm output for tests/research.rs cases(): reprocess_session over \
                       every synth_tracking scenario (seed 7, clips renamed to catalog clips, \
                       asd-core fixture catalog and baselines, default profile, ageMonths 30) and rescore_session over one \
                       synth session per profile with lowConfidenceValidRatio 0.6 and 0.9. \
                       sha256 is over the result's JSON bytes without `tracking`, which the \
                       wasm build must reproduce exactly; `tracking` is compared to 1e-12 \
                       (platform libm, see tests/research.rs).",
        "vectors": vectors,
    });
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden/research_v1.json");
    std::fs::write(path, serde_json::to_string_pretty(&doc).unwrap() + "\n").unwrap();
}
