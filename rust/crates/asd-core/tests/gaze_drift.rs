//! owner: WP-G.4
//!
//! Drift monitor (DRF-1..DRF-3) and re-calibration advice (CAL-9, VAL-4). Offsets use 3-4-5
//! triangles so every expected drift is exact.

use std::path::PathBuf;

use asd_core::gaze::drift::{advise, median_pose, AdviceState, DriftMonitor};
use asd_core::types::tracking::{
    CalModelKind, CalibrationPose, CalibrationQuality, DriftCheckRecord, DriftParams, Orientation,
    ProcessedSample, RecalAction, RecalReason, TargetKind, TargetOutcome, TargetResult, TrackState,
    TrackingQualityParams,
};
use serde_json::Value;

fn result(kind: TargetKind, at: f64, offset: Option<[f64; 2]>) -> TargetResult {
    let hit = offset.is_some();
    TargetResult {
        kind,
        x_norm: 0.5,
        y_norm: 0.5,
        started_at_sec: at,
        ended_at_sec: at + 1.2,
        attempt: 1,
        outcome: if hit {
            TargetOutcome::Hit
        } else {
            TargetOutcome::Missed
        },
        valid_samples: if hit { 30 } else { 3 },
        total_samples: 36,
        data_loss: 0.0,
        raw_median_norm: None,
        calibrated_median_norm: None,
        eye_pos_mm_median: None,
        accuracy_deg: offset.map(|o| (o[0] * o[0] + o[1] * o[1]).sqrt()),
        offset_deg: offset,
        precision_rms_deg: None,
        precision_rms_raw_deg: None,
        sd_deg: None,
    }
}

fn val(offset: Option<[f64; 2]>) -> TargetResult {
    result(TargetKind::Validation, 0.0, offset)
}

fn check(at: f64, offset: Option<[f64; 2]>) -> TargetResult {
    result(TargetKind::DriftCheck, at, offset)
}

fn monitor_with_bias(bias: [f64; 2]) -> DriftMonitor {
    let mut m = DriftMonitor::new(DriftParams::default());
    m.set_reference(&[val(Some(bias)), val(Some(bias)), val(None), val(Some(bias))]);
    m
}

#[test]
fn constant_bias_is_not_drift() {
    let mut m = monitor_with_bias([2.0, -1.0]);
    let r = m.record_check(&check(30.0, Some([2.0, -1.0])));
    assert_eq!(r.drift_deg, Some(0.0));
    assert_eq!(r.offset_deg, Some(5.0f64.sqrt()));
    assert!(!r.missed);
    assert_eq!(r.at_sec, 30.0);
    assert!(!m.triggered());
}

#[test]
fn reference_is_mean_of_hit_validation_offsets() {
    let mut m = DriftMonitor::new(DriftParams::default());
    // Mean of (0,0), (2,0), (1,3) = (1,1); the missed target is ignored.
    m.set_reference(&[
        val(Some([0.0, 0.0])),
        val(Some([2.0, 0.0])),
        val(None),
        val(Some([1.0, 3.0])),
    ]);
    assert_eq!(m.reference_offset_deg(), Some([1.0, 1.0]));
    // Offset (4, 5) -> difference (3, 4) -> drift 5.
    let r = m.record_check(&check(10.0, Some([4.0, 5.0])));
    assert_eq!(r.drift_deg, Some(5.0));
}

#[test]
fn hard_threshold_fires_on_one_check() {
    let mut m = monitor_with_bias([0.0, 0.0]);
    m.record_check(&check(10.0, Some([3.0, 4.0])));
    assert!(
        m.triggered(),
        "drift 5.0 > hardDeg 3.0 fires on a single check"
    );
}

#[test]
fn hard_threshold_boundary() {
    // A single 3.0 check is above warn (1.5) once, but not above hard (3.0): no trigger.
    let mut m = monitor_with_bias([0.0, 0.0]);
    m.record_check(&check(10.0, Some([3.0, 0.0])));
    assert!(!m.triggered());
    let mut m = monitor_with_bias([0.0, 0.0]);
    m.record_check(&check(10.0, Some([3.0, 0.0001])));
    assert!(m.triggered());
}

#[test]
fn warn_on_two_consecutive_checks_fires() {
    let mut m = monitor_with_bias([1.0, 1.0]);
    m.record_check(&check(10.0, Some([1.0 + 1.6, 1.0])));
    assert!(!m.triggered());
    m.record_check(&check(20.0, Some([1.0, 1.0 - 1.6])));
    assert!(m.triggered());
}

