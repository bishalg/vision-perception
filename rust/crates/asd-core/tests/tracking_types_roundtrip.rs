//! owner: WP-G.1
//!
//! Every gaze-tracking type round-trips through JSON: keys are camelCase (snake_case for the
//! PIPE-2 raw sample line only), optional fields are absent keys and never `null`, and the
//! RFC 8785 (JCS) bytes are stable. Also the REP-1 raw log format and the SCH-3 defaults.

use asd_core::gaze::geometry::lookup;
use asd_core::gaze::raw_log::{self, RawLog, RawLogError};
use asd_core::gaze::PIPELINE_VERSION;
use asd_core::types::tracking::*;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value};
use std::fmt::Debug;

const RAW_FIXTURE: &str = include_str!("fixtures/raw_tracking_sample.jsonl");

#[derive(Clone, Copy, PartialEq)]
enum Casing {
    Camel,
    Snake,
}

/// Walks every object key (except inside opaque `groundTruth`) and every value.
fn check_keys(v: &Value, casing: Casing, path: &str) {
    match v {
        Value::Null => panic!("null at {path}: optional fields must be absent"),
        Value::Object(m) => {
            for (k, x) in m {
                match casing {
                    Casing::Camel => assert!(!k.contains('_'), "non-camelCase key {path}.{k}"),
                    Casing::Snake => assert!(
                        !k.chars().any(|c| c.is_ascii_uppercase()),
                        "non-snake_case key {path}.{k}"
                    ),
                }
                if k != "groundTruth" {
                    check_keys(x, casing, &format!("{path}.{k}"));
                }
            }
        }
        Value::Array(a) => a.iter().for_each(|x| check_keys(x, casing, path)),
        _ => {}
    }
}

fn roundtrip<T: Serialize + DeserializeOwned + PartialEq + Debug>(
    value: &T,
    casing: Casing,
) -> Value {
    let v = serde_json::to_value(value).expect("encode");
    check_keys(&v, casing, "$");
    let back: T = serde_json::from_value(v.clone()).expect("decode");
    assert_eq!(&back, value);
    let a = serde_jcs::to_vec(value).unwrap();
    let b = serde_jcs::to_vec(&back).unwrap();
    assert_eq!(a, b, "JCS bytes differ after round trip");
    v
}

fn keys(v: &Value) -> Vec<String> {
    v.as_object().unwrap().keys().cloned().collect()
}

fn m4(seed: f64) -> [f64; 16] {
    std::array::from_fn(|i| seed + i as f64)
}

fn full_sample() -> RawTrackingSample {
    RawTrackingSample {
        t_sec: 12.5,
        face_tracked: true,
        face_transform: Some(m4(0.5)),
        look_at: Some([0.01, -0.02, 0.3]),
        left_eye_transform: Some(m4(1.5)),
        right_eye_transform: Some(m4(2.5)),
        eye_blink_left: Some(0.1),
        eye_blink_right: Some(0.2),
        ambient_intensity: Some(950.0),
        ambient_color_temp: Some(5200.0),
        light_direction: Some([0.1, 0.2, -0.9]),
        orientation: Orientation::LandscapeLeft,
        clip_media_time_sec: Some(3.25),
        stimulus_id: Some("stim_05".into()),
    }
}

fn lost_sample() -> RawTrackingSample {
    RawTrackingSample {
        t_sec: 12.516,
        face_tracked: false,
        face_transform: None,
        look_at: None,
        left_eye_transform: None,
        right_eye_transform: None,
        eye_blink_left: None,
        eye_blink_right: None,
        ambient_intensity: None,
        ambient_color_temp: None,
        light_direction: None,
        orientation: Orientation::Portrait,
        clip_media_time_sec: None,
        stimulus_id: None,
    }
}

#[test]
fn raw_tracking_sample_is_snake_case_and_absent_not_null() {
    let v = roundtrip(&full_sample(), Casing::Snake);
    assert_eq!(
        keys(&v),
        [
            "ambient_color_temp",
            "ambient_intensity",
            "clip_media_time_sec",
            "eye_blink_left",
            "eye_blink_right",
            "face_tracked",
            "face_transform",
            "left_eye_transform",
            "light_direction",
            "look_at",
            "orientation",
            "right_eye_transform",
            "stimulus_id",
            "t_sec",
        ]
    );
    assert_eq!(v["orientation"], "landscapeLeft");
    let v = roundtrip(&lost_sample(), Casing::Snake);
    assert_eq!(keys(&v), ["face_tracked", "orientation", "t_sec"]);
}

