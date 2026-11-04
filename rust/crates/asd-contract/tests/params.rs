//! owner: WP-X.0
//!
//! PRM-1..4 and PRM-7: the parameter registry describes every leaf of the 1.3.0 scoring
//! profile (completeness, both directions), refuses out-of-range and unknown members with
//! their paths, keeps the cross-field rules, and the 1.2.0 sections default to today's
//! constants so older profiles decode to the same behaviour.
//!
//! DSP-1, DSP-2, DSP-6 (WP-X.0c): the 16 `display.*` / `sessionQuality.*` rows, their
//! cross-field rules, `ageBands[].representativeMonths`, and `validate_for_device`.

use std::collections::BTreeSet;

use asd_contract::config::{
    IvtUnit, ScoringProfile, TrackingQualityParams, SCORING_PROFILE_SCHEMA_VERSION,
};
use asd_contract::params::{
    self, leaf_paths, registry, validate, validate_for_device, DocumentKind, ParamKind, Reprocess,
    Stage, Unit, NON_PARAMETER_LEAVES, PARAM_REGISTRY_FILE, PARAM_REGISTRY_ID, REGISTRY_VERSION,
};
use asd_contract::profile_checks::{profile_problems, scoring_profile_problems};
use asd_contract::schema;
use serde_json::{json, Value};

fn default_body() -> Value {
    serde_json::to_value(ScoringProfile::default()).unwrap()
}

fn problems(body: &Value) -> Vec<(String, String)> {
    validate(DocumentKind::ScoringProfile, body)
        .into_iter()
        .map(|p| (p.path, p.message))
        .collect()
}

fn paths(body: &Value) -> Vec<String> {
    problems(body).into_iter().map(|(p, _)| p).collect()
}

fn set(body: &mut Value, path: &str, v: Value) {
    let mut node = body;
    let parts: Vec<&str> = path.split('.').collect();
    for key in &parts[..parts.len() - 1] {
        node = node.get_mut(*key).unwrap();
    }
    node[parts[parts.len() - 1]] = v;
}

// ---- PRM-2: completeness ----------------------------------------------------------------

#[test]
fn every_leaf_of_the_default_profile_is_registered_and_every_row_is_a_leaf() {
    let leaves: BTreeSet<String> = leaf_paths(&default_body())
        .into_iter()
        .filter(|p| !NON_PARAMETER_LEAVES.contains(&p.as_str()))
        .collect();
    let ids: BTreeSet<String> = registry().iter().map(|s| s.id.clone()).collect();
    let missing: Vec<&String> = leaves.difference(&ids).collect();
    let stale: Vec<&String> = ids.difference(&leaves).collect();
    assert!(
        missing.is_empty(),
        "profile leaves without a ParamSpec: {missing:?}"
    );
    assert!(
        stale.is_empty(),
        "ParamSpecs without a profile leaf: {stale:?}"
    );
    assert_eq!(ids.len(), registry().len(), "ids are unique");
    // Every trackingQuality leaf is a parameter (most of the registry).
    let tracking = ids
        .iter()
        .filter(|i| i.starts_with("trackingQuality."))
        .count();
    assert!(tracking >= 60, "{tracking} trackingQuality rows");
}

#[test]
fn the_completeness_check_names_a_field_without_a_spec() {
    // What the test above reports when a field is added without a row (acceptance 1).
    let mut body = default_body();
    body["metrics"]["newKnob"] = json!(1.0);
    let ids: BTreeSet<String> = registry().iter().map(|s| s.id.clone()).collect();
    let missing: Vec<String> = leaf_paths(&body)
        .into_iter()
        .filter(|p| !NON_PARAMETER_LEAVES.contains(&p.as_str()) && !ids.contains(p))
        .collect();
    assert_eq!(missing, ["metrics.newKnob"]);
    // And `validate` refuses it as unknown.
    assert_eq!(
        problems(&body),
        [(
            "metrics.newKnob".to_string(),
            "metrics.newKnob: unknown parameter".to_string()
        )]
    );
}

