//! owner: WP-G.4
//!
//! Session `TrackingQuality` summary (QS-1), `estimatedAccuracyDeg` (QS-2) and the verdict
//! (QS-4). Optional values must be absent keys, never defaults (CON-14).

use std::path::PathBuf;

use asd_core::gaze::summary::{
    conditions, estimated_accuracy_deg, sample_rate_hz, tracking_quality, verdict, SummaryInput,
    VerdictInput,
};
use asd_core::gaze::PIPELINE_VERSION;
use asd_core::types::tracking::{
    CalibrationQuality, CalibrationRecord, ConfidenceSummary, DriftCheckRecord, GeometrySource,
    LossSummary, Orientation, ProcessedSample, TrackState, TrackingConditions, TrackingDevice,
    TrackingQualityParams, TrackingVerdict, ValidationRecord, VerdictParams,
    TRACKING_PARAMS_SCHEMA_VERSION,
};
use serde_json::Value;

fn validation(acc: Option<f64>) -> ValidationRecord {
    ValidationRecord {
        complete: acc.is_some(),
        targets: Vec::new(),
        accuracy_deg: acc,
        precision_rms_deg: None,
        precision_rms_raw_deg: None,
        data_loss: 0.0,
    }
}

fn chk(offset: Option<f64>) -> DriftCheckRecord {
    DriftCheckRecord {
        at_sec: 0.0,
        x_norm: 0.5,
        y_norm: 0.5,
        offset_deg: offset,
        drift_deg: None,
        missed: offset.is_none(),
    }
}

// ---- QS-2: the four availability cases ------------------------------------------------

#[test]
fn qs2_validation_and_checks_takes_max_of_validation_and_check_median() {
    let v = validation(Some(2.0));
    // Check median of 1.0, 3.0, 4.0 = 3.0 > 2.0.
    let c = vec![chk(Some(1.0)), chk(None), chk(Some(4.0)), chk(Some(3.0))];
    assert_eq!(estimated_accuracy_deg(Some(&v), &c), Some(3.0));
    // Check median below validation: validation wins.
    let c = vec![chk(Some(1.0)), chk(Some(1.5))];
    assert_eq!(estimated_accuracy_deg(Some(&v), &c), Some(2.0));
    // A single hit check counts when validation exists.
    let c = vec![chk(Some(2.5))];
    assert_eq!(estimated_accuracy_deg(Some(&v), &c), Some(2.5));
}

#[test]
fn qs2_validation_without_hit_checks_is_validation() {
    let v = validation(Some(2.2));
    assert_eq!(estimated_accuracy_deg(Some(&v), &[]), Some(2.2));
    assert_eq!(
        estimated_accuracy_deg(Some(&v), &[chk(None), chk(None)]),
        Some(2.2)
    );
}

#[test]
fn qs2_no_validation_needs_two_hit_checks() {
    let two = vec![chk(Some(1.0)), chk(None), chk(Some(2.0))];
    assert_eq!(estimated_accuracy_deg(None, &two), Some(1.5));
    // A validation record without an accuracy (no target hit) counts as no validation.
    let v = validation(None);
    assert_eq!(estimated_accuracy_deg(Some(&v), &two), Some(1.5));
}

#[test]
fn qs2_otherwise_absent() {
    assert_eq!(estimated_accuracy_deg(None, &[]), None);
    assert_eq!(
        estimated_accuracy_deg(None, &[chk(Some(1.0)), chk(None)]),
        None
    );
    assert_eq!(
        estimated_accuracy_deg(Some(&validation(None)), &[chk(Some(1.0))]),
        None
    );
}

// ---- QS-4: verdict at each boundary ---------------------------------------------------

fn vin(acc: Option<f64>) -> VerdictInput {
    VerdictInput {
        estimated_accuracy_deg: acc,
        valid_ratio: 0.9,
        data_loss: 0.0,
        geometry_source: GeometrySource::Table,
        calibration_quality: CalibrationQuality::Good,
    }
}

#[test]
fn qs4_accuracy_bands_inclusive_upper_bounds() {
    let p = VerdictParams::default();
    let cases = [
        (Some(0.0), TrackingVerdict::Good),
        (Some(2.5), TrackingVerdict::Good),
        (Some(2.5001), TrackingVerdict::Fair),
        (Some(3.5), TrackingVerdict::Fair),
        (Some(3.5001), TrackingVerdict::Poor),
        (Some(5.0), TrackingVerdict::Poor),
        (Some(5.0001), TrackingVerdict::Unusable),
        (None, TrackingVerdict::Unusable),
        (Some(f64::NAN), TrackingVerdict::Unusable),
    ];
    for (acc, want) in cases {
        assert_eq!(verdict(&vin(acc), &p), want, "{acc:?}");
    }
}