#[test]
fn warn_boundary_is_strict() {
    let mut m = monitor_with_bias([0.0, 0.0]);
    m.record_check(&check(10.0, Some([1.5, 0.0])));
    m.record_check(&check(20.0, Some([0.0, 1.5])));
    assert!(!m.triggered(), "1.5 is not above warnDeg 1.5");
}

#[test]
fn non_consecutive_warn_pair_does_not_fire() {
    let mut m = monitor_with_bias([0.0, 0.0]);
    m.record_check(&check(10.0, Some([2.0, 0.0])));
    m.record_check(&check(20.0, Some([0.5, 0.0])));
    m.record_check(&check(30.0, Some([2.0, 0.0])));
    assert!(!m.triggered());
    // A missed check also breaks the run.
    let mut m = monitor_with_bias([0.0, 0.0]);
    m.record_check(&check(10.0, Some([2.0, 0.0])));
    let r = m.record_check(&check(20.0, None));
    assert!(r.missed);
    assert_eq!(r.offset_deg, None);
    assert_eq!(r.drift_deg, None);
    m.record_check(&check(30.0, Some([2.0, 0.0])));
    assert!(!m.triggered());
    // ... and the next consecutive one fires.
    m.record_check(&check(40.0, Some([2.0, 0.0])));
    assert!(m.triggered());
}

#[test]
fn without_validation_reference_drift_is_absent() {
    let mut m = DriftMonitor::new(DriftParams::default());
    let r = m.record_check(&check(10.0, Some([4.0, 3.0])));
    assert_eq!(r.offset_deg, Some(5.0));
    assert_eq!(r.drift_deg, None);
    assert!(!m.triggered());
    assert_eq!(m.max_drift_deg(), None);
    let json = serde_json::to_value(&r).unwrap();
    assert!(json.get("driftDeg").is_none());
}

#[test]
fn recalibration_clears_reference_and_trigger_but_keeps_history() {
    let mut m = monitor_with_bias([0.0, 0.0]);
    m.record_check(&check(10.0, Some([4.0, 0.0])));
    assert!(m.triggered());
    m.on_recalibrated();
    assert!(!m.triggered());
    assert_eq!(m.reference_offset_deg(), None);
    assert_eq!(m.checks().len(), 1);
    assert_eq!(m.max_drift_deg(), Some(4.0));
    m.set_reference(&[val(Some([0.0, 0.0]))]);
    m.record_check(&check(20.0, Some([2.0, 0.0])));
    assert!(!m.triggered(), "warn run restarted after re-calibration");
}

#[test]
fn check_targets_cycle_through_layout() {
    let p = DriftParams::default();
    let mut m = DriftMonitor::new(p.clone());
    let mut seen = Vec::new();
    for i in 0..7 {
        seen.push(m.next_check_target());
        m.record_check(&check(i as f64, None));
    }
    let l = &p.check_layout;
    assert_eq!(seen, vec![l[0], l[1], l[2], l[3], l[4], l[0], l[1]]);
}

#[test]
fn max_drift_over_checks() {
    let mut m = monitor_with_bias([0.0, 0.0]);
    m.record_check(&check(10.0, Some([0.3, 0.4])));
    m.record_check(&check(20.0, None));
    m.record_check(&check(30.0, Some([0.6, 0.8])));
    assert_eq!(m.max_drift_deg(), Some(1.0));
}

// ---------------------------------------------------------------------------------------
// Advice (CAL-9)
// ---------------------------------------------------------------------------------------

fn calm() -> AdviceState {
    AdviceState {
        calibration_quality: Some(CalibrationQuality::Good),
        calibration_model: Some(CalModelKind::ScreenAffine),
        validation_accuracy_deg: Some(1.0),
        drift_triggered: false,
        calibration_pose: Some(pose(50.0, 0.0, 0.0)),
        recent_pose: Some(pose(50.0, 0.0, 0.0)),
        calibration_orientation: Some(Orientation::Portrait),
        current_orientation: Some(Orientation::Portrait),
        calibration_age_sec: Some(60.0),
        recalibrations_done: 0,
        recalibrations_in_battery: 0,
        in_battery: false,
    }
}

fn pose(d: f64, yaw: f64, pitch: f64) -> CalibrationPose {
    CalibrationPose {
        distance_cm: d,
        yaw_deg: yaw,
        pitch_deg: pitch,
    }
}