#[test]
fn rows_carry_defaults_message_keys_and_consistent_kinds() {
    let body = default_body();
    for spec in registry() {
        assert_eq!(
            Some(&spec.default),
            params::get_path(&body, &spec.id),
            "{}",
            spec.id
        );
        assert_eq!(spec.label_key, format!("param.{}.label", spec.id));
        assert_eq!(spec.help_key, format!("param.{}.help", spec.id));
        match &spec.kind {
            ParamKind::Number { min, max, step, .. } => {
                assert!(min < max && *step > 0.0, "{}", spec.id);
                let v = spec.default.as_f64().unwrap();
                assert!(*min <= v && v <= *max, "{} default {v} in range", spec.id);
            }
            ParamKind::Integer { min, max, .. } => {
                let v = spec.default.as_i64().unwrap();
                assert!(*min <= v && v <= *max, "{} default {v} in range", spec.id);
            }
            ParamKind::Bool => assert!(spec.default.is_boolean(), "{}", spec.id),
            ParamKind::Enum { values } => {
                assert!(values.contains(&spec.default.as_str().unwrap().to_owned()))
            }
            ParamKind::PointList { .. } | ParamKind::AgeBands { .. } => {
                assert!(spec.default.is_array(), "{}", spec.id)
            }
        }
        let since = spec.since.as_str();
        assert!(
            ["1.0.0", "1.1.0", "1.2.0", "1.3.0"].contains(&since),
            "{}",
            spec.id
        );
    }
    assert_eq!(params::registry_version(), REGISTRY_VERSION);
}

#[test]
fn owner_gated_knobs_are_the_scr_rows_and_the_degree_ivt() {
    let gated: BTreeSet<&str> = registry()
        .iter()
        .filter(|s| s.owner_gate.as_deref() == Some("H-G2"))
        .map(|s| s.id.as_str())
        .collect();
    assert!(gated.contains("trackingQuality.scoring.maxAccuracyDegForScoring"));
    assert!(gated.contains("trackingQuality.scoring.aoiResolutionFactor"));
    assert!(gated.contains("trackingQuality.scoring.ivtVelocityDegPerSec"));
    assert!(gated.contains("metrics.ivtUnit"));
    assert!(!gated.contains("metrics.ivtVelocityUnitsPerSec"));
    // Layouts are shown during capture: only a new capture changes them.
    assert_eq!(
        params::spec("trackingQuality.calibration.layout5")
            .unwrap()
            .reprocess,
        Reprocess::CaptureOnly
    );
    assert_eq!(
        params::spec("flag.greenFloor").unwrap().reprocess,
        Reprocess::Score
    );
    assert_eq!(
        params::spec("metrics.ivtVelocityUnitsPerSec")
            .unwrap()
            .reprocess,
        Reprocess::Frames
    );
}

// ---- PRM-3: validation -----------------------------------------------------------------

#[test]
fn the_default_profile_and_older_bodies_are_valid() {
    assert!(problems(&default_body()).is_empty());
    assert!(profile_problems(&ScoringProfile::default()).is_empty());

    // A 1.1.0 body (no 1.2.0 sections) and a 1.0.0 body (no trackingQuality either).
    let mut v110 = default_body();
    for k in ["metrics", "aoi", "ageBands", "norms", "review"] {
        v110.as_object_mut().unwrap().remove(k);
    }
    v110["schemaVersion"] = json!("1.1.0");
    for k in ["display", "sessionQuality"] {
        v110.as_object_mut().unwrap().remove(k);
    }
    assert!(problems(&v110).is_empty());
    let mut v100 = v110.clone();
    v100.as_object_mut().unwrap().remove("trackingQuality");
    v100["schemaVersion"] = json!("1.0.0");
    assert!(problems(&v100).is_empty());
}