#[test]
fn enums_use_spec_spellings() {
    let s = |v: Value| v.as_str().unwrap().to_string();
    assert_eq!(
        s(json!(Orientation::PortraitUpsideDown)),
        "portraitUpsideDown"
    );
    assert_eq!(s(json!(Orientation::LandscapeRight)), "landscapeRight");
    assert_eq!(s(json!(TrackState::Offscreen)), "offscreen");
    assert_eq!(s(json!(RayStatus::OutOfRange)), "outOfRange");
    assert_eq!(s(json!(GeometrySource::IdiomDefault)), "idiomDefault");
    assert_eq!(s(json!(ScreenSource::LegacyDefault)), "legacy-default");
    assert_eq!(s(json!(ScreenSource::Spec)), "spec");
    assert_eq!(s(json!(CameraOffsetSource::Unmeasured)), "unmeasured");
    assert_eq!(s(json!(CalModelKind::ScreenPoly2)), "screenPoly2");
    assert_eq!(s(json!(CalModelKind::AngularAffine)), "angularAffine");
    assert_eq!(s(json!(CalibrationQuality::Skipped)), "skipped");
    assert_eq!(s(json!(TrackingVerdict::Unusable)), "unusable");
    assert_eq!(s(json!(TargetKind::DriftCheck)), "driftCheck");
    assert_eq!(s(json!(TargetOutcome::NeedsRetry)), "needsRetry");
    assert_eq!(s(json!(RecalAction::Recalibrate)), "recalibrate");
    assert_eq!(
        s(json!(RecalReason::OrientationChanged)),
        "orientationChanged"
    );
    assert_eq!(s(json!(TrackingSource::ArkitFace)), "arkitFace");
}

#[test]
fn processed_sample_and_target_result_roundtrip() {
    let full = ProcessedSample {
        t_sec: 1.0,
        track_state: TrackState::Tracked,
        ray_status: Some(RayStatus::Ok),
        raw_point_norm: Some([-0.2, 0.4]),
        calibrated_point_norm: Some([0.1, 0.45]),
        filtered_point_norm: Some([0.11, 0.44]),
        eye_pos_mm: Some([30.0, 60.0, 450.0]),
        distance_cm: Some(45.0),
        head_yaw_rad: Some(0.1),
        head_pitch_rad: Some(-0.05),
        head_roll_rad: Some(0.01),
        head_speed_deg_per_sec: Some(12.0),
        eye_speed_deg_per_sec: Some(40.0),
        left_eye_openness: Some(0.9),
        right_eye_openness: Some(0.85),
        ambient_intensity: Some(800.0),
        confidence: 0.92,
        valid: true,
        gap_filled: false,
        active_aoi_names: vec!["Face".into()],
        stimulus_id: Some("s".into()),
        clip_media_time_sec: Some(2.0),
    };
    roundtrip(&full, Casing::Camel);
    let lost = ProcessedSample {
        t_sec: 1.016,
        track_state: TrackState::Lost,
        ray_status: None,
        raw_point_norm: None,
        calibrated_point_norm: None,
        filtered_point_norm: None,
        eye_pos_mm: None,
        distance_cm: None,
        head_yaw_rad: None,
        head_pitch_rad: None,
        head_roll_rad: None,
        head_speed_deg_per_sec: None,
        eye_speed_deg_per_sec: None,
        left_eye_openness: None,
        right_eye_openness: None,
        ambient_intensity: None,
        confidence: 0.0,
        valid: false,
        gap_filled: false,
        active_aoi_names: vec![],
        stimulus_id: None,
        clip_media_time_sec: None,
    };
    let v = roundtrip(&lost, Casing::Camel);
    assert_eq!(
        keys(&v),
        ["confidence", "gapFilled", "tSec", "trackState", "valid"]
    );

    let hit = TargetResult {
        kind: TargetKind::Validation,
        x_norm: 0.3,
        y_norm: 0.7,
        started_at_sec: 10.0,
        ended_at_sec: 11.4,
        attempt: 1,
        outcome: TargetOutcome::Hit,
        valid_samples: 55,
        total_samples: 60,
        data_loss: 0.08,
        raw_median_norm: Some([0.33, 0.66]),
        calibrated_median_norm: Some([0.31, 0.69]),
        eye_pos_mm_median: Some([35.0, 70.0, 450.0]),
        accuracy_deg: Some(1.2),
        offset_deg: Some([0.8, -0.9]),
        precision_rms_deg: Some(0.3),
        precision_rms_raw_deg: Some(0.6),
        sd_deg: Some(0.4),
    };
    roundtrip(&hit, Casing::Camel);
    let missed = TargetResult {
        outcome: TargetOutcome::Missed,
        raw_median_norm: None,
        calibrated_median_norm: None,
        eye_pos_mm_median: None,
        accuracy_deg: None,
        offset_deg: None,
        precision_rms_deg: None,
        precision_rms_raw_deg: None,
        sd_deg: None,
        ..hit
    };
    let v = roundtrip(&missed, Casing::Camel);
    assert!(v.get("accuracyDeg").is_none());

    roundtrip(&RecalAdvice::default(), Casing::Camel);
    roundtrip(
        &RecalAdvice {
            action: RecalAction::Recalibrate,
            reasons: vec![RecalReason::Drift, RecalReason::PoseShift],
            limit_reached: true,
        },
        Casing::Camel,
    );
}