#[test]
fn qs4_valid_ratio_boundary() {
    let p = VerdictParams::default();
    let at = VerdictInput {
        valid_ratio: 0.50,
        ..vin(Some(1.0))
    };
    assert_eq!(verdict(&at, &p), TrackingVerdict::Good);
    let below = VerdictInput {
        valid_ratio: 0.4999,
        ..vin(Some(1.0))
    };
    assert_eq!(verdict(&below, &p), TrackingVerdict::Unusable);
}

#[test]
fn qs4_data_loss_boundaries() {
    let p = VerdictParams::default();
    let with = |dl: f64| VerdictInput {
        data_loss: dl,
        ..vin(Some(1.0))
    };
    assert_eq!(verdict(&with(0.20), &p), TrackingVerdict::Good);
    assert_eq!(verdict(&with(0.2001), &p), TrackingVerdict::Fair);
    assert_eq!(verdict(&with(0.40), &p), TrackingVerdict::Fair);
    assert_eq!(verdict(&with(0.4001), &p), TrackingVerdict::Poor);
    assert_eq!(verdict(&with(f64::NAN), &p), TrackingVerdict::Poor);
}

#[test]
fn qs4_idiom_default_and_skipped_cap_at_fair() {
    let p = VerdictParams::default();
    let idiom = VerdictInput {
        geometry_source: GeometrySource::IdiomDefault,
        ..vin(Some(1.0))
    };
    assert_eq!(verdict(&idiom, &p), TrackingVerdict::Fair);
    let skipped = VerdictInput {
        calibration_quality: CalibrationQuality::Skipped,
        ..vin(Some(1.0))
    };
    assert_eq!(verdict(&skipped, &p), TrackingVerdict::Fair);
    // A cap never improves a worse verdict.
    let poor = VerdictInput {
        geometry_source: GeometrySource::IdiomDefault,
        ..vin(Some(4.0))
    };
    assert_eq!(verdict(&poor, &p), TrackingVerdict::Poor);
}

// ---- conditions and sample rate -------------------------------------------------------

fn s(t: f64, d: f64, speed: f64, ambient: Option<f64>, state: TrackState) -> ProcessedSample {
    ProcessedSample {
        t_sec: t,
        track_state: state,
        ray_status: None,
        raw_point_norm: None,
        calibrated_point_norm: None,
        filtered_point_norm: None,
        eye_pos_mm: None,
        distance_cm: Some(d),
        head_yaw_rad: None,
        head_pitch_rad: None,
        head_roll_rad: None,
        head_speed_deg_per_sec: Some(speed),
        eye_speed_deg_per_sec: None,
        left_eye_openness: None,
        right_eye_openness: None,
        ambient_intensity: ambient,
        confidence: 1.0,
        valid: true,
        gap_filled: false,
        active_aoi_names: Vec::new(),
        stimulus_id: None,
        clip_media_time_sec: None,
    }
}

#[test]
fn conditions_over_tracked_frames() {
    // Tracked distances 40, 45, 50, 55, 60: median 50, p25 45, p75 55 -> IQR 10.
    // Head speeds 0, 10, 20, 30, 40: p90 at index 3.6 -> 30 + 0.6 * 10 = 36.
    let v = vec![
        s(0.0, 40.0, 0.0, Some(800.0), TrackState::Tracked),
        s(0.1, 45.0, 10.0, Some(1000.0), TrackState::Tracked),
        s(0.2, 99.0, 500.0, Some(5.0), TrackState::Blink),
        s(0.3, 50.0, 20.0, None, TrackState::Tracked),
        s(0.4, 55.0, 30.0, Some(1200.0), TrackState::Tracked),
        s(0.5, 60.0, 40.0, None, TrackState::Tracked),
    ];
    let c = conditions(&v, Some(true));
    assert_eq!(c.distance_cm_median, Some(50.0));
    assert_eq!(c.distance_cm_iqr, Some(10.0));
    assert!((c.head_speed_deg_per_sec_p90.unwrap() - 36.0).abs() < 1e-12);
    assert_eq!(c.ambient_intensity_median, Some(1000.0));
    assert_eq!(c.light_asymmetry, None);
    assert_eq!(c.glasses, Some(true));
}

#[test]
fn conditions_without_tracked_frames_are_all_absent() {
    let v = vec![s(0.0, 50.0, 1.0, Some(900.0), TrackState::Lost)];
    let c = conditions(&v, None);
    assert_eq!(c, TrackingConditions::default());
    assert_eq!(serde_json::to_value(&c).unwrap(), serde_json::json!({}));
}