#[test]
fn range_refusals_name_the_member_and_the_range() {
    let mut body = default_body();
    set(&mut body, "metrics.ivtVelocityUnitsPerSec", json!(60.0));
    set(&mut body, "aoi.marginNorm", json!(-0.2));
    set(&mut body, "norms.minN", json!(3));
    set(&mut body, "flag.greenFloor", json!(1.5));
    set(&mut body, "metrics.ivtUnit", json!("pixels"));
    set(&mut body, "norms.includeImputed", json!("yes"));
    set(&mut body, "aoi.dwellCapMs", json!(100.5));
    let got = problems(&body);
    assert_eq!(
        got,
        [
            ("flag.greenFloor", "flag.greenFloor must be in [0, 1]"),
            (
                "metrics.ivtVelocityUnitsPerSec",
                "metrics.ivtVelocityUnitsPerSec must be in [0.5, 50]"
            ),
            (
                "metrics.ivtUnit",
                "metrics.ivtUnit must be one of: norm, deg"
            ),
            ("aoi.marginNorm", "aoi.marginNorm must be in [-0.1, 0.1]"),
            ("aoi.dwellCapMs", "aoi.dwellCapMs must be an integer"),
            ("norms.minN", "norms.minN must be in [5, 100000]"),
            (
                "norms.includeImputed",
                "norms.includeImputed must be true or false"
            ),
        ]
        .map(|(p, m)| (p.to_string(), m.to_string()))
    );
}

#[test]
fn point_lists_and_tracking_members_are_checked() {
    let mut body = default_body();
    set(
        &mut body,
        "trackingQuality.calibration.layout5",
        json!([[0.5, 1.5]]),
    );
    set(&mut body, "trackingQuality.drift.checkLayout", json!([]));
    set(&mut body, "trackingQuality.filter.beta", json!(2.0));
    assert_eq!(
        paths(&body),
        [
            "trackingQuality.calibration.layout5",
            "trackingQuality.drift.checkLayout",
            "trackingQuality.filter.beta",
        ]
    );
}

#[test]
fn cross_field_rules_follow_the_registry_without_duplicates() {
    // Both values in range, but yellow above green: the cross-field rule reports it.
    let mut body = default_body();
    set(&mut body, "flag.yellowFloor", json!(0.9));
    assert_eq!(
        problems(&body),
        [(
            "flag.yellowFloor".to_string(),
            "flag.yellowFloor must not exceed flag.greenFloor".to_string()
        )]
    );
    // drift.warnDeg above hardDeg (asd-core TrackingQualityParams::validate).
    let mut body = default_body();
    set(&mut body, "trackingQuality.drift.warnDeg", json!(4.0));
    assert_eq!(paths(&body), ["trackingQuality.drift.warnDeg"]);
    // Out of range in the registry and in the legacy unit check: reported once.
    let mut body = default_body();
    set(&mut body, "lowConfidenceValidRatio", json!(2.0));
    assert_eq!(paths(&body), ["lowConfidenceValidRatio"]);
}

#[test]
fn age_bands_must_be_contiguous_ascending_and_bounded() {
    let mut body = default_body();
    body["ageBands"] = json!([
        {"id": "a", "minMonths": 12, "maxMonths": 23},
        {"id": "b", "minMonths": 30, "maxMonths": 47}
    ]);
    let got = problems(&body);
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].0, "ageBands.1");
    assert!(got[0].1.contains("contiguous"), "{}", got[0].1);

    body["ageBands"] = json!([{"id": "a", "minMonths": 0, "maxMonths": 300}]);
    assert_eq!(paths(&body), ["ageBands.0"]);
    body["ageBands"] = json!([
        {"id": "a", "minMonths": 0, "maxMonths": 11},
        {"id": "a", "minMonths": 12, "maxMonths": 23}
    ]);
    assert_eq!(paths(&body), ["ageBands.1"]);
    body["ageBands"] = json!([]);
    assert_eq!(
        problems(&body),
        [(
            "ageBands".to_string(),
            "ageBands must have 1 to 12 bands".to_string()
        )]
    );
    // Other bands are allowed (a researcher may test them, COH-3).
    body["ageBands"] = json!([
        {"id": "0-23", "minMonths": 0, "maxMonths": 23},
        {"id": "24-71", "minMonths": 24, "maxMonths": 71}
    ]);
    assert!(problems(&body).is_empty());
}

