//! owner: WP-R.3
//!
//! CON-5: the default `ScoringProfile` equals today's Swift constants (read from the Swift
//! sources, so a change on either side fails here), config documents use the agreed key
//! casing, and a bundle composed from the bundled catalog and baselines validates against
//! the exported `config-bundle-v1.json`.

use std::path::PathBuf;

use asd_contract::config::{ConfigBundle, FlagThresholds, QualityGateParams, ScoringProfile};
use asd_contract::schema;
use asd_core::types::{NormativeBaselineSet, StimulusCatalog};
use serde_json::{json, Value};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn kit_source(name: &str) -> String {
    let path = repo_root()
        .join("domains/asd_screening/ios/Sources/CurvAsdGazeKit")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// The numeric literal after `marker` on the same line, e.g. `static let greenFloor = 0.65`
/// or `minDistanceCm: Double = 35.0,`.
fn swift_constant(source: &str, marker: &str) -> f64 {
    let line = source
        .lines()
        .find(|l| l.contains(marker))
        .unwrap_or_else(|| panic!("`{marker}` not found in Swift source"));
    let after = &line[line.find(marker).unwrap() + marker.len()..];
    let after = after.split_once('=').map_or(after, |(_, rhs)| rhs);
    let literal: String = after
        .trim_start()
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    literal
        .parse()
        .unwrap_or_else(|_| panic!("`{marker}`: no literal in `{line}`"))
}

#[test]
fn default_scoring_profile_equals_the_swift_constants() {
    let flag_src = kit_source("AttentionFlag.swift");
    let engine_src = kit_source("PerVideoScoringEngine.swift");
    let gate_src = kit_source("GazeQualityGate.swift");
    let p = ScoringProfile::default();

    assert_eq!(
        p.flag.green_floor,
        swift_constant(&flag_src, "static let greenFloor")
    );
    assert_eq!(
        p.flag.yellow_floor,
        swift_constant(&flag_src, "static let yellowFloor")
    );
    assert_eq!(
        p.flag.low_confidence_share,
        swift_constant(&flag_src, "static let lowConfidenceShare")
    );
    assert_eq!(
        p.flag.atypical_share_for_red,
        swift_constant(&flag_src, "static let atypicalShareForRed")
    );
    assert_eq!(
        p.low_confidence_valid_ratio,
        swift_constant(&engine_src, "static let lowConfidenceValidRatio")
    );
    let g = &p.quality_gate;
    assert_eq!(
        g.min_distance_cm,
        swift_constant(&gate_src, "minDistanceCm: Double =")
    );
    assert_eq!(
        g.max_distance_cm,
        swift_constant(&gate_src, "maxDistanceCm: Double =")
    );
    assert_eq!(
        g.max_yaw_rad,
        swift_constant(&gate_src, "maxYawRad: Double =")
    );
    assert_eq!(
        g.max_pitch_rad,
        swift_constant(&gate_src, "maxPitchRad: Double =")
    );
    assert_eq!(
        g.min_confidence,
        swift_constant(&gate_src, "minConfidence: Double =")
    );
    assert_eq!(
        g.blink_threshold,
        swift_constant(&gate_src, "blinkThreshold: Double =")
    );
}

#[test]
fn default_scoring_profile_literal_values() {
    // Spelled out so a reviewer sees the numbers (F3, `GazeQualityGate.init`).
    assert_eq!(
        ScoringProfile::default(),
        ScoringProfile {
            schema_version: "1.0.0".into(),
            flag: FlagThresholds {
                green_floor: 0.65,
                yellow_floor: 0.35,
                low_confidence_share: 0.50,
                atypical_share_for_red: 0.40,
            },
            low_confidence_valid_ratio: 0.60,
            quality_gate: QualityGateParams {
                min_distance_cm: 35.0,
                max_distance_cm: 65.0,
                max_yaw_rad: 0.61,
                max_pitch_rad: 0.52,
                min_confidence: 0.50,
                blink_threshold: 0.45,
            },
        }
    );
}

#[test]
fn scoring_profile_is_camel_case() {
    let v = serde_json::to_value(ScoringProfile::default()).unwrap();
    assert_eq!(
        v,
        json!({
            "schemaVersion": "1.0.0",
            "flag": {
                "greenFloor": 0.65,
                "yellowFloor": 0.35,
                "lowConfidenceShare": 0.5,
                "atypicalShareForRed": 0.4
            },
            "lowConfidenceValidRatio": 0.6,
            "qualityGate": {
                "minDistanceCm": 35.0,
                "maxDistanceCm": 65.0,
                "maxYawRad": 0.61,
                "maxPitchRad": 0.52,
                "minConfidence": 0.5,
                "blinkThreshold": 0.45
            }
        })
    );
    let back: ScoringProfile = serde_json::from_value(v).unwrap();
    assert_eq!(back, ScoringProfile::default());
}

fn bundled() -> (StimulusCatalog, NormativeBaselineSet) {
    let res = repo_root().join("domains/asd_screening/ios/Sources/CurvAsdGazeKit/Resources");
    let catalog = std::fs::read_to_string(res.join("stimulus_catalog.json")).unwrap();
    let baselines = std::fs::read_to_string(res.join("normative_baselines.json")).unwrap();
    (
        serde_json::from_str(&catalog).unwrap(),
        serde_json::from_str(&baselines).unwrap(),
    )
}

fn sample_bundle() -> ConfigBundle {
    let (catalog, baselines) = bundled();
    ConfigBundle {
        bundle_version: "2026.10.30-1".into(),
        key_id: "ed25519-test".into(),
        catalog,
        baselines,
        scoring_profile: ScoringProfile::default(),
        feature_flags: [("norm_collection".to_string(), false)].into(),
        signed_at: "2026-10-30T12:00:00.000Z".into(),
        signature: String::new(),
    }
}

#[test]
fn config_bundle_is_snake_case_at_the_top_level() {
    let v = serde_json::to_value(sample_bundle()).unwrap();
    let mut keys: Vec<&String> = v.as_object().unwrap().keys().collect();
    keys.sort();
    assert_eq!(
        keys,
        [
            "baselines",
            "bundle_version",
            "catalog",
            "feature_flags",
            "key_id",
            "scoring_profile",
            "signature",
            "signed_at"
        ]
    );
    assert!(
        v["catalog"].get("agePacks").is_some(),
        "documents keep camelCase"
    );
    assert!(v["scoring_profile"].get("qualityGate").is_some());
}

#[test]
fn bundled_documents_compose_into_a_bundle_that_validates() {
    let schema = schema::config_bundle_v1();
    assert_eq!(
        schema["$id"],
        json!("https://schemas.curv.org/asd/config-bundle-v1.json")
    );
    let mut bundle = serde_json::to_value(sample_bundle()).unwrap();
    bundle["signature"] = json!("ed25519:AAAA");
    schema::validate(&schema, &bundle).expect("bundle validates");

    let mut bad: Value = bundle.clone();
    bad["scoring_profile"]["flag"]
        .as_object_mut()
        .unwrap()
        .remove("greenFloor");
    assert!(schema::validate(&schema, &bad).is_err());

    let mut bad = bundle;
    bad["signature"] = json!("rsa:xyz");
    assert!(
        schema::validate(&schema, &bad).is_err(),
        "signature prefix is part of the contract"
    );
}

#[test]
fn bundled_catalog_and_baselines_validate_against_their_schema_definitions() {
    // Guards the schema-only mirrors of the asd-core types: every key the real documents
    // carry is declared (strict check: unknown keys rejected in this test only).
    let mut schema = schema::config_bundle_v1();
    schema::forbid_additional_properties(&mut schema);
    let mut bundle = serde_json::to_value(sample_bundle()).unwrap();
    bundle["signature"] = json!("ed25519:AAAA");
    schema::validate(&schema, &bundle).expect("strict validation of bundled documents");
}
