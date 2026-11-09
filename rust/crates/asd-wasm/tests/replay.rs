//! owner: WP-X.19
//!
//! The replay exports (`replayEvents`, `replayRibbon`, `replayTrace`, `expectedBands`,
//! `replayHeatmap`, `heatmapCohort`, `heatmapDifference`, `outlierScreen`), run twice with
//! the same source:
//! - natively (`cargo test -p asd-wasm`), and
//! - as wasm32 under Node (`scripts/build_asd_wasm.sh --test`, wasm-bindgen-test).
//!
//! Native == wasm32: `tests/golden/replay_v1.json` holds the native result of every case;
//! both targets must reproduce it to 1e-9 (the KDE goes through `exp`, which comes from the
//! platform libm natively and from Rust's libm port on wasm32, so bytes may differ in the last
//! bit; the grid is `f32` anyway). Re-bless natively with
//! `ASD_WASM_BLESS=1 cargo test -p asd-wasm --test replay bless_replay_golden`.
//!
//! Inputs are generated here (scripted gaze over two catalog clips, deterministic jitter) or
//! compiled in: wasm tests have no file system.

use asd_contract::config::ScoringProfile;
use asd_core::analysis::AnalysisProfile;
use asd_core::replay::{self, EventParams};
use asd_core::types::{
    Clip, GazeFrame, GazeFrameAffect, GazeFrameSample, GazeQualityVerdict, StimulusCatalog,
};
use serde_json::{json, Value};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

const TOLERANCE: f64 = 1e-9;

const CATALOG: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../asd-core/tests/fixtures/stimulus_catalog.json"
));
const BASELINES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../asd-core/tests/fixtures/normative_baselines.json"
));
const GOLDEN: &str = include_str!("golden/replay_v1.json");

const CUE: &str = "stim_02_gaze_cue_right_8s";
const GEO: &str = "stim_05_geopref_split_10s";

fn parse(text: &str) -> Value {
    serde_json::from_str(text).expect("JSON")
}

fn ok(envelope: &str) -> Value {
    let v = parse(envelope);
    assert_eq!(v["ok"], true, "expected success: {envelope}");
    v["result"].clone()
}