#[test]
fn sample_rate_from_median_interval() {
    // Intervals 1/60 x3 and one 0.1 s hiccup: median 1/60 -> 60 Hz.
    let ts = [
        0.0,
        1.0 / 60.0,
        2.0 / 60.0,
        2.0 / 60.0 + 0.1,
        3.0 / 60.0 + 0.1,
    ];
    let v: Vec<_> = ts
        .iter()
        .map(|&t| s(t, 50.0, 0.0, None, TrackState::Lost))
        .collect();
    assert!((sample_rate_hz(&v).unwrap() - 60.0).abs() < 1e-6);
    assert_eq!(sample_rate_hz(&v[..1]), None);
    assert_eq!(sample_rate_hz(&[]), None);
}

// ---- QS-1 assembly --------------------------------------------------------------------

fn input(validation: Option<ValidationRecord>, checks: Vec<DriftCheckRecord>) -> SummaryInput {
    SummaryInput {
        device: TrackingDevice {
            model_identifier: "iPhone17,3".to_string(),
            geometry_source: GeometrySource::Table,
            orientation: Orientation::Portrait,
            screen_width_mm: 64.0,
            screen_height_mm: 139.0,
            sample_rate_hz: Some(60.0),
        },
        calibration: CalibrationRecord {
            model: None,
            layout_points: 5,
            usable_points: 0,
            missing_points: 0,
            residual_deg: None,
            loo_cv_deg: None,
            condition_number: None,
            quality: CalibrationQuality::Good,
            pose: None,
            age_sec_at_start: None,
            attempts: 1,
        },
        validation,
        drift_checks: checks,
        recalibrations: Vec::new(),
        conditions: TrackingConditions::default(),
        loss: LossSummary::default(),
        confidence: ConfidenceSummary {
            mean_sample: 0.9,
            mean_fixation: None,
            low_confidence_sample_ratio: 0.05,
        },
        valid_ratio: 0.9,
        data_loss: 0.1,
    }
}

#[test]
fn summary_fills_versions_estimate_verdict_and_max_drift() {
    let mut checks = vec![chk(Some(2.0)), chk(Some(3.0))];
    checks[0].drift_deg = Some(0.5);
    checks[1].drift_deg = Some(1.25);
    let tq = tracking_quality(
        input(Some(validation(Some(1.0))), checks),
        &TrackingQualityParams::default(),
    );
    assert_eq!(tq.pipeline_version, PIPELINE_VERSION);
    assert_eq!(tq.params_version, TRACKING_PARAMS_SCHEMA_VERSION);
    assert_eq!(tq.estimated_accuracy_deg, Some(2.5));
    assert_eq!(tq.max_drift_deg, Some(1.25));
    assert_eq!(tq.verdict, TrackingVerdict::Good);
}

#[test]
fn unmeasured_summary_fields_are_absent_keys() {
    let tq = tracking_quality(input(None, Vec::new()), &TrackingQualityParams::default());
    assert_eq!(tq.estimated_accuracy_deg, None);
    assert_eq!(tq.verdict, TrackingVerdict::Unusable);
    let json = serde_json::to_value(&tq).unwrap();
    for key in ["estimatedAccuracyDeg", "validation", "maxDriftDeg"] {
        assert!(json.get(key).is_none(), "{key} must be absent: {json}");
    }
    let cal = &json["calibration"];
    for key in [
        "model",
        "residualDeg",
        "looCvDeg",
        "conditionNumber",
        "pose",
    ] {
        assert!(cal.get(key).is_none(), "calibration.{key} must be absent");
    }
    assert!(json["confidence"].get("meanFixation").is_none());
    assert_eq!(json["conditions"], serde_json::json!({}));
    assert!(!json.to_string().contains("null"), "no nulls: {json}");
}

#[test]
fn summary_uses_profile_verdict_params() {
    let mut params = TrackingQualityParams::default();
    params.verdict.accuracy_bands.good_max_deg = 0.5;
    let tq = tracking_quality(input(Some(validation(Some(1.0))), Vec::new()), &params);
    assert_eq!(tq.verdict, TrackingVerdict::Fair);
}

// ---- goldens: tests/golden/gaze/summary/*.json ----------------------------------------

#[test]
fn golden_summaries() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden/gaze/summary");
    let mut files: Vec<_> = std::fs::read_dir(&root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    files.sort();
    assert!(files.len() >= 2);
    for path in files {
        let name = path.display().to_string();
        let doc: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(doc["function"], "summary::tracking_quality", "{name}");
        let params: TrackingQualityParams =
            serde_json::from_value(doc["input"]["params"].clone()).unwrap();
        let inp: SummaryInput = serde_json::from_value(doc["input"]["summary"].clone())
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        let actual = serde_json::to_value(tracking_quality(inp, &params)).unwrap();
        assert_eq!(actual, doc["expected"], "{name}");
    }
}
