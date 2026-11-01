//! WP-A.13 audit: `TrackingQualityParams::validate` refuses delivered values the pipeline
//! cannot run with (empty layouts, zero sample minimums, negative windows, an inverted
//! distance comfort zone, non-finite numbers). Paths are camelCase dotted, relative to
//! `trackingQuality`.

use asd_core::types::tracking::{BenchCondition, TrackingQualityParams};

fn paths(p: &TrackingQualityParams) -> Vec<String> {
    match p.validate() {
        Ok(()) => Vec::new(),
        Err(problems) => problems.into_iter().map(|p| p.path).collect(),
    }
}

#[test]
fn defaults_are_valid() {
    assert_eq!(TrackingQualityParams::default().validate(), Ok(()));
}

#[test]
fn empty_layouts_are_refused() {
    let mut p = TrackingQualityParams::default();
    p.calibration.layout5.clear();
    p.calibration.layout9.clear();
    p.validation.layout.clear();
    p.validation.bench_layout13.clear();
    p.drift.check_layout.clear();
    let got = paths(&p);
    for path in [
        "calibration.layout5",
        "calibration.layout9",
        "validation.layout",
        "validation.benchLayout13",
        "drift.checkLayout",
    ] {
        assert!(got.iter().any(|g| g == path), "{path} missing from {got:?}");
    }
}

#[test]
fn layout_points_must_be_finite_and_on_screen() {
    let mut p = TrackingQualityParams::default();
    p.calibration.layout5[1] = [f64::NAN, 0.5];
    p.validation.layout[0] = [1.5, 0.5];
    assert_eq!(paths(&p), ["calibration.layout5", "validation.layout"]);
}

#[test]
fn sample_minimums_must_be_at_least_one() {
    let mut p = TrackingQualityParams::default();
    p.calibration.min_samples_per_target = 0;
    p.validation.min_samples_per_target = 0;
    p.drift.min_samples = 0;
    assert_eq!(
        paths(&p),
        [
            "calibration.minSamplesPerTarget",
            "validation.minSamplesPerTarget",
            "drift.minSamples"
        ]
    );
}

#[test]
fn validation_needs_reachable_targets_hit() {
    let mut p = TrackingQualityParams::default();
    p.validation.min_targets_hit = 5; // layout has 4 points
    assert_eq!(paths(&p), ["validation.minTargetsHit"]);
    p.validation.min_targets_hit = 0;
    assert_eq!(paths(&p), ["validation.minTargetsHit"]);
}

#[test]
fn windows_must_be_non_negative_and_collect_positive() {
    let mut p = TrackingQualityParams::default();
    p.calibration.settle_ms = -1.0;
    p.validation.collect_ms = 0.0;
    p.drift.settle_ms = f64::NAN;
    assert_eq!(
        paths(&p),
        [
            "calibration.settleMs",
            "validation.collectMs",
            "drift.settleMs"
        ]
    );
}

#[test]
fn distance_comfort_zone_must_be_ordered() {
    let mut p = TrackingQualityParams::default();
    p.confidence.distance_comfort_min_cm = 70.0;
    assert_eq!(paths(&p), ["confidence.distanceComfortMinCm"]);
    let problems = p.validate().unwrap_err();
    assert!(problems[0].message.contains("distanceComfortMinCm"));
}

#[test]
fn every_number_must_be_finite() {
    let mut p = TrackingQualityParams::default();
    p.filter.beta = f64::INFINITY;
    p.gap_fill.max_ms = f64::NAN;
    p.recalibration.pose_window_sec = f64::NEG_INFINITY;
    p.scoring.aoi_resolution_factor = f64::NAN;
    let got = paths(&p);
    for path in [
        "filter.beta",
        "gapFill.maxMs",
        "recalibration.poseWindowSec",
        "scoring.aoiResolutionFactor",
    ] {
        assert!(got.iter().any(|g| g == path), "{path} missing from {got:?}");
    }
}

#[test]
fn bands_and_thresholds_must_be_ordered() {
    let mut p = TrackingQualityParams::default();
    p.calibration.bands.fair_max_deg = 2.0; // below good 2.5
    p.validation.warn_accuracy_deg = 6.0; // above max 5.0
    p.drift.warn_deg = 4.0; // above hard 3.0
    p.verdict.poor_above_data_loss = 1.5;
    assert_eq!(
        paths(&p),
        [
            "calibration.bands",
            "validation.warnAccuracyDeg",
            "drift.warnDeg",
            "verdict.poorAboveDataLoss"
        ]
    );
}

#[test]
fn schema_version_is_required() {
    let p = TrackingQualityParams {
        schema_version: " ".into(),
        ..Default::default()
    };
    assert_eq!(paths(&p), ["schemaVersion"]);
}

#[test]
fn bench_conditions_are_the_ben4_matrix_in_spec_order() {
    let names: Vec<&str> = BenchCondition::ALL.iter().map(|c| c.as_str()).collect();
    assert_eq!(
        names,
        [
            "baseline",
            "distance",
            "head",
            "motion",
            "glasses",
            "lowLight",
            "sideLight",
            "occlusion",
            "blinks"
        ]
    );
    for c in BenchCondition::ALL {
        assert_eq!(
            serde_json::to_value(c).unwrap(),
            serde_json::Value::String(c.as_str().to_owned())
        );
    }
}