#[test]
fn unknown_members_under_registered_objects_are_refused() {
    let mut body = default_body();
    body["qualityGate"]["maxRollRad"] = json!(0.3);
    body["trackingQuality"]["filter"]["gamma"] = json!(1);
    body["futureSection"] = json!({"bands": {}});
    assert_eq!(
        paths(&body),
        [
            "futureSection",
            "qualityGate.maxRollRad",
            "trackingQuality.filter.gamma"
        ]
    );
    assert!(problems(&json!([1, 2]))[0].1.contains("JSON object"));
}

#[test]
fn typed_profile_problems_match_the_json_validation() {
    let mut p = ScoringProfile::default();
    p.metrics.min_fixation_duration_sec = 5.0;
    p.flag.yellow_floor = 0.9;
    let typed: Vec<String> = profile_problems(&p).into_iter().map(|x| x.path).collect();
    assert_eq!(
        typed,
        ["metrics.minFixationDurationSec", "flag.yellowFloor"]
    );
    // The legacy rule set is unchanged (callers not yet on the registry).
    assert_eq!(
        scoring_profile_problems(&p)
            .into_iter()
            .map(|x| x.path)
            .collect::<Vec<_>>(),
        ["flag.yellowFloor"]
    );
}

// ---- PRM-7: profile 1.2.0 ----------------------------------------------------------------

#[test]
fn profile_1_2_0_defaults_are_todays_constants() {
    assert_eq!(SCORING_PROFILE_SCHEMA_VERSION, "1.3.0");
    let p = ScoringProfile::default();
    assert_eq!(p.metrics.ivt_velocity_units_per_sec, 5.0);
    assert_eq!(
        p.metrics.ivt_velocity_units_per_sec,
        asd_core::metrics::VELOCITY_THRESHOLD_UNITS_PER_SEC
    );
    assert_eq!(p.metrics.ivt_velocity_deg_per_sec, 30.0);
    assert_eq!(p.metrics.ivt_unit, IvtUnit::Norm);
    assert_eq!(
        p.metrics.min_fixation_duration_sec,
        asd_core::metrics::MIN_FIXATION_DURATION_SEC
    );
    assert_eq!(
        p.metrics.fallback_frame_interval_sec,
        asd_core::metrics::FRAME_INTERVAL_SEC
    );
    assert_eq!(p.metrics.smile_reactivity_window_sec, 0.5);
    assert_eq!(p.metrics.flat_affect_gain, 50.0);
    assert_eq!(p.aoi.margin_norm, 0.0);
    assert_eq!(p.aoi.dwell_cap_ms, 100);
    // The norm brackets, plus the 1.3.0 representative months (DSP-2).
    let bare: Vec<_> = p
        .age_bands
        .iter()
        .map(|b| asd_core::types::AgeBracket::new(b.id.clone(), b.min_months, b.max_months))
        .collect();
    assert_eq!(bare, asd_core::norms::default_age_brackets());
    assert_eq!(p.norms.min_n as usize, asd_core::norms::DEFAULT_MIN_N);
    assert!(!p.norms.include_imputed);
    assert_eq!(p.review.low_quality_valid_ratio, 0.80);
    assert_eq!(
        p.low_confidence_valid_ratio,
        asd_core::scoring::LOW_CONFIDENCE_VALID_RATIO
    );
}

