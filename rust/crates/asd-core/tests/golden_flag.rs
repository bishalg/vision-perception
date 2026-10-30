//! owner: WP-R.1
//!
//! Golden parity for `flag::assess` (CORE-4, FFI-4). Vectors in
//! `tests/golden/flag/*.json` were produced by the unchanged Swift `AttentionFlagAlgorithm`.

use std::path::PathBuf;

use asd_core::flag::{self, FlagThresholds};
use asd_core::types::{AttentionFlag, PerVideoSessionScore};
use serde_json::Value;

fn vectors(file: &str) -> Vec<(String, Value, Value)> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden/flag")
        .join(file);
    let doc: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(doc["schema"], "asd-golden/1");
    assert_eq!(doc["function"], "assess");
    let out: Vec<_> = doc["vectors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| {
            (
                v["name"].as_str().unwrap().to_owned(),
                v["input"].clone(),
                v["expected"].clone(),
            )
        })
        .collect();
    assert!(!out.is_empty());
    out
}

#[test]
fn golden_assess() {
    for (name, input, expected) in vectors("assess_v1.json") {
        let score: PerVideoSessionScore = serde_json::from_value(input["score"].clone())
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        let thresholds: FlagThresholds = match input.get("thresholds") {
            Some(t) => serde_json::from_value(t.clone()).unwrap(),
            None => FlagThresholds::default(),
        };
        let actual = serde_json::to_value(flag::assess(&score, &thresholds)).unwrap();
        assert_eq!(actual, expected, "{name}");
    }
}

#[test]
fn default_thresholds_equal_swift_constants() {
    let t = FlagThresholds::default();
    assert_eq!(t.green_floor, 0.65);
    assert_eq!(t.yellow_floor, 0.35);
    assert_eq!(t.low_confidence_share, 0.50);
    assert_eq!(t.atypical_share_for_red, 0.40);
}

#[test]
fn thresholds_use_profile_key_names() {
    let t: FlagThresholds = serde_json::from_str(
        r#"{"greenFloor":0.7,"yellowFloor":0.3,"lowConfidenceShare":0.6,"atypicalShareForRed":0.5}"#,
    )
    .unwrap();
    assert_eq!(t.green_floor, 0.7);
    let back = serde_json::to_value(t).unwrap();
    assert_eq!(back["atypicalShareForRed"], 0.5);
}

/// The profile thresholds are honoured, not the constants: a composite that is yellow under
/// the defaults turns green when the green floor is lowered.
#[test]
fn custom_thresholds_change_the_flag() {
    let (_, input, expected) = vectors("assess_v1.json")
        .into_iter()
        .find(|(n, _, _)| n == "testYellowWhenCompositeSitsBetweenTheFloors")
        .unwrap();
    assert_eq!(expected["flag"], "yellow");
    let score: PerVideoSessionScore = serde_json::from_value(input["score"].clone()).unwrap();
    let lowered = FlagThresholds {
        green_floor: 0.5,
        ..FlagThresholds::default()
    };
    assert_eq!(flag::assess(&score, &lowered).flag, AttentionFlag::Green);
    let raised = FlagThresholds {
        yellow_floor: 0.6,
        ..FlagThresholds::default()
    };
    assert_eq!(flag::assess(&score, &raised).flag, AttentionFlag::Red);
}

#[test]
fn every_detail_carries_the_screening_disclaimer() {
    for (name, _, expected) in vectors("assess_v1.json") {
        assert_eq!(expected["isScreeningSignalOnly"], true, "{name}");
        assert!(
            expected["detail"]
                .as_str()
                .unwrap()
                .ends_with("This is a screening signal, not a diagnosis."),
            "{name}"
        );
    }
}
