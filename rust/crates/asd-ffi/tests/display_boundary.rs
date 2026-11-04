//! owner: WP-X.0c
//!
//! DSP-5: the display records the kit decodes once per configuration.
//! - `display_params_from_profile(default) == defaults`, and a 1.2.0 profile (no 1.3.0
//!   sections) gives the same defaults; a changed leaf reaches the record.
//! - `age_bands_from_profile(default)` gives representative months 14, 20, 28, 40, 54, also
//!   for a 1.2.0 profile whose bands carry none.
//! - `display_band` / `session_quality_verdict` are the core decisions.
//! - Non-JSON is `AsdError::InvalidJson`; a malformed section gives its defaults.

use asd_ffi::{
    age_bands_from_profile, default_scoring_profile_json, display_band,
    display_params_from_profile, session_quality_verdict, AsdBandStatus, AsdComparator,
    AsdDisplayMetric, AsdDisplayParams, AsdError,
};
use serde_json::{json, Value};

fn default_profile() -> Value {
    serde_json::from_str(&default_scoring_profile_json()).unwrap()
}

fn params(profile: &Value) -> AsdDisplayParams {
    display_params_from_profile(profile.to_string()).unwrap()
}

#[test]
fn default_profile_round_trips_to_the_defaults() {
    let p = params(&default_profile());
    assert_eq!(p, AsdDisplayParams::defaults());
    assert_eq!(
        (
            p.social_preference_pct_ok_min,
            p.eye_dwell_pct_monitor_min,
            p.first_eye_latency_ms_monitor_max,
            p.flat_affect_ok_max,
            p.scanpath_max_gap_ms,
            p.optimal_min_valid_ratio,
            p.acceptable_min_valid_ratio,
            p.aoi_dwell_cap_ms,
        ),
        (60.0, 18.0, 750.0, 0.25, 120, 0.75, 0.50, 100)
    );
}

#[test]
fn a_1_2_0_profile_and_a_malformed_section_give_the_defaults() {
    let mut v120 = default_profile();
    for k in ["display", "sessionQuality"] {
        v120.as_object_mut().unwrap().remove(k);
    }
    v120["schemaVersion"] = json!("1.2.0");
    assert_eq!(params(&v120), AsdDisplayParams::defaults());

    let mut bad = default_profile();
    bad["display"] = json!("not an object");
    bad["sessionQuality"]["optimalMinValidRatio"] = json!("high");
    assert_eq!(params(&bad), AsdDisplayParams::defaults());

    for text in ["", "nope", "[1]"] {
        assert!(matches!(
            display_params_from_profile(text.into()),
            Err(AsdError::InvalidJson { .. })
        ));
        assert!(matches!(
            age_bands_from_profile(text.into()),
            Err(AsdError::InvalidJson { .. })
        ));
    }
}

#[test]
fn a_changed_leaf_reaches_the_record_and_the_band() {
    let mut v = default_profile();
    v["display"]["bands"]["eyeDwellPct"]["okMin"] = json!(30);
    v["aoi"]["dwellCapMs"] = json!(50);
    let p = params(&v);
    assert_eq!((p.eye_dwell_pct_ok_min, p.aoi_dwell_cap_ms), (30.0, 50));
    let b = display_band(AsdDisplayMetric::EyeDwellPct, Some(27.0), p.clone());
    assert_eq!(
        (b.status, b.comparator, b.low, b.high, b.unit.as_str()),
        (
            AsdBandStatus::Monitor,
            AsdComparator::AtLeast,
            Some(30.0),
            None,
            "percent"
        )
    );
    let d = AsdDisplayParams::defaults();
    assert_eq!(
        display_band(AsdDisplayMetric::EyeDwellPct, Some(27.0), d.clone()).status,
        AsdBandStatus::Ok
    );
    assert_eq!(
        display_band(AsdDisplayMetric::FirstEyeLatencyMs, None, d.clone()).status,
        AsdBandStatus::NotObserved
    );
    let lat = display_band(AsdDisplayMetric::FirstEyeLatencyMs, Some(550.0), d);
    assert_eq!(
        (lat.status, lat.comparator, lat.high),
        (AsdBandStatus::Monitor, AsdComparator::Below, Some(550.0))
    );
}

#[test]
fn session_quality_verdict_is_the_core_verdict_in_wire_spelling() {
    let d = AsdDisplayParams::defaults();
    let cases = [
        (0.75, false, "optimal"),
        (0.749, false, "acceptable"),
        (0.50, false, "acceptable"),
        (0.499, false, "degraded"),
        (0.75, true, "degraded"),
        (0.50, true, "degraded"),
    ];
    for (ratio, interrupted, want) in cases {
        assert_eq!(session_quality_verdict(ratio, interrupted, d.clone()), want);
    }
    let mut v = default_profile();
    v["sessionQuality"]["optimalMinValidRatio"] = json!(0.85);
    assert_eq!(
        session_quality_verdict(0.80, false, params(&v)),
        "acceptable"
    );
}

#[test]
fn age_bands_carry_the_representative_months() {
    let months = |profile: &Value| -> Vec<(String, i64)> {
        age_bands_from_profile(profile.to_string())
            .unwrap()
            .into_iter()
            .map(|b| (b.id, b.representative_months))
            .collect()
    };
    let want: Vec<(String, i64)> = [
        ("12-17", 14),
        ("18-23", 20),
        ("24-35", 28),
        ("36-47", 40),
        ("48-60", 54),
    ]
    .map(|(id, m)| (id.to_string(), m))
    .into();
    assert_eq!(months(&default_profile()), want);

    // A 1.2.0 profile: default bands without representativeMonths keep today's ages; no
    // ageBands at all gives the defaults.
    let mut v120 = default_profile();
    for band in v120["ageBands"].as_array_mut().unwrap() {
        band.as_object_mut().unwrap().remove("representativeMonths");
    }
    assert_eq!(months(&v120), want);
    v120.as_object_mut().unwrap().remove("ageBands");
    assert_eq!(months(&v120), want);

    // Other bands: the stated value, else the integer midpoint.
    let mut v = default_profile();
    v["ageBands"] = json!([
        {"id": "12-23", "minMonths": 12, "maxMonths": 23, "representativeMonths": 18},
        {"id": "24-47", "minMonths": 24, "maxMonths": 47},
        {"id": "48-60", "minMonths": 48, "maxMonths": 60}
    ]);
    assert_eq!(
        months(&v),
        [("12-23", 18), ("24-47", 35), ("48-60", 54)].map(|(id, m)| (id.to_string(), m))
    );
}
