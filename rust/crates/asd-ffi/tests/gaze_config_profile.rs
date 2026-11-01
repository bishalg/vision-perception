//! owner: WP-A.13
//!
//! CFG-3: `gaze_config_json_for_profile` builds the per-session `PipelineConfig` from the
//! device geometry (as `gaze_default_config_json`) with `qualityGate` and `params` taken from
//! the delivered `ScoringProfile`.
//!
//! - A 1.1.0 profile with a changed `trackingQuality` threshold (drift) reaches the config.
//! - A 1.0.0 profile (no `trackingQuality`) yields JSON identical to `gaze_default_config_json`.
//! - `qualityGate` comes from the profile.
//! - A four-point `calibration.layout5` reaches the config (acceptance 24).
//! - Malformed profile JSON is `AsdError::InvalidJson`, never a panic.

use asd_contract::config::ScoringProfile;
use asd_core::gaze::pipeline::PipelineConfig;
use asd_ffi::gaze::{gaze_config_json_for_profile, gaze_default_config_json, GazeOrientation};
use asd_ffi::{default_scoring_profile_json, AsdError};
use serde_json::{json, Value};

const MODEL: &str = "iPhone15,2";

fn config_for(profile: &Value, orientation: GazeOrientation) -> Value {
    let j = gaze_config_json_for_profile(MODEL.into(), orientation, profile.to_string())
        .expect("config for profile");
    serde_json::from_str(&j).expect("config is JSON")
}

fn default_profile() -> Value {
    serde_json::from_str(&default_scoring_profile_json()).expect("default profile is JSON")
}

#[test]
fn changed_drift_threshold_in_a_1_1_0_profile_reaches_the_config() {
    let mut profile = default_profile();
    assert_eq!(profile["schemaVersion"], "1.1.0");
    profile["trackingQuality"]["drift"]["warnDeg"] = json!(3.25);
    profile["trackingQuality"]["drift"]["hardDeg"] = json!(7.5);

    let config = config_for(&profile, GazeOrientation::Portrait);
    assert_eq!(config["params"]["drift"]["warnDeg"], json!(3.25));
    assert_eq!(config["params"]["drift"]["hardDeg"], json!(7.5));
    assert_eq!(config["params"], profile["trackingQuality"]);
    // It still decodes as the pipeline's own config type.
    let typed: PipelineConfig = serde_json::from_value(config).expect("PipelineConfig");
    assert!((typed.params.drift.warn_deg - 3.25).abs() < 1e-12);
}

#[test]
fn profile_1_0_0_without_tracking_quality_equals_the_default_config_byte_for_byte() {
    let mut profile = default_profile();
    profile["schemaVersion"] = json!("1.0.0");
    profile
        .as_object_mut()
        .expect("profile object")
        .remove("trackingQuality");

    for orientation in [
        GazeOrientation::Portrait,
        GazeOrientation::LandscapeLeft,
        GazeOrientation::LandscapeRight,
        GazeOrientation::PortraitUpsideDown,
    ] {
        let from_profile =
            gaze_config_json_for_profile(MODEL.into(), orientation, profile.to_string()).unwrap();
        let default = gaze_default_config_json(MODEL.into(), orientation).unwrap();
        assert_eq!(from_profile, default, "{orientation:?}");
    }
}

#[test]
fn the_bundled_default_profile_equals_the_default_config_byte_for_byte() {
    let from_profile = gaze_config_json_for_profile(
        "iPhone17,3".into(),
        GazeOrientation::Portrait,
        default_scoring_profile_json(),
    )
    .unwrap();
    let default = gaze_default_config_json("iPhone17,3".into(), GazeOrientation::Portrait).unwrap();
    assert_eq!(from_profile, default);
}

#[test]
fn quality_gate_comes_from_the_profile() {
    let mut profile = default_profile();
    profile["qualityGate"] = json!({
        "minDistanceCm": 30.0,
        "maxDistanceCm": 70.0,
        "maxYawRad": 0.5,
        "maxPitchRad": 0.4,
        "minConfidence": 0.65,
        "blinkThreshold": 0.3
    });
    let config = config_for(&profile, GazeOrientation::Portrait);
    assert_eq!(config["qualityGate"], profile["qualityGate"]);
}

#[test]
fn geometry_still_comes_from_the_device_model() {
    let profile = default_profile();
    let config = config_for(&profile, GazeOrientation::LandscapeLeft);
    let default: Value = serde_json::from_str(
        &gaze_default_config_json(MODEL.into(), GazeOrientation::LandscapeLeft).unwrap(),
    )
    .unwrap();
    for key in [
        "modelIdentifier",
        "geometrySource",
        "geometry",
        "orientation",
    ] {
        assert_eq!(config[key], default[key], "{key}");
    }
}

