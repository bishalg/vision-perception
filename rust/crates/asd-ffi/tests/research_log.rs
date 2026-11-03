//! owner: WP-X.0
//!
//! RFL-1: the research frame log writers the iOS kit calls (`gaze_research_log_header_line`,
//! `gaze_research_log_sample_line`). They take the asd-core JSON objects, apply the bench
//! encoder's canonical rules, and every document they write parses with
//! `asd_core::gaze::raw_log` (what `asd-cli gaze replay --check` runs).

use std::path::PathBuf;

use asd_core::gaze::raw_log;
use asd_ffi::raw_log::{gaze_research_log_header_line, gaze_research_log_sample_line};
use asd_ffi::AsdError;
use serde_json::{json, Value};

fn fixture_lines() -> Vec<String> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../asd-core/tests/fixtures/raw_tracking_sample.jsonl");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
        .lines()
        .map(str::to_owned)
        .collect()
}

fn research_header() -> Value {
    let mut header: Value = serde_json::from_str(&fixture_lines()[0]).unwrap();
    header["sessionId"] = json!("SESS-RES-1");
    header["participantToken"] = json!("child-RES");
    header["bundleVersion"] = json!("2026.11.0");
    header["profileVersion"] = json!("1.2.0");
    header["clips"] = json!([{
        "clipId": "stim_01",
        "order": 0,
        "catalogVersion": "1.4.0",
        "videoAspect": 1.7777777777777777,
        "aois": [{"name": "eyes", "startSec": 0.0, "endSec": 10.0,
                  "rect": {"xMin": 0.3, "yMin": 0.2, "xMax": 0.7, "yMax": 0.4},
                  "roles": ["social"]}]
    }]);
    header
}

#[test]
fn a_research_log_written_by_the_wrappers_parses() {
    let lines = fixture_lines();
    let header = gaze_research_log_header_line(research_header().to_string()).unwrap();
    let mut doc = header.clone();
    doc.push('\n');
    for (i, line) in lines.iter().skip(1).enumerate() {
        let mut sample: Value = serde_json::from_str(line).unwrap();
        if i == 0 {
            sample["stimulus_id"] = json!("stim_01");
            sample["affect"] = json!({"socialSmileIntensity": 0.25, "expressivityIndex": 0.4});
        }
        doc.push_str(&gaze_research_log_sample_line(sample.to_string()).unwrap());
        doc.push('\n');
    }
    let log = raw_log::parse(&doc).expect("research log parses");
    assert_eq!(log.header.session_id.as_deref(), Some("SESS-RES-1"));
    assert_eq!(log.header.clips[0].aois[0].name, "eyes");
    assert_eq!(
        log.samples[0]
            .affect
            .as_ref()
            .unwrap()
            .social_smile_intensity,
        Some(0.25)
    );
    assert_eq!(log.samples.len(), lines.len() - 1);
}

#[test]
fn a_sample_line_without_additions_equals_the_fixture_line() {
    for line in fixture_lines().iter().skip(1) {
        assert_eq!(&gaze_research_log_sample_line(line.clone()).unwrap(), line);
    }
}

#[test]
fn untracked_frames_lose_face_fields_and_bad_input_is_refused() {
    let mut sample: Value = serde_json::from_str(&fixture_lines()[1]).unwrap();
    sample["face_tracked"] = json!(false);
    let line: Value =
        serde_json::from_str(&gaze_research_log_sample_line(sample.to_string()).unwrap()).unwrap();
    assert!(line.get("face_transform").is_none() && line.get("look_at").is_none());

    assert!(matches!(
        gaze_research_log_sample_line("{".into()),
        Err(AsdError::InvalidJson { .. })
    ));
    let mut header = research_header();
    header.as_object_mut().unwrap().remove("sessionId");
    assert!(matches!(
        gaze_research_log_header_line(header.to_string()),
        Err(AsdError::InvalidArgument { .. })
    ));
    let mut header = research_header();
    header["format"] = json!("asd-raw-tracking/2");
    assert!(matches!(
        gaze_research_log_header_line(header.to_string()),
        Err(AsdError::InvalidArgument { .. })
    ));
}