fn full_quality() -> TrackingQuality {
    TrackingQuality {
        pipeline_version: PIPELINE_VERSION.into(),
        params_version: TRACKING_PARAMS_SCHEMA_VERSION.into(),
        device: TrackingDevice {
            model_identifier: "iPhone17,3".into(),
            geometry_source: GeometrySource::Table,
            orientation: Orientation::Portrait,
            screen_width_mm: 65.1,
            screen_height_mm: 141.1,
            sample_rate_hz: Some(60.0),
        },
        calibration: CalibrationRecord {
            model: Some(CalModelKind::ScreenAffine),
            layout_points: 5,
            usable_points: 5,
            missing_points: 0,
            residual_deg: Some(0.9),
            loo_cv_deg: Some(1.8),
            condition_number: Some(42.0),
            quality: CalibrationQuality::Good,
            pose: Some(CalibrationPose {
                distance_cm: 44.0,
                yaw_deg: 2.0,
                pitch_deg: -3.0,
            }),
            age_sec_at_start: Some(12.0),
            attempts: 1,
        },
        validation: Some(ValidationRecord {
            complete: true,
            targets: vec![
                ValidationTargetRecord {
                    x_norm: 0.3,
                    y_norm: 0.3,
                    accuracy_deg: Some(1.1),
                    precision_rms_deg: Some(0.2),
                    sd_deg: Some(0.3),
                    data_loss: 0.05,
                    valid_samples: 50,
                    missed: false,
                },
                ValidationTargetRecord {
                    x_norm: 0.7,
                    y_norm: 0.7,
                    accuracy_deg: None,
                    precision_rms_deg: None,
                    sd_deg: None,
                    data_loss: 0.9,
                    valid_samples: 3,
                    missed: true,
                },
            ],
            accuracy_deg: Some(1.1),
            precision_rms_deg: Some(0.2),
            precision_rms_raw_deg: Some(0.5),
            data_loss: 0.1,
        }),
        drift_checks: vec![
            DriftCheckRecord {
                at_sec: 40.0,
                x_norm: 0.5,
                y_norm: 0.5,
                offset_deg: Some(1.4),
                drift_deg: Some(0.6),
                missed: false,
            },
            DriftCheckRecord {
                at_sec: 55.0,
                x_norm: 0.3,
                y_norm: 0.3,
                offset_deg: None,
                drift_deg: None,
                missed: true,
            },
        ],
        max_drift_deg: Some(0.6),
        recalibrations: vec![RecalibrationRecord {
            at_sec: 70.0,
            reason: RecalReason::Drift,
            new_quality: CalibrationQuality::Fair,
        }],
        conditions: TrackingConditions {
            distance_cm_median: Some(45.0),
            distance_cm_iqr: Some(4.0),
            head_speed_deg_per_sec_p90: Some(20.0),
            ambient_intensity_median: Some(900.0),
            light_asymmetry: Some(0.1),
            glasses: Some(false),
        },
        loss: LossSummary {
            track_loss_sec: 1.0,
            blink_sec: 0.5,
            offscreen_sec: 0.2,
            artifact_sec: 0.1,
            gap_filled_sec: 0.05,
            longest_gap_sec: 0.8,
        },
        confidence: ConfidenceSummary {
            mean_sample: 0.8,
            mean_fixation: Some(0.85),
            low_confidence_sample_ratio: 0.1,
        },
        estimated_accuracy_deg: Some(1.4),
        verdict: TrackingVerdict::Good,
    }
}