#[test]
fn a_1_1_0_profile_decodes_with_the_1_2_0_defaults() {
    let mut v = default_body();
    for k in [
        "metrics",
        "aoi",
        "ageBands",
        "norms",
        "review",
        "display",
        "sessionQuality",
    ] {
        v.as_object_mut().unwrap().remove(k);
    }
    v["schemaVersion"] = json!("1.1.0");
    let p: ScoringProfile = serde_json::from_value(v).unwrap();
    assert_eq!(p.schema_version, "1.1.0");
    let d = ScoringProfile::default();
    assert_eq!(p.metrics, d.metrics);
    assert_eq!(p.aoi, d.aoi);
    assert_eq!(p.age_bands, d.age_bands);
    assert_eq!(p.norms, d.norms);
    assert_eq!(p.review, d.review);
    assert_eq!(p.tracking_quality, TrackingQualityParams::default());

    // A partial section fills the rest from the defaults.
    let mut v = default_body();
    v["metrics"] = json!({"ivtVelocityUnitsPerSec": 2.0});
    let p: ScoringProfile = serde_json::from_value(v).unwrap();
    assert_eq!(p.metrics.ivt_velocity_units_per_sec, 2.0);
    assert_eq!(p.metrics.min_fixation_duration_sec, 0.08);
}

#[test]
fn gaze_processing_view_borrows_the_four_sections() {
    let p = ScoringProfile::default();
    let view = p.gaze_processing();
    assert!(std::ptr::eq(view.metrics, &p.metrics));
    assert!(std::ptr::eq(view.aoi, &p.aoi));
    assert!(std::ptr::eq(view.quality_gate, &p.quality_gate));
    assert!(std::ptr::eq(view.tracking_quality, &p.tracking_quality));
}

// ---- PRM-4: export -----------------------------------------------------------------------

#[test]
fn the_registry_is_exported_with_its_id_and_matches_the_committed_file() {
    let v = params::registry_json();
    assert_eq!(v["$id"], PARAM_REGISTRY_ID);
    assert_eq!(v["registryVersion"], REGISTRY_VERSION);
    assert_eq!(
        v["params"].as_array().unwrap().len(),
        registry().len(),
        "every row exported"
    );
    let doc = schema::documents()
        .into_iter()
        .find(|d| d.file_name == PARAM_REGISTRY_FILE)
        .expect("exported by `asd-cli schema export`");
    assert_eq!(doc.value, v);
    // The `ParamRegistry` definition in api-v1.json describes the document.
    let api = schema::api_v1();
    let mut def = api.clone();
    def["$ref"] = json!("#/$defs/ParamRegistry");
    let mut instance = v.clone();
    instance.as_object_mut().unwrap().remove("$id");
    schema::validate(&def, &instance).expect("registry validates against ParamRegistry");
}

#[test]
fn bundle_schema_carries_the_1_2_0_sections() {
    let schema = schema::config_bundle_v1();
    let profile = &schema["$defs"]["ScoringProfile"];
    for k in ["metrics", "aoi", "ageBands", "norms", "review"] {
        assert!(profile["properties"].get(k).is_some(), "{k}");
        assert!(
            !profile["required"].as_array().unwrap().contains(&json!(k)),
            "{k} is optional (serde default)"
        );
    }
}

// ---- DSP-1, DSP-2, DSP-6: profile 1.3.0 (WP-X.0c) ---------------------------------------

const DSP_ROWS: [(&str, f64); 16] = [
    ("display.bands.socialPreferencePct.okMin", 60.0),
    ("display.bands.socialPreferencePct.monitorMin", 50.0),
    ("display.bands.eyeDwellPct.okMin", 25.0),
    ("display.bands.eyeDwellPct.monitorMin", 18.0),
    ("display.bands.mouthDwellPct.okMin", 10.0),
    ("display.bands.mouthDwellPct.okMax", 30.0),
    ("display.bands.firstEyeLatencyMs.okMax", 550.0),
    ("display.bands.firstEyeLatencyMs.monitorMax", 750.0),
    ("display.bands.smileIntensityPct.okMin", 20.0),
    ("display.bands.smileIntensityPct.monitorMin", 10.0),
    ("display.bands.smileReactivity.okMin", 0.30),
    ("display.bands.surprisePct.okMin", 15.0),
    ("display.bands.flatAffect.okMax", 0.25),
    ("display.scanpathMaxGapMs", 120.0),
    ("sessionQuality.optimalMinValidRatio", 0.75),
    ("sessionQuality.acceptableMinValidRatio", 0.50),
];

