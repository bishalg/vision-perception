//! owner: WP-X.10
//!
//! `asd-cli research reprocess | evaluate` through the real binary (RPX-5): exit codes, the
//! `--level frames` refusal on a log without clips, and output byte-equal to the core's
//! `evaluate` on the asd-core test fixtures.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use asd_core::analysis::{evaluate, AnalysisProfile, LevelRequest, ResearchInput, SessionInput};
use asd_core::gaze::aoi_space::{NormBox, ScreenAoi};
use asd_core::gaze::raw_log::{self, RawLog};
use asd_core::gaze::reprocess::ReprocessOptions;
use asd_core::synth_tracking::{generate, Scenario};
use asd_core::types::tracking::RawLogClip;
use asd_core::types::{NormativeBaselineSet, StimulusCatalog};
use serde_json::Value;

const CORE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../asd-core/tests/fixtures");
const SESSION: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../apps/ios_asd_gaze_app/Tests/Fixtures/expected-session-v1.7.json"
);
const SCORED_AT: &str = "2026-11-01T00:00:00Z";

fn cli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_asd-cli"))
        .args(args)
        .output()
        .expect("run asd-cli")
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("asd-cli-research-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

fn p(path: &Path) -> &str {
    path.to_str().unwrap()
}

fn baselines_path() -> String {
    format!("{CORE}/normative_baselines.json")
}

fn catalog_path() -> String {
    format!("{CORE}/stimulus_catalog.json")
}

fn with_clips(mut log: RawLog) -> RawLog {
    let mut ids: Vec<String> = Vec::new();
    for s in &log.samples {
        if let Some(id) = &s.stimulus_id {
            if !ids.contains(id) {
                ids.push(id.clone());
            }
        }
    }
    let aois = vec![ScreenAoi {
        name: "Social_Actor".to_owned(),
        start_sec: 0.0,
        end_sec: 60.0,
        rect: NormBox {
            x_min: 0.0,
            y_min: 0.0,
            x_max: 0.5,
            y_max: 1.0,
        },
        roles: Vec::new(),
    }];
    log.header.clips = ids
        .iter()
        .enumerate()
        .map(|(order, id)| RawLogClip {
            clip_id: id.clone(),
            order: order as u32,
            catalog_version: None,
            aois: aois.clone(),
            video_aspect: None,
        })
        .collect();
    log
}

fn core_baselines() -> NormativeBaselineSet {
    serde_json::from_str(&fs::read_to_string(baselines_path()).unwrap()).unwrap()
}

#[test]
fn reprocess_output_equals_the_core_evaluation_byte_for_byte() {
    let dir = scratch("reprocess");
    let log = with_clips(generate(Scenario::Ideal, 7));
    let path = dir.join("log.jsonl");
    fs::write(&path, raw_log::to_jsonl(&log).unwrap()).unwrap();

    let o = cli(&[
        "research",
        "reprocess",
        p(&path),
        "--catalog",
        &catalog_path(),
        "--baselines",
        &baselines_path(),
        "--age-months",
        "28",
        "--level",
        "frames",
        "--trace-hz",
        "10",
        "--scored-at",
        SCORED_AT,
    ]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));

    let catalog: StimulusCatalog =
        serde_json::from_str(&fs::read_to_string(catalog_path()).unwrap()).unwrap();
    let parsed = raw_log::parse(&fs::read_to_string(&path).unwrap()).unwrap();
    let session = SessionInput {
        age_months: Some(28),
        scored_at_utc: SCORED_AT.to_owned(),
        research: Some(ResearchInput {
            log: parsed,
            catalog,
            options: ReprocessOptions {
                trace_hz: Some(10),
                ..ReprocessOptions::default()
            },
        }),
        ..SessionInput::default()
    };
    let expected = evaluate(
        &session,
        &AnalysisProfile::default(),
        &core_baselines(),
        LevelRequest::Frames,
    );
    let expected = format!("{}\n", serde_json::to_string_pretty(&expected).unwrap());
    assert_eq!(stdout(&o), expected);
    let v: Value = serde_json::from_str(&stdout(&o)).unwrap();
    assert_eq!(v["level"], "frames");
    assert!(v["trace"].is_array());
}