fn p() -> TrackingQualityParams {
    TrackingQualityParams::default()
}

#[test]
fn calm_session_needs_nothing() {
    let a = advise(&calm(), &p());
    assert_eq!(a.action, RecalAction::None);
    assert!(a.reasons.is_empty());
    assert!(!a.limit_reached);
}

#[test]
fn each_trigger_alone() {
    let cases: Vec<(AdviceState, RecalReason)> = vec![
        (
            AdviceState {
                calibration_quality: Some(CalibrationQuality::Poor),
                ..calm()
            },
            RecalReason::CalibrationQuality,
        ),
        (
            AdviceState {
                calibration_quality: Some(CalibrationQuality::Failed),
                ..calm()
            },
            RecalReason::CalibrationQuality,
        ),
        (
            AdviceState {
                validation_accuracy_deg: Some(5.01),
                ..calm()
            },
            RecalReason::ValidationAccuracy,
        ),
        (
            AdviceState {
                drift_triggered: true,
                ..calm()
            },
            RecalReason::Drift,
        ),
        (
            AdviceState {
                recent_pose: Some(pose(60.1, 0.0, 0.0)),
                ..calm()
            },
            RecalReason::PoseShift,
        ),
        (
            AdviceState {
                recent_pose: Some(pose(50.0, -15.1, 0.0)),
                ..calm()
            },
            RecalReason::PoseShift,
        ),
        (
            AdviceState {
                recent_pose: Some(pose(50.0, 0.0, 15.1)),
                ..calm()
            },
            RecalReason::PoseShift,
        ),
        (
            AdviceState {
                current_orientation: Some(Orientation::LandscapeLeft),
                ..calm()
            },
            RecalReason::OrientationChanged,
        ),
        (
            AdviceState {
                calibration_age_sec: Some(600.1),
                ..calm()
            },
            RecalReason::CalibrationAge,
        ),
    ];
    for (state, reason) in cases {
        let a = advise(&state, &p());
        assert_eq!(a.action, RecalAction::Recalibrate, "{reason:?}");
        assert_eq!(a.reasons, vec![reason]);
        assert!(!a.limit_reached);
    }
}

#[test]
fn thresholds_are_strict() {
    let edge = AdviceState {
        validation_accuracy_deg: Some(3.5),
        recent_pose: Some(pose(60.0, 15.0, -15.0)),
        calibration_age_sec: Some(600.0),
        calibration_quality: Some(CalibrationQuality::Fair),
        ..calm()
    };
    let a = advise(&edge, &p());
    assert_eq!(a.action, RecalAction::None, "{a:?}");
}

#[test]
fn validation_warn_band_warns_only() {
    let a = advise(
        &AdviceState {
            validation_accuracy_deg: Some(4.0),
            ..calm()
        },
        &p(),
    );
    assert_eq!(a.action, RecalAction::Warn);
    assert_eq!(a.reasons, vec![RecalReason::ValidationAccuracy]);
}

#[test]
fn pose_shift_suppressed_for_angular_affine() {
    let s = AdviceState {
        calibration_model: Some(CalModelKind::AngularAffine),
        recent_pose: Some(pose(70.0, 20.0, 20.0)),
        ..calm()
    };
    assert_eq!(advise(&s, &p()).action, RecalAction::None);
    for model in [
        CalModelKind::ScreenAffine,
        CalModelKind::ScreenHomography,
        CalModelKind::ScreenPoly2,
    ] {
        let s = AdviceState {
            calibration_model: Some(model),
            ..s.clone()
        };
        assert_eq!(advise(&s, &p()).reasons, vec![RecalReason::PoseShift]);
    }
}

#[test]
fn several_reasons_in_trigger_order() {
    let s = AdviceState {
        calibration_age_sec: Some(900.0),
        drift_triggered: true,
        calibration_quality: Some(CalibrationQuality::Poor),
        ..calm()
    };
    let a = advise(&s, &p());
    assert_eq!(
        a.reasons,
        vec![
            RecalReason::CalibrationQuality,
            RecalReason::Drift,
            RecalReason::CalibrationAge
        ]
    );
}