#[test]
fn tracking_quality_full_roundtrip_has_qs1_keys() {
    let v = roundtrip(&full_quality(), Casing::Camel);
    assert_eq!(
        keys(&v),
        [
            "calibration",
            "conditions",
            "confidence",
            "device",
            "driftChecks",
            "estimatedAccuracyDeg",
            "loss",
            "maxDriftDeg",
            "paramsVersion",
            "pipelineVersion",
            "recalibrations",
            "validation",
            "verdict",
        ]
    );
    assert_eq!(v["pipelineVersion"], "asd-gaze/1.0.0");
    assert_eq!(
        keys(&v["calibration"]),
        [
            "ageSecAtStart",
            "attempts",
            "conditionNumber",
            "layoutPoints",
            "looCvDeg",
            "missingPoints",
            "model",
            "pose",
            "quality",
            "residualDeg",
            "usablePoints",
        ]
    );
    assert_eq!(
        keys(&v["loss"]),
        [
            "artifactSec",
            "blinkSec",
            "gapFilledSec",
            "longestGapSec",
            "offscreenSec",
            "trackLossSec",
        ]
    );
}

#[test]
fn tracking_quality_unmeasured_fields_are_absent() {
    let mut q = full_quality();
    q.validation = None;
    q.drift_checks.clear();
    q.max_drift_deg = None;
    q.recalibrations.clear();
    q.estimated_accuracy_deg = None;
    q.device.sample_rate_hz = None;
    q.calibration = CalibrationRecord {
        model: None,
        layout_points: 0,
        usable_points: 0,
        missing_points: 0,
        residual_deg: None,
        loo_cv_deg: None,
        condition_number: None,
        quality: CalibrationQuality::Skipped,
        pose: None,
        age_sec_at_start: None,
        attempts: 0,
    };
    q.conditions = TrackingConditions::default();
    q.confidence.mean_fixation = None;
    q.verdict = TrackingVerdict::Unusable;
    let v = roundtrip(&q, Casing::Camel);
    for absent in ["validation", "maxDriftDeg", "estimatedAccuracyDeg"] {
        assert!(v.get(absent).is_none(), "{absent} must be absent");
    }
    assert_eq!(v["conditions"], json!({}));
    assert_eq!(
        keys(&v["calibration"]),
        [
            "attempts",
            "layoutPoints",
            "missingPoints",
            "quality",
            "usablePoints"
        ]
    );
    assert!(v["device"].get("sampleRateHz").is_none());
}

#[test]
fn device_geometry_roundtrip() {
    let g = lookup("iPhone17,3").geometry;
    let v = roundtrip(&g, Casing::Camel);
    assert!(v.get("ref").is_some());
    let d = lookup("unknown").geometry;
    let v = roundtrip(&d, Casing::Camel);
    assert!(v.get("nativePixels").is_none() && v.get("ppi").is_none());
}

// ---------------------------------------------------------------- params (SCH-3)

