//! owner: WP-R.3
//!
//! CON-5: the default `ScoringProfile` equals today's Swift constants (read from the Swift
//! sources, so a change on either side fails here), config documents use the agreed key
//! casing, and a bundle composed from the bundled catalog and baselines validates against
//! the exported `config-bundle-v1.json`.

use std::path::PathBuf;

use asd_contract::config::{
    ConfigBundle, FlagThresholds, QualityGateParams, ScoringProfile, TrackingQualityParams,
    SCORING_PROFILE_SCHEMA_VERSION,
};
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
            schema_version: "1.1.0".into(),
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
            tracking_quality: asd_core::types::tracking::TrackingQualityParams::default(),
        }
    );
}

#[test]
fn scoring_profile_is_camel_case() {
    let v = serde_json::to_value(ScoringProfile::default()).unwrap();
    let tracking = serde_json::to_value(TrackingQualityParams::default()).unwrap();
    assert_eq!(
        v,
        json!({
            "schemaVersion": "1.1.0",
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
            },
            "trackingQuality": tracking
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

// ---- SCH-3 (WP-G.6): `ScoringProfile.trackingQuality` ------------------------------------

#[test]
fn scoring_profile_schema_version_is_1_1_0() {
    assert_eq!(SCORING_PROFILE_SCHEMA_VERSION, "1.1.0");
    assert_eq!(ScoringProfile::default().schema_version, "1.1.0");
}

#[test]
fn tracking_quality_params_are_the_asd_core_type_and_defaults() {
    // Single source: the contract re-exports the asd-core type, it does not copy it.
    let p: asd_core::types::tracking::TrackingQualityParams =
        ScoringProfile::default().tracking_quality;
    assert_eq!(
        p,
        asd_core::types::tracking::TrackingQualityParams::default()
    );
    assert_eq!(
        p.schema_version,
        asd_core::types::tracking::TRACKING_PARAMS_SCHEMA_VERSION
    );
}

#[test]
fn an_old_profile_without_tracking_quality_decodes_to_the_defaults() {
    let mut v = serde_json::to_value(ScoringProfile::default()).unwrap();
    v.as_object_mut().unwrap().remove("trackingQuality");
    v["schemaVersion"] = json!("1.0.0");
    let p: ScoringProfile = serde_json::from_value(v).expect("1.0.0 profile decodes");
    assert_eq!(p.tracking_quality, TrackingQualityParams::default());
    assert_eq!(
        p.schema_version, "1.0.0",
        "the document's own version is kept"
    );

    // A partial object fills the rest from the defaults (per-struct serde(default)).
    let mut v = serde_json::to_value(ScoringProfile::default()).unwrap();
    v["trackingQuality"] = json!({"provisional": false, "drift": {"warnDeg": 2.0}});
    let p: ScoringProfile = serde_json::from_value(v).unwrap();
    assert!(!p.tracking_quality.provisional);
    assert_eq!(p.tracking_quality.drift.warn_deg, 2.0);
    assert_eq!(
        p.tracking_quality.drift.hard_deg,
        TrackingQualityParams::default().drift.hard_deg
    );
}

#[test]
fn an_old_bundle_without_tracking_quality_decodes_and_validates() {
    let mut bundle = serde_json::to_value(sample_bundle()).unwrap();
    bundle["signature"] = json!("ed25519:AAAA");
    let profile = bundle["scoring_profile"].as_object_mut().unwrap();
    profile.remove("trackingQuality");
    profile.insert("schemaVersion".into(), json!("1.0.0"));
    let decoded: ConfigBundle = serde_json::from_value(bundle.clone()).expect("old bundle");
    assert_eq!(
        decoded.scoring_profile.tracking_quality,
        TrackingQualityParams::default()
    );
    schema::validate(&schema::config_bundle_v1(), &bundle).expect("old bundle validates");
}

#[test]
fn exported_tracking_quality_default_equals_the_asd_core_defaults() {
    let core =
        serde_json::to_value(asd_core::types::tracking::TrackingQualityParams::default()).unwrap();
    let schema = schema::config_bundle_v1();
    let profile = &schema["$defs"]["ScoringProfile"];
    assert_eq!(
        profile["properties"]["trackingQuality"]["default"], core,
        "schema default is the asd-core default"
    );
    assert_eq!(profile["examples"][0]["trackingQuality"], core);
    let required = profile["required"].as_array().unwrap();
    assert!(
        !required.contains(&json!("trackingQuality")),
        "trackingQuality is optional (serde default)"
    );
    // Field-level defaults inside the params come from the same asd-core values.
    let drift = &schema["$defs"]["DriftParams"]["properties"];
    assert_eq!(drift["warnDeg"]["default"], core["drift"]["warnDeg"]);
    assert_eq!(drift["hardDeg"]["default"], core["drift"]["hardDeg"]);
    let committed: Value = serde_json::from_str(
        &std::fs::read_to_string(repo_root().join("docs/spec/schemas/asd/config-bundle-v1.json"))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(committed, schema, "config-bundle-v1.json regenerated");
}

#[test]
fn schema_rejects_bad_tracking_params() {
    let schema = schema::config_bundle_v1();
    let mut base = serde_json::to_value(sample_bundle()).unwrap();
    base["signature"] = json!("ed25519:AAAA");
    for (pointer, bad) in [
        (
            "/scoring_profile/trackingQuality/drift/warnDeg",
            json!("1.5"),
        ),
        (
            "/scoring_profile/trackingQuality/calibration/defaultModel",
            json!("cubic"),
        ),
        ("/scoring_profile/trackingQuality/provisional", json!(1)),
        (
            "/scoring_profile/trackingQuality/calibration/layout5/0",
            json!([0.5]),
        ),
    ] {
        let mut v = base.clone();
        *v.pointer_mut(pointer).unwrap() = bad;
        assert!(
            schema::validate(&schema, &v).is_err(),
            "{pointer} must be rejected"
        );
    }
}
