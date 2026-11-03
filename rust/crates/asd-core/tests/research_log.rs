//! owner: WP-X.0
//!
//! RFL-1: the research frame log is `asd-raw-tracking/1` extended additively. An old log
//! (the committed WP-G.1 sample, a synthetic log) parses with the additions empty and writes
//! back byte for byte; a research log with clips, AOIs, ids and per-frame affect round-trips;
//! `replay` ignores the additions.

use std::path::PathBuf;

use asd_core::gaze::aoi_space::{NormBox, ScreenAoi};
use asd_core::gaze::raw_log::{self, RawLog};
use asd_core::gaze::replay::{replay, ReplayOptions};
use asd_core::synth_tracking::{generate, Scenario};
use asd_core::types::tracking::RawLogClip;
use asd_core::types::GazeFrameAffect;

fn fixture() -> String {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/raw_tracking_sample.jsonl");
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn research(mut log: RawLog) -> RawLog {
    log.header.clips = vec![RawLogClip {
        clip_id: "synth_clip_1".into(),
        order: 0,
        catalog_version: Some("1.4.0".into()),
        aois: vec![ScreenAoi {
            name: "eyes".into(),
            start_sec: 0.0,
            end_sec: 10.0,
            rect: NormBox {
                x_min: 0.3,
                y_min: 0.2,
                x_max: 0.7,
                y_max: 0.4,
            },
            roles: vec!["social".into()],
        }],
        video_aspect: Some(16.0 / 9.0),
    }];
    log.header.bundle_version = Some("2026.11.0".into());
    log.header.profile_version = Some("1.2.0".into());
    log.header.session_id = Some("SESS-RES-1".into());
    log.header.participant_token = Some("child-RES".into());
    for s in log.samples.iter_mut().filter(|s| s.stimulus_id.is_some()) {
        s.affect = Some(GazeFrameAffect {
            social_smile_intensity: Some(0.25),
            expressivity_index: Some(0.4),
            surprise_score: None,
        });
    }
    log
}

#[test]
fn an_old_log_parses_with_empty_additions_and_writes_back_unchanged() {
    let text = fixture();
    let log = raw_log::parse(&text).expect("the WP-G.1 sample parses");
    assert!(log.header.clips.is_empty());
    assert!(log.header.session_id.is_none() && log.header.participant_token.is_none());
    assert!(log.header.bundle_version.is_none() && log.header.profile_version.is_none());
    assert!(log.samples.iter().all(|s| s.affect.is_none()));

    let synth = generate(Scenario::Ideal, 7);
    let jsonl = raw_log::to_jsonl(&synth).unwrap();
    for key in [
        "clips",
        "sessionId",
        "participantToken",
        "bundleVersion",
        "affect",
    ] {
        assert!(
            !jsonl.contains(&format!("\"{key}\"")),
            "{key} absent from an old log"
        );
    }
    assert_eq!(
        raw_log::to_jsonl(&raw_log::parse(&jsonl).unwrap()).unwrap(),
        jsonl
    );
}

#[test]
fn a_research_log_round_trips() {
    let log = research(generate(Scenario::Ideal, 7));
    let jsonl = raw_log::to_jsonl(&log).unwrap();
    let header: serde_json::Value = serde_json::from_str(jsonl.lines().next().unwrap()).unwrap();
    assert_eq!(header["sessionId"], "SESS-RES-1");
    assert_eq!(header["clips"][0]["clipId"], "synth_clip_1");
    assert_eq!(header["clips"][0]["aois"][0]["rect"]["xMin"], 0.3);
    let with_affect = jsonl
        .lines()
        .skip(1)
        .find(|l| l.contains("\"affect\""))
        .expect("a recorded frame carries affect");
    let sample: serde_json::Value = serde_json::from_str(with_affect).unwrap();
    assert_eq!(sample["affect"]["socialSmileIntensity"], 0.25);
    assert!(sample["affect"].get("surpriseScore").is_none());
    assert_eq!(raw_log::parse(&jsonl).unwrap(), log);
}

#[test]
fn replay_ignores_the_research_additions() {
    let plain = generate(Scenario::Ideal, 11);
    let extended = research(plain.clone());
    let a = replay(&plain, &ReplayOptions::default()).unwrap();
    let b = replay(&extended, &ReplayOptions::default()).unwrap();
    assert_eq!(
        serde_json::to_string(&a).unwrap(),
        serde_json::to_string(&b).unwrap()
    );
}