#[test]
fn params_defaults_match_technical_md() {
    let p = TrackingQualityParams::default();
    assert_eq!(p.schema_version, "1.0.0");
    assert!(p.provisional);
    let c = &p.calibration;
    assert_eq!(
        (c.settle_ms, c.collect_ms, c.max_target_wait_ms),
        (400.0, 1200.0, 4000.0)
    );
    assert_eq!(
        (c.min_samples_per_target, c.max_retries_per_target),
        (20, 1)
    );
    assert_eq!(c.mad_reject_k, 3.0);
    assert_eq!(c.default_model, CalModelKind::ScreenAffine);
    assert!(!c.auto_select);
    assert_eq!(c.min_improvement_deg, 0.3);
    assert_eq!(c.eligibility_margin_points, 2);
    assert_eq!(
        (
            c.bands.good_max_deg,
            c.bands.fair_max_deg,
            c.bands.poor_max_deg
        ),
        (2.5, 3.5, 5.0)
    );
    assert_eq!((c.min_usable_points, c.max_condition_number), (4, 1e6));
    assert_eq!(
        c.layout5,
        vec![
            [0.5, 0.5],
            [0.12, 0.12],
            [0.88, 0.12],
            [0.12, 0.88],
            [0.88, 0.88]
        ]
    );
    assert_eq!(c.layout9.len(), 9);
    assert_eq!(
        &c.layout9[5..],
        &[[0.5, 0.12], [0.12, 0.5], [0.88, 0.5], [0.5, 0.88]]
    );

    let r = &p.recalibration;
    assert_eq!(
        (
            r.pose_shift_distance_pct,
            r.pose_shift_angle_deg,
            r.pose_window_sec
        ),
        (20.0, 15.0, 3.0)
    );
    assert_eq!(r.max_calibration_age_sec, 600.0);
    assert_eq!(
        (
            r.max_recalibrations_per_session,
            r.max_recalibrations_in_battery
        ),
        (2, 1)
    );

    let v = &p.validation;
    assert_eq!(
        v.layout,
        vec![[0.3, 0.3], [0.7, 0.3], [0.3, 0.7], [0.7, 0.7]]
    );
    assert_eq!(
        (
            v.settle_ms,
            v.collect_ms,
            v.min_samples_per_target,
            v.min_targets_hit
        ),
        (400.0, 1000.0, 12, 3)
    );
    assert_eq!((v.warn_accuracy_deg, v.max_accuracy_deg), (3.5, 5.0));
    assert_eq!(v.bench_layout13.len(), 13);

    let d = &p.drift;
    assert_eq!(
        d.check_layout,
        vec![[0.5, 0.5], [0.3, 0.3], [0.7, 0.3], [0.3, 0.7], [0.7, 0.7]]
    );
    assert_eq!(
        (d.settle_ms, d.collect_ms, d.min_samples),
        (300.0, 900.0, 10)
    );
    assert_eq!((d.warn_deg, d.warn_consecutive, d.hard_deg), (1.5, 2, 3.0));

    let t = &p.track_state;
    assert_eq!(
        (
            t.blink_recovery_ms,
            t.max_eye_speed_deg_per_sec,
            t.offscreen_margin_norm
        ),
        (50.0, 1000.0, 0.02)
    );

    let k = &p.confidence;
    assert_eq!(
        (k.eye_openness_comfort, k.pose_comfort_fraction),
        (0.70, 0.6)
    );
    assert_eq!(
        (k.distance_comfort_min_cm, k.distance_comfort_max_cm),
        (40.0, 60.0)
    );
    assert_eq!(
        (
            k.head_speed_comfort_deg_per_sec,
            k.max_head_speed_deg_per_sec
        ),
        (30.0, 120.0)
    );
    assert_eq!(k.pose_shift_comfort_pct, 10.0);

    let g = &p.gap_fill;
    assert_eq!(
        (g.max_ms, g.max_deg, g.loss_interval_cap_ms),
        (75.0, 1.0, 100.0)
    );
    let f = &p.filter;
    assert_eq!((f.min_cutoff_hz, f.beta, f.d_cutoff_hz), (1.2, 0.015, 1.0));
    let q = &p.verdict;
    assert_eq!(q.accuracy_bands, AccuracyBands::default());
    assert_eq!(
        (
            q.unusable_below_valid_ratio,
            q.poor_above_data_loss,
            q.fair_above_data_loss
        ),
        (0.5, 0.4, 0.2)
    );
    let s = &p.scoring;
    assert_eq!(
        (
            s.max_accuracy_deg_for_scoring,
            s.aoi_resolution_factor,
            s.ivt_velocity_deg_per_sec
        ),
        (5.0, 2.0, 30.0)
    );
}

#[test]
fn params_roundtrip_and_partial_bundles_decode_to_defaults() {
    let v = roundtrip(&TrackingQualityParams::default(), Casing::Camel);
    assert_eq!(v["calibration"]["defaultModel"], "screenAffine");
    let empty: TrackingQualityParams = serde_json::from_value(json!({})).unwrap();
    assert_eq!(empty, TrackingQualityParams::default());
    let partial: TrackingQualityParams = serde_json::from_value(json!({
        "provisional": false,
        "calibration": { "settleMs": 500, "autoSelect": true },
        "drift": { "warnDeg": 2.0 },
        "futureSection": { "x": 1 }
    }))
    .unwrap();
    let mut expected = TrackingQualityParams {
        provisional: false,
        ..TrackingQualityParams::default()
    };
    expected.calibration.settle_ms = 500.0;
    expected.calibration.auto_select = true;
    expected.drift.warn_deg = 2.0;
    assert_eq!(partial, expected);
}