fn err_kind(envelope: &str) -> String {
    let v = parse(envelope);
    assert_eq!(v["ok"], false, "expected failure: {envelope}");
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

// --- inputs ----------------------------------------------------------------------------------

fn clip(id: &str) -> Clip {
    let catalog: StimulusCatalog = serde_json::from_str(CATALOG).unwrap();
    catalog.clips.into_iter().find(|c| c.id == id).unwrap()
}

fn clip_json(id: &str) -> String {
    serde_json::to_string(&clip(id)).unwrap()
}

/// Deterministic jitter in [-0.002, 0.002) (64-bit LCG, no libm).
struct Jitter(u64);

impl Jitter {
    fn next(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((self.0 >> 11) as f64 / (1u64 << 53) as f64 - 0.5) * 0.004
    }
}

/// `n` frames at 60 Hz of clip `id`: the gaze holds at each `(first_frame, x, y)` and moves
/// there from the previous hold over 4 frames; frames in `invalid` ranges are invalid.
fn scripted(
    id: &str,
    n: usize,
    holds: &[(usize, f64, f64)],
    invalid: &[(usize, usize)],
    seed: u64,
) -> Vec<GazeFrame> {
    let mut jitter = Jitter(seed);
    (0..n)
        .map(|i| {
            let k = holds.iter().rposition(|h| h.0 <= i).unwrap();
            let (start, x, y) = holds[k];
            let (mut x, mut y) = (x, y);
            if k > 0 && i - start < 4 {
                let a = (i - start + 1) as f64 / 4.0;
                let (_, px, py) = holds[k - 1];
                x = px + (x - px) * a;
                y = py + (y - py) * a;
            }
            let bad = invalid.iter().any(|&(a, b)| (a..b).contains(&i));
            let t = i as f64 / 60.0;
            GazeFrame {
                gaze: GazeFrameSample {
                    is_valid: !bad,
                    x_norm: x + jitter.next(),
                    y_norm: y + jitter.next(),
                    video_time_sec: 100.0 + t,
                    clip_media_time_sec: Some(t),
                },
                affect: GazeFrameAffect::default(),
                active_aoi_names: Vec::new(),
                quality_verdict: if bad {
                    GazeQualityVerdict::Rejected
                } else {
                    GazeQualityVerdict::Optimal
                },
                stimulus_id: Some(id.to_owned()),
            }
        })
        .collect()
}

/// Joint-attention clip: face, a look away, a blink, back to the face, the cued target, a
/// track loss, the target again.
fn cue_frames() -> Vec<GazeFrame> {
    scripted(
        CUE,
        480,
        &[
            (0, 0.5, 0.4),
            (90, 0.2, 0.8),
            (150, 0.5, 0.45),
            (216, 0.8, 0.25),
            (408, 0.82, 0.3),
        ],
        &[(120, 132), (360, 408)],
        11,
    )
}

/// Preference clip: social and geometric halves alternating every 1.2 s, one dropout frame.
fn geo_frames(seed: u64) -> Vec<GazeFrame> {
    let holds: Vec<(usize, f64, f64)> = (0..7)
        .map(|k| (k * 72, if k % 2 == 0 { 0.25 } else { 0.75 }, 0.5))
        .collect();
    scripted(GEO, 480, &holds, &[(300, 301)], seed)
}

fn session_frames() -> Vec<GazeFrame> {
    let mut f = geo_frames(3);
    f.extend(cue_frames());
    f
}

fn json<T: serde::Serialize + ?Sized>(v: &T) -> String {
    serde_json::to_string(v).unwrap()
}

fn profile_json() -> String {
    json(&ScoringProfile::default())
}

fn idt_profile_json() -> String {
    let mut p = serde_json::to_value(ScoringProfile::default()).unwrap();
    p["metrics"]["fixationAlgorithm"] = json!("idt");
    p["metrics"]["idtDispersionNorm"] = json!(0.03);
    p.to_string()
}

fn rows_json() -> String {
    let rows: Vec<Value> = (0..14)
        .map(|i| {
            json!({
                "sessionId": format!("s{i:02}"),
                "metrics": {
                    "social_dwell_ratio": if i == 13 { 0.02 } else { 0.4 + 0.02 * i as f64 },
                    "first_entry_latency_ms": 300.0 + 25.0 * i as f64,
                },
                "validRatio": if i == 2 { 0.4 } else { 0.9 },
                "clipsCompleted": if i == 5 { 3 } else { 10 },
                "qualityVerdict": if i == 7 { "degraded" } else { "optimal" },
            })
        })
        .collect();
    json(&rows)
}

const RULES: &str = r#"[
  {"kind":"minValidRatio","id":"q.valid","min":0.6},
  {"kind":"minClipsCompleted","id":"q.clips","min":5},
  {"kind":"minQualityVerdict","id":"q.verdict","min":"acceptable"},
  {"kind":"explicit","id":"manual","sessionIds":["s09"]},
  {"kind":"tukey","id":"tukey.social","metric":"social_dwell_ratio"},
  {"kind":"mad","id":"mad.latency","metric":"first_entry_latency_ms","z":1.0}
]"#;

// --- cases -----------------------------------------------------------------------------------

fn cases() -> Vec<(&'static str, String)> {
    let frames = json(&session_frames());
    let cue = json(&cue_frames());
    let grids: Vec<Value> = (1..=12)
        .map(|s| {
            ok(&asd_wasm::replay_heatmap(
                &json(&geo_frames(s)),
                Some(GEO.into()),
                16,
                9,
                0.08,
                None,
                None,
            ))
        })
        .collect();
    let grids_json = json(&grids);
    vec![
        (
            "events_ivt",
            asd_wasm::replay_events(&frames, &profile_json()),
        ),
        (
            "events_idt",
            asd_wasm::replay_events(&frames, &idt_profile_json()),
        ),
        (
            "ribbon_cue",
            asd_wasm::replay_ribbon(&frames, &clip_json(CUE), None),
        ),
        (
            "ribbon_geo_margin",
            asd_wasm::replay_ribbon(
                &frames,
                &clip_json(GEO),
                Some(json(&{
                    let mut p = ScoringProfile::default();
                    p.aoi.margin_norm = 0.05;
                    p
                })),
            ),
        ),
        (
            "trace_face_age30",
            asd_wasm::replay_trace(
                &cue,
                &clip_json(CUE),
                BASELINES,
                &profile_json(),
                r#"{"kind":"aoiDwell","aoi":"Actor_Face"}"#,
                1.0,
                0.25,
                Some(30),
            ),
        ),
        (
            "trace_social_role",
            asd_wasm::replay_trace(
                &frames,
                &clip_json(GEO),
                BASELINES,
                &profile_json(),
                r#"{"kind":"roleDwell","role":"social"}"#,
                1.0,
                0.5,
                None,
            ),
        ),
        (
            "trace_valid_age14",
            asd_wasm::replay_trace(
                &cue,
                &clip_json(CUE),
                BASELINES,
                &profile_json(),
                r#"{"kind":"validRatio"}"#,
                2.0,
                1.0,
                Some(14),
            ),
        ),
        (
            "bands_latency_age14",
            asd_wasm::expected_bands(
                BASELINES,
                "stim_01_social_peekaboo_8s",
                14,
                "first_entry_latency_ms",
                Some("Eye_Region".into()),
            ),
        ),
        (
            "heatmap_cue",
            asd_wasm::replay_heatmap(&frames, Some(CUE.into()), 32, 18, 0.05, None, None),
        ),
        (
            "heatmap_cue_window",
            asd_wasm::replay_heatmap(&frames, Some(CUE.into()), 16, 9, 0.1, Some(3.6), Some(6.0)),
        ),
        ("cohort_12", asd_wasm::heatmap_cohort(&grids_json, None)),
        (
            "cohort_suppressed",
            asd_wasm::heatmap_cohort(&json(&grids[..11]), None),
        ),
        (
            "difference",
            asd_wasm::heatmap_difference(&json(&grids[0]), &json(&grids[1])),
        ),
        ("outliers", asd_wasm::outlier_screen(&rows_json(), RULES)),
    ]
}