#[test]
fn session_ceiling_turns_advice_into_warning() {
    let s = AdviceState {
        drift_triggered: true,
        recalibrations_done: 2,
        ..calm()
    };
    let a = advise(&s, &p());
    assert_eq!(a.action, RecalAction::Warn);
    assert!(a.limit_reached);
    assert_eq!(a.reasons, vec![RecalReason::Drift]);
    // One below the ceiling is still allowed.
    let s = AdviceState {
        recalibrations_done: 1,
        ..s
    };
    assert_eq!(advise(&s, &p()).action, RecalAction::Recalibrate);
}

#[test]
fn battery_ceiling_is_one() {
    let s = AdviceState {
        drift_triggered: true,
        in_battery: true,
        recalibrations_done: 1,
        recalibrations_in_battery: 1,
        ..calm()
    };
    let a = advise(&s, &p());
    assert_eq!(a.action, RecalAction::Warn);
    assert!(a.limit_reached);
    // The same count outside the battery is under the session ceiling (2).
    let s = AdviceState {
        in_battery: false,
        ..s
    };
    assert_eq!(advise(&s, &p()).action, RecalAction::Recalibrate);
    // Inside the battery with none used yet.
    let s = AdviceState {
        in_battery: true,
        recalibrations_in_battery: 0,
        ..s
    };
    assert_eq!(advise(&s, &p()).action, RecalAction::Recalibrate);
}

#[test]
fn unmeasured_inputs_never_trigger() {
    let s = AdviceState::default();
    let a = advise(&s, &p());
    assert_eq!(a.action, RecalAction::None);
    assert!(a.reasons.is_empty());
}

fn pose_sample(t: f64, d: f64, yaw_deg: f64, state: TrackState) -> ProcessedSample {
    ProcessedSample {
        t_sec: t,
        track_state: state,
        ray_status: None,
        raw_point_norm: None,
        calibrated_point_norm: None,
        filtered_point_norm: None,
        eye_pos_mm: None,
        distance_cm: Some(d),
        head_yaw_rad: Some(yaw_deg.to_radians()),
        head_pitch_rad: Some(0.0),
        head_roll_rad: None,
        head_speed_deg_per_sec: None,
        eye_speed_deg_per_sec: None,
        left_eye_openness: None,
        right_eye_openness: None,
        ambient_intensity: None,
        confidence: 1.0,
        valid: true,
        gap_filled: false,
        active_aoi_names: Vec::new(),
        stimulus_id: None,
        clip_media_time_sec: None,
    }
}

#[test]
fn median_pose_over_window_uses_tracked_frames() {
    let s = vec![
        pose_sample(0.0, 90.0, 40.0, TrackState::Tracked), // outside window
        pose_sample(7.5, 44.0, 2.0, TrackState::Tracked),
        pose_sample(8.0, 46.0, 4.0, TrackState::Tracked),
        pose_sample(8.5, 10.0, 80.0, TrackState::Lost), // not tracked
        pose_sample(9.0, 52.0, 6.0, TrackState::Tracked),
    ];
    let mp = median_pose(&s, 6.0, 9.0).unwrap();
    assert_eq!(mp.distance_cm, 46.0);
    assert!((mp.yaw_deg - 4.0).abs() < 1e-12);
    assert_eq!(mp.pitch_deg, 0.0);
    assert!(median_pose(&s, 20.0, 30.0).is_none());
}

// ---------------------------------------------------------------------------------------
// Goldens: tests/golden/gaze/drift/*.json (a sequence of checks through the monitor).
// ---------------------------------------------------------------------------------------

#[test]
fn golden_drift_sequences() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden/gaze/drift");
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
        assert_eq!(doc["function"], "drift::DriftMonitor", "{name}");
        let params: DriftParams = serde_json::from_value(doc["input"]["params"].clone()).unwrap();
        let validation: Vec<TargetResult> =
            serde_json::from_value(doc["input"]["validation"].clone()).unwrap();
        let checks: Vec<TargetResult> =
            serde_json::from_value(doc["input"]["checks"].clone()).unwrap();
        let mut m = DriftMonitor::new(params);
        m.set_reference(&validation);
        let mut triggered_after = Vec::new();
        for c in &checks {
            m.record_check(c);
            triggered_after.push(m.triggered());
        }
        let records: Vec<DriftCheckRecord> = m.checks().to_vec();
        let actual = serde_json::json!({
            "checks": records,
            "maxDriftDeg": m.max_drift_deg(),
            "triggeredAfter": triggered_after,
        });
        assert_eq!(actual, doc["expected"], "{name}");
    }
}