#[test]
fn four_point_calibration_layout_reaches_the_config() {
    let mut profile = default_profile();
    let layout = json!([[0.5, 0.5], [0.12, 0.12], [0.88, 0.12], [0.12, 0.88]]);
    profile["trackingQuality"]["calibration"]["layout5"] = layout.clone();
    let config = config_for(&profile, GazeOrientation::Portrait);
    assert_eq!(config["params"]["calibration"]["layout5"], layout);
}

#[test]
fn malformed_profile_is_invalid_json_not_a_panic() {
    let mut wrong_type = default_profile();
    wrong_type["qualityGate"]["minDistanceCm"] = json!("near");
    let mut bad_params = default_profile();
    bad_params["trackingQuality"]["drift"] = json!(42);
    let mut no_gate = default_profile();
    no_gate.as_object_mut().unwrap().remove("qualityGate");

    for input in [
        String::new(),
        "not json".to_owned(),
        "[]".to_owned(),
        "null".to_owned(),
        "{".to_owned(),
        wrong_type.to_string(),
        bad_params.to_string(),
        no_gate.to_string(),
    ] {
        let r =
            gaze_config_json_for_profile(MODEL.into(), GazeOrientation::Portrait, input.clone());
        assert!(
            matches!(r, Err(AsdError::InvalidJson { .. })),
            "{input:?} -> {r:?}"
        );
    }
}

#[test]
fn round_trips_through_the_contract_profile_type() {
    // A profile serialised by the contract crate (what the backend signs) is accepted as is.
    let mut profile = ScoringProfile::default();
    profile.quality_gate.min_confidence = 0.7;
    profile.tracking_quality.drift.warn_deg = 2.75;
    let j = gaze_config_json_for_profile(
        MODEL.into(),
        GazeOrientation::Portrait,
        serde_json::to_string(&profile).unwrap(),
    )
    .unwrap();
    let typed: PipelineConfig = serde_json::from_str(&j).unwrap();
    assert!((typed.quality_gate.min_confidence - 0.7).abs() < 1e-12);
    assert_eq!(typed.params, profile.tracking_quality);
}

/// WP-A.13: a changed `qualityGate` in the profile reaches the pipeline's config and the
/// frames-file writer made from that pipeline (one source; Swift re-serialises nothing).
#[test]
fn changed_quality_gate_reaches_the_pipeline_and_its_writer() {
    use asd_ffi::gaze::GazePipelineHandle;
    let mut profile = default_profile();
    profile["qualityGate"]["minDistanceCm"] = json!(28.0);
    profile["qualityGate"]["blinkThreshold"] = json!(0.3);
    let config =
        gaze_config_json_for_profile(MODEL.into(), GazeOrientation::Portrait, profile.to_string())
            .unwrap();
    let pipeline = GazePipelineHandle::new(config).unwrap();
    let held: Value = serde_json::from_str(&pipeline.config_json().unwrap()).unwrap();
    assert_eq!(held["qualityGate"], profile["qualityGate"]);
    let writer = pipeline.telemetry_writer(0.0);
    let gate: Value = serde_json::from_str(&writer.quality_gate_json().unwrap()).unwrap();
    assert_eq!(gate, profile["qualityGate"]);
}

/// WP-A.13: a profile that decodes but fails the publisher's value checks (asd-contract
/// `profile_checks`, incl. `TrackingQualityParams::validate`) never configures the pipeline.
#[test]
fn profile_with_invalid_values_is_refused() {
    let mut empty_layout = default_profile();
    empty_layout["trackingQuality"]["calibration"]["layout5"] = json!([]);
    let mut zero_collect = default_profile();
    zero_collect["trackingQuality"]["calibration"]["collectMs"] = json!(0);
    let mut drift_inverted = default_profile();
    drift_inverted["trackingQuality"]["drift"]["warnDeg"] = json!(9.0);
    drift_inverted["trackingQuality"]["drift"]["hardDeg"] = json!(4.0);
    let mut gate_inverted = default_profile();
    gate_inverted["qualityGate"]["minDistanceCm"] = json!(90.0);

    for (profile, member) in [
        (empty_layout, "trackingQuality.calibration.layout5"),
        (zero_collect, "trackingQuality.calibration.collectMs"),
        (drift_inverted, "trackingQuality.drift.warnDeg"),
        (gate_inverted, "qualityGate"),
    ] {
        match gaze_config_json_for_profile(
            MODEL.into(),
            GazeOrientation::Portrait,
            profile.to_string(),
        ) {
            Err(AsdError::InvalidJson { message }) => {
                assert!(message.contains(member), "{member}: {message}")
            }
            other => panic!("{member}: expected InvalidJson, got {other:?}"),
        }
    }
}