#[test]
fn replay_matches_blessed_native_output() {
    let golden = parse(GOLDEN);
    assert_eq!(golden["function"], "replay");
    let expected = golden["vectors"].as_object().expect("vectors object");
    let cases = cases();
    assert_eq!(
        expected.len(),
        cases.len(),
        "re-bless after changing cases()"
    );
    for (name, out) in &cases {
        assert_close(&ok(out), &expected[*name], name);
    }
}

/// Writes `tests/golden/replay_v1.json` from the native build when `ASD_WASM_BLESS=1`.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn bless_replay_golden() {
    if std::env::var("ASD_WASM_BLESS").as_deref() != Ok("1") {
        return;
    }
    let vectors: serde_json::Map<String, Value> = cases()
        .iter()
        .map(|(name, out)| ((*name).to_owned(), ok(out)))
        .collect();
    let doc = json!({
        "schema": "asd-golden/1",
        "function": "replay",
        "derivation": "Native asd-wasm replay exports for the cases in tests/replay.rs \
                       (scripted 60 Hz gaze over two catalog clips of asd-core's fixture \
                       catalog, fixture baselines). The wasm build must reproduce it to 1e-9.",
        "vectors": vectors,
    });
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden/replay_v1.json");
    std::fs::write(path, serde_json::to_string_pretty(&doc).unwrap() + "\n").unwrap();
}

// --- the exports are the core ----------------------------------------------------------------

#[test]
fn exports_equal_the_core_calls() {
    let frames = session_frames();
    let out = ok(&asd_wasm::replay_events(&json(&frames), &profile_json()));
    let core = replay::events_by_clip(&frames, &EventParams::from(&AnalysisProfile::default()));
    assert_eq!(out["clips"], serde_json::to_value(&core).unwrap());
    assert_eq!(out["clips"][0]["stimulusId"], GEO);
    assert_eq!(out["clips"][1]["stimulusId"], CUE);
    assert_eq!(out["warnings"], json!([]));

    // The I-DT profile is honoured and differs from I-VT.
    let idt = ok(&asd_wasm::replay_events(
        &json(&frames),
        &idt_profile_json(),
    ));
    assert_eq!(idt["params"]["algorithm"], "idt");
    assert_ne!(idt["clips"], out["clips"]);

    let cue = replay::clip_frames(&frames, CUE);
    let ribbon = ok(&asd_wasm::replay_ribbon(
        &json(&frames),
        &clip_json(CUE),
        None,
    ));
    let core = replay::aoi_ribbon(&cue, &clip(CUE).areas, &replay::HitParams::default());
    assert_eq!(ribbon["segments"], serde_json::to_value(&core).unwrap());
}