#[test]
fn the_16_rows_of_1_3_0_are_registered_with_their_defaults() {
    assert_eq!(registry().len(), 88 + 16, "acceptance 12: 104 rows");
    let added: Vec<&str> = registry()
        .iter()
        .filter(|s| s.since == "1.3.0")
        .map(|s| s.id.as_str())
        .collect();
    assert_eq!(added, DSP_ROWS.map(|(id, _)| id));
    for (id, default) in DSP_ROWS {
        let spec = params::spec(id).unwrap();
        assert_eq!(spec.default.as_f64(), Some(default), "{id}");
        assert_eq!(spec.reprocess, Reprocess::Score, "{id}");
        assert!(spec.owner_gate.is_none(), "{id}");
        let want_stage = if id.starts_with("display.") {
            Stage::Display
        } else {
            Stage::Metrics
        };
        assert_eq!(spec.stage, want_stage, "{id}");
    }
    assert!(matches!(
        params::spec("display.bands.firstEyeLatencyMs.okMax")
            .unwrap()
            .kind,
        ParamKind::Number {
            min: 50.0,
            max: 5000.0,
            step: 10.0,
            unit: Unit::Millis
        }
    ));
    assert!(matches!(
        params::spec("display.scanpathMaxGapMs").unwrap().kind,
        ParamKind::Integer {
            min: 16,
            max: 1000,
            unit: Unit::Millis
        }
    ));
    assert_eq!(REGISTRY_VERSION, "1.0.0", "rows added, none changed");
}

#[test]
fn a_1_2_0_profile_decodes_with_the_1_3_0_defaults() {
    let mut v = default_body();
    for k in ["display", "sessionQuality"] {
        v.as_object_mut().unwrap().remove(k);
    }
    for band in v["ageBands"].as_array_mut().unwrap() {
        band.as_object_mut().unwrap().remove("representativeMonths");
    }
    v["schemaVersion"] = json!("1.2.0");
    assert!(problems(&v).is_empty());
    let p: ScoringProfile = serde_json::from_value(v).unwrap();
    let d = ScoringProfile::default();
    assert_eq!(p.display, d.display);
    assert_eq!(p.session_quality, d.session_quality);
    // Every older section is unchanged.
    assert_eq!(
        (&p.flag, &p.metrics, &p.aoi, &p.norms, &p.review),
        (&d.flag, &d.metrics, &d.aoi, &d.norms, &d.review)
    );
    // The bands carry no representative months; the effective ages are still today's.
    assert!(p
        .age_bands
        .iter()
        .all(|b| b.representative_months.is_none()));
    let months: Vec<i64> = p
        .age_bands
        .iter()
        .map(asd_contract::config::representative_months)
        .collect();
    assert_eq!(months, [14, 20, 28, 40, 54]);

    // A partial band object fills the rest from the defaults.
    let mut v = default_body();
    v["display"] = json!({"bands": {"eyeDwellPct": {"okMin": 30}}});
    let p: ScoringProfile = serde_json::from_value(v).unwrap();
    assert_eq!(p.display.bands.eye_dwell_pct.ok_min, 30.0);
    assert_eq!(p.display.bands.eye_dwell_pct.monitor_min, 18.0);
    assert_eq!(p.display.scanpath_max_gap_ms, 120);
}