#[test]
fn frames_level_on_a_log_without_clips_exits_2_with_a_message() {
    let dir = scratch("noclips");
    let path = dir.join("log.jsonl");
    let log = generate(Scenario::Ideal, 7);
    assert!(log.header.clips.is_empty());
    fs::write(&path, raw_log::to_jsonl(&log).unwrap()).unwrap();
    let o = cli(&[
        "research",
        "reprocess",
        p(&path),
        "--catalog",
        &catalog_path(),
        "--baselines",
        &baselines_path(),
        "--age-months",
        "28",
        "--level",
        "frames",
    ]);
    assert_eq!(o.status.code(), Some(2));
    assert!(stderr(&o).contains("has no clips"), "{}", stderr(&o));
    assert!(stdout(&o).is_empty());
}

#[test]
fn evaluate_output_equals_the_core_score_level_evaluation() {
    let o = cli(&[
        "research",
        "evaluate",
        SESSION,
        "--baselines",
        &baselines_path(),
        "--age-months",
        "28",
        "--scored-at",
        SCORED_AT,
    ]);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));

    let doc: Value = serde_json::from_str(&fs::read_to_string(SESSION).unwrap()).unwrap();
    let raw = &doc["rawPayload"];
    let session = SessionInput {
        session_id: raw["sessionId"].as_str().unwrap_or("").to_owned(),
        age_months: Some(28),
        reports: serde_json::from_value(raw["perVideo"].clone()).unwrap(),
        has_reports: raw.get("perVideo").is_some(),
        valid_frame_ratio: raw["validFrameRatio"].as_f64(),
        tracking: serde_json::from_value(raw["trackingQuality"].clone()).ok(),
        scored_at_utc: SCORED_AT.to_owned(),
        research: None,
    };
    let expected = evaluate(
        &session,
        &AnalysisProfile::default(),
        &core_baselines(),
        LevelRequest::Score,
    );
    assert_eq!(
        stdout(&o),
        format!("{}\n", serde_json::to_string_pretty(&expected).unwrap())
    );
    assert_eq!(
        serde_json::from_str::<Value>(&stdout(&o)).unwrap()["level"],
        "score"
    );
}

#[test]
fn evaluate_frames_level_and_reprocess_score_level_exit_2() {
    let o = cli(&[
        "research",
        "evaluate",
        SESSION,
        "--baselines",
        &baselines_path(),
        "--level",
        "frames",
    ]);
    assert_eq!(o.status.code(), Some(2));
    assert!(stderr(&o).contains("no frame log"), "{}", stderr(&o));

    let o = cli(&[
        "research",
        "reprocess",
        "whatever.jsonl",
        "--catalog",
        &catalog_path(),
        "--baselines",
        &baselines_path(),
        "--age-months",
        "28",
        "--level",
        "score",
    ]);
    assert_eq!(o.status.code(), Some(2));
    assert!(stderr(&o).contains("no score rows"), "{}", stderr(&o));
}

#[test]
fn usage_and_input_errors_exit_1() {
    assert_eq!(cli(&["research"]).status.code(), Some(1));
    assert_eq!(cli(&["research", "bogus"]).status.code(), Some(1));
    assert_eq!(cli(&["research", "evaluate"]).status.code(), Some(1));
    // missing baselines
    assert_eq!(
        cli(&["research", "evaluate", SESSION]).status.code(),
        Some(1)
    );
    // bad flag value, unknown flag, unreadable file
    for extra in [["--age-months", "x"], ["--level", "nope"], ["--bogus", "1"]] {
        let mut args = vec!["research", "evaluate", SESSION];
        args.extend(extra);
        assert_eq!(cli(&args).status.code(), Some(1), "{extra:?}");
    }
    let b = baselines_path();
    let o = cli(&[
        "research",
        "evaluate",
        "/nonexistent.json",
        "--baselines",
        &b,
    ]);
    assert_eq!(o.status.code(), Some(1));
    assert!(stderr(&o).contains("/nonexistent.json"));
}

#[test]
fn invalid_profile_exits_1_and_names_the_problem() {
    let dir = scratch("profile");
    let mut profile = serde_json::to_value(AnalysisProfile::default()).unwrap();
    profile["metrics"]["ivtVelocityUnitsPerSec"] = Value::from(60.0);
    let path = dir.join("bad.json");
    fs::write(&path, profile.to_string()).unwrap();
    let b = baselines_path();
    let o = cli(&[
        "research",
        "evaluate",
        SESSION,
        "--baselines",
        &b,
        "--profile",
        p(&path),
    ]);
    assert_eq!(o.status.code(), Some(1));
    assert!(
        stderr(&o).contains("metrics.ivtVelocityUnitsPerSec"),
        "{}",
        stderr(&o)
    );
}