#[test]
fn replay_reads_the_script() {
    let frames = json(&session_frames());
    // The cue clip's events: the blink (frames 120..132) and the loss (360..408) are there.
    let out = ok(&asd_wasm::replay_events(&frames, &profile_json()));
    let kinds: Vec<&str> = out["clips"][1]["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["kind"].as_str().unwrap())
        .collect();
    assert!(
        kinds.contains(&"blink") && kinds.contains(&"lost"),
        "{kinds:?}"
    );
    assert!(
        kinds.iter().filter(|k| **k == "saccade").count() >= 3,
        "{kinds:?}"
    );

    // The ribbon starts on the face and ends on the cued target.
    let ribbon = ok(&asd_wasm::replay_ribbon(&frames, &clip_json(CUE), None));
    let seg = ribbon["segments"].as_array().unwrap();
    assert_eq!(seg[0]["aoi"], "Actor_Face");
    assert_eq!(seg[0]["roles"], json!(["social", "orienting"]));
    assert_eq!(seg.last().unwrap()["aoi"], "Gaze_Target_Right");

    // The face trace has the published band for 24-35 months (default range).
    let tr = ok(&asd_wasm::replay_trace(
        &frames,
        &clip_json(CUE),
        BASELINES,
        &profile_json(),
        r#"{"kind":"aoiDwell","aoi":"Actor_Face"}"#,
        1.0,
        0.25,
        Some(30),
    ));
    assert_eq!(tr["bracketId"], "24-35");
    assert_eq!(tr["bands"][0]["segmentId"], "cue_face");
    assert_eq!(tr["bands"][0]["mean"], 0.35);
    assert_eq!(tr["trace"]["points"][0]["value"], 1.0);

    // Screening: the quality gates, the manual exclusion, then Tukey catches s13.
    let screen = ok(&asd_wasm::outlier_screen(&rows_json(), RULES));
    let ids: Vec<&str> = screen["excluded"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["sessionId"].as_str().unwrap())
        .collect();
    assert_eq!(&ids[..5], ["s02", "s05", "s07", "s09", "s13"]);
}

#[test]
fn bad_input_is_a_structured_error() {
    let frames = json(&session_frames());
    assert_eq!(
        err_kind(&asd_wasm::replay_events("[", &profile_json())),
        "invalidJson"
    );
    assert_eq!(
        err_kind(&asd_wasm::replay_events(&frames, "{")),
        "invalidJson"
    );
    assert_eq!(
        err_kind(&asd_wasm::replay_ribbon(&frames, "{}", None)),
        "invalidJson"
    );
    let trace = |metric: &str, w: f64, s: f64, age: Option<i32>| {
        asd_wasm::replay_trace(
            &frames,
            &clip_json(CUE),
            BASELINES,
            &profile_json(),
            metric,
            w,
            s,
            age,
        )
    };
    assert_eq!(
        err_kind(&trace(r#"{"kind":"nope"}"#, 1.0, 1.0, None)),
        "invalidJson"
    );
    assert_eq!(
        err_kind(&trace(r#"{"kind":"anyAoi"}"#, 0.0, 1.0, None)),
        "invalidArgument"
    );
    assert_eq!(
        err_kind(&trace(r#"{"kind":"anyAoi"}"#, 1.0, f64::NAN, None)),
        "invalidArgument"
    );
    assert_eq!(
        err_kind(&trace(r#"{"kind":"anyAoi"}"#, 1.0, 1.0, Some(-1))),
        "invalidArgument"
    );
    assert_eq!(
        err_kind(&asd_wasm::replay_heatmap(
            &frames, None, 0, 9, 0.1, None, None
        )),
        "invalidArgument"
    );
    assert_eq!(
        err_kind(&asd_wasm::replay_heatmap(
            &frames, None, 9, 9, -1.0, None, None
        )),
        "invalidArgument"
    );
    assert_eq!(
        err_kind(&asd_wasm::replay_heatmap(
            &frames,
            None,
            9,
            9,
            0.1,
            Some(1.0),
            None
        )),
        "invalidArgument"
    );
    let g = |w: u32| {
        json!({"w": w, "h": 1, "cells": vec![0.0; w as usize], "nSamples": 0, "nGrids": 1})
            .to_string()
    };
    assert_eq!(
        err_kind(&asd_wasm::heatmap_difference(&g(2), &g(3))),
        "invalidArgument"
    );
    assert_eq!(
        err_kind(&asd_wasm::outlier_screen(
            &rows_json(),
            r#"[{"kind":"mad","id":"x","metric":"m","z":0}]"#
        )),
        "invalidArgument"
    );
    assert_eq!(
        err_kind(&asd_wasm::outlier_screen("{}", "[]")),
        "invalidJson"
    );
}