#[test]
fn display_and_session_quality_ranges_and_orders_are_refused() {
    let mut body = default_body();
    set(&mut body, "display.bands.eyeDwellPct.okMin", json!(120));
    assert_eq!(
        problems(&body),
        [(
            "display.bands.eyeDwellPct.okMin".to_string(),
            "display.bands.eyeDwellPct.okMin must be in [0, 100]".to_string()
        )]
    );

    // monitorMin above okMin (both in range): the cross-field rule.
    let mut body = default_body();
    set(&mut body, "display.bands.eyeDwellPct.monitorMin", json!(30));
    assert_eq!(
        problems(&body),
        [(
            "display.bands.eyeDwellPct.monitorMin".to_string(),
            "display.bands.eyeDwellPct.monitorMin must not exceed okMin".to_string()
        )]
    );

    // Acceptance 12: latency okMax 900 above monitorMax 750.
    let mut body = default_body();
    set(
        &mut body,
        "display.bands.firstEyeLatencyMs.okMax",
        json!(900),
    );
    assert_eq!(paths(&body), ["display.bands.firstEyeLatencyMs.okMax"]);

    let mut body = default_body();
    set(&mut body, "display.bands.mouthDwellPct.okMin", json!(40));
    set(
        &mut body,
        "display.bands.smileIntensityPct.monitorMin",
        json!(25),
    );
    set(
        &mut body,
        "display.bands.socialPreferencePct.monitorMin",
        json!(65),
    );
    set(
        &mut body,
        "sessionQuality.acceptableMinValidRatio",
        json!(0.8),
    );
    set(&mut body, "display.scanpathMaxGapMs", json!(5));
    assert_eq!(
        paths(&body),
        [
            "display.scanpathMaxGapMs",
            "display.bands.socialPreferencePct.monitorMin",
            "display.bands.mouthDwellPct.okMin",
            "display.bands.smileIntensityPct.monitorMin",
            "sessionQuality.acceptableMinValidRatio",
        ]
    );
}

#[test]
fn representative_months_must_lie_in_their_band() {
    let mut body = default_body();
    assert_eq!(body["ageBands"][2]["representativeMonths"], 28);
    // Acceptance 12: 40 is outside band 24-35.
    body["ageBands"][2]["representativeMonths"] = json!(40);
    assert_eq!(
        problems(&body),
        [(
            "ageBands.2".to_string(),
            "ageBands.2.representativeMonths must be an integer in [24, 35]".to_string()
        )]
    );
    body["ageBands"][2]["representativeMonths"] = json!(27.5);
    assert_eq!(paths(&body), ["ageBands.2"]);
    // At either bound, or absent, is fine.
    body["ageBands"][2]["representativeMonths"] = json!(35);
    assert!(problems(&body).is_empty());
    body["ageBands"][2]["representativeMonths"] = json!(24);
    assert!(problems(&body).is_empty());
    body["ageBands"][2]
        .as_object_mut()
        .unwrap()
        .remove("representativeMonths");
    assert!(problems(&body).is_empty());
}

#[test]
fn device_validation_ignores_unknown_members_but_not_known_ranges() {
    let mut body = default_body();
    body["futureSection"] = json!({"knob": 1});
    body["display"]["bands"]["newMetric"] = json!({"okMin": 1});
    let device = |b: &Value| -> Vec<String> {
        validate_for_device(DocumentKind::ScoringProfile, b)
            .into_iter()
            .map(|p| p.path)
            .collect()
    };
    assert!(device(&body).is_empty());
    assert_eq!(
        params::unknown_parameters(DocumentKind::ScoringProfile, &body),
        ["display.bands.newMetric", "futureSection"]
    );
    // Publish stays strict.
    assert_eq!(paths(&body), ["display.bands.newMetric", "futureSection"]);

    // A known member out of range, and a cross-field rule, still fail on the device.
    set(&mut body, "display.bands.eyeDwellPct.okMin", json!(120));
    assert_eq!(device(&body), ["display.bands.eyeDwellPct.okMin"]);
    let mut body = default_body();
    body["futureSection"] = json!(true);
    set(&mut body, "flag.yellowFloor", json!(0.9));
    assert_eq!(device(&body), ["flag.yellowFloor"]);
    // The default profile and an older one pass on the device too.
    assert!(device(&default_body()).is_empty());
}