// ---------------------------------------------------------------- raw log (REP-1)

fn header() -> RawLogHeader {
    let hit = lookup("iPhone17,3");
    RawLogHeader {
        format: RAW_LOG_FORMAT.into(),
        source: TrackingSource::ArkitFace,
        device_model: hit.model_identifier,
        orientation: Orientation::Portrait,
        geometry_source: hit.source,
        geometry: hit.geometry,
        targets: vec![ScheduledTarget {
            kind: TargetKind::Calibration,
            x_norm: 0.5,
            y_norm: 0.5,
            start_sec: 0.0,
            end_sec: 1.6,
            label: None,
        }],
        app_version: Some("1.0 (1)".into()),
        condition: Some("baseline".into()),
        ground_truth: Some(json!({ "bias_deg": [1.0, -0.5] })),
    }
}

#[test]
fn raw_log_header_roundtrip() {
    let v = roundtrip(&header(), Casing::Camel);
    assert_eq!(v["format"], "asd-raw-tracking/1");
    // groundTruth is generator-owned and opaque (snake_case inside is allowed).
    assert_eq!(v["groundTruth"]["bias_deg"], json!([1.0, -0.5]));
}

#[test]
fn raw_log_fixture_parses_and_reencodes_identically() {
    let log = raw_log::parse(RAW_FIXTURE).expect("committed fixture is a valid log");
    assert_eq!(log.header.format, RAW_LOG_FORMAT);
    assert_eq!(log.samples.len(), 3);
    assert!(log.samples[0].face_tracked && !log.samples[2].face_tracked);
    let text = raw_log::to_jsonl(&log).unwrap();
    assert_eq!(
        text, RAW_FIXTURE,
        "fixture must be in canonical writer form"
    );
    for line in RAW_FIXTURE.lines().skip(1) {
        let v: Value = serde_json::from_str(line).unwrap();
        check_keys(&v, Casing::Snake, "$");
    }
}

#[test]
fn raw_log_writer_parser_roundtrip() {
    let log = RawLog {
        header: header(),
        samples: vec![full_sample(), lost_sample()],
    };
    let text = raw_log::to_jsonl(&log).unwrap();
    assert_eq!(text.lines().count(), 3);
    assert!(text.ends_with('\n'));
    assert_eq!(raw_log::parse(&text).unwrap(), log);
}

#[test]
fn raw_log_rejects_bad_files() {
    let s1 = serde_json::to_string(&full_sample()).unwrap();
    let s2 = serde_json::to_string(&lost_sample()).unwrap();
    let h = serde_json::to_string(&header()).unwrap();

    assert_eq!(raw_log::parse(""), Err(RawLogError::Empty));
    assert_eq!(raw_log::parse("\n\n"), Err(RawLogError::Empty));
    // Missing header: the first line is a sample.
    assert!(matches!(
        raw_log::parse(&format!("{s1}\n{s2}\n")),
        Err(RawLogError::MissingHeader { .. })
    ));
    // Wrong format version.
    let mut bad = header();
    bad.format = "asd-raw-tracking/2".into();
    let bad = serde_json::to_string(&bad).unwrap();
    assert_eq!(
        raw_log::parse(&format!("{bad}\n{s1}\n")),
        Err(RawLogError::UnsupportedFormat {
            found: "asd-raw-tracking/2".into()
        })
    );
    // Non-monotonic t_sec (12.516 then 12.5).
    assert_eq!(
        raw_log::parse(&format!("{h}\n{s2}\n{s1}\n")),
        Err(RawLogError::NonMonotonicTime { line: 3 })
    );
    // Equal timestamps are not monotonic either.
    assert_eq!(
        raw_log::parse(&format!("{h}\n{s1}\n{s1}\n")),
        Err(RawLogError::NonMonotonicTime { line: 3 })
    );
    // A malformed sample line reports its line number.
    assert!(matches!(
        raw_log::parse(&format!("{h}\n{s1}\n{{\"t_sec\": 13}}\n")),
        Err(RawLogError::Json { line: 3, .. })
    ));
    // A second header is not a sample.
    assert!(matches!(
        raw_log::parse(&format!("{h}\n{h}\n")),
        Err(RawLogError::Json { line: 2, .. })
    ));
    // Header only is a valid (empty) log.
    assert_eq!(raw_log::parse(&format!("{h}\n")).unwrap().samples.len(), 0);
}
