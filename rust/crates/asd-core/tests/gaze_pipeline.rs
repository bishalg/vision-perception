//! owner: WP-G.3
//!
//! PIPE-1, PIPE-5, TRK-1, TRK-2, CNF-4, GAP-2/3 end to end through `GazePipeline`: order of the
//! stages (track state, raycast, calibration, confidence, 1-Euro on `tracked` samples only,
//! AOI hit test), calibration / validation / drift-check targets through one target API, the
//! session summary accounting (loss, `validRatio`, confidence, conditions), and determinism.
//!
//! The synthetic head (camera frame of `gaze::raycast::CAMERA_AXES`: metres, portrait native,
//! +x right, +y towards the camera edge, viewer at z < 0, screen plane z = 0) sits 40 cm in
//! front of the centre of a 100 x 200 mm panel and looks at a scripted screen point with a
//! known angular eye-model bias.

use std::path::PathBuf;

use asd_core::gaze::aoi_space::{NormBox, ScreenAoi};
use asd_core::gaze::calibration::{CalModel, CalibrationOutcome, TargetStatus};
use asd_core::gaze::filter::OneEuro2D;
use asd_core::gaze::gapfill;
use asd_core::gaze::geometry::idiom_default;
use asd_core::gaze::pipeline::{GazePipeline, PipelineConfig, PipelineError, TelemetryWriter};
use asd_core::gaze::summary;
use asd_core::quality::QualityGate;
use asd_core::types::tracking::{
    CalModelKind, CalibrationQuality, DeviceGeometry, DeviceIdiom, GeometrySource, Orientation,
    ProcessedSample, RawTrackingSample, RecalAction, RecalReason, TargetKind, TargetOutcome,
    TargetResult, TrackState, TrackingQualityParams, TrackingVerdict,
};
use serde_json::{json, Value};

type M4 = [f64; 16];

const HZ: f64 = 60.0;

// ------------------------------------------------------------------------------------------
// Synthetic scene
// ------------------------------------------------------------------------------------------

fn test_geo() -> DeviceGeometry {
    let mut g = idiom_default(DeviceIdiom::Phone);
    g.screen_width_mm = 100.0;
    g.screen_height_mm = 200.0;
    g.camera_offset_mm.x = 50.0;
    g.camera_offset_mm.y = -8.0;
    g
}

fn config() -> PipelineConfig {
    PipelineConfig {
        model_identifier: "test,1".into(),
        geometry_source: GeometrySource::Table,
        geometry: test_geo(),
        orientation: Orientation::Portrait,
        quality_gate: QualityGate::default(),
        params: TrackingQualityParams::default(),
        calibration: None,
        glasses: None,
    }
}

fn translation(t: [f64; 3]) -> M4 {
    let mut m = [0.0; 16];
    m[0] = 1.0;
    m[5] = 1.0;
    m[10] = 1.0;
    m[15] = 1.0;
    m[12] = t[0];
    m[13] = t[1];
    m[14] = t[2];
    m
}

/// Rotation about the camera y axis by `deg` plus translation (column-major).
fn yaw_rot(deg: f64, t: [f64; 3]) -> M4 {
    let (s, c) = deg.to_radians().sin_cos();
    let mut m = translation(t);
    m[0] = c;
    m[2] = -s;
    m[8] = s;
    m[10] = c;
    m
}

fn rigid_inverse_apply(m: &M4, p: [f64; 3]) -> [f64; 3] {
    let d = [p[0] - m[12], p[1] - m[13], p[2] - m[14]];
    [
        m[0] * d[0] + m[1] * d[1] + m[2] * d[2],
        m[4] * d[0] + m[5] * d[1] + m[6] * d[2],
        m[8] * d[0] + m[9] * d[1] + m[10] * d[2],
    ]
}

const EYE_MID_FACE: [f64; 3] = [0.0, 0.025, 0.01];

fn angles_of(d: [f64; 3]) -> [f64; 2] {
    [
        d[0].atan2(d[2]).to_degrees(),
        d[1].atan2((d[0] * d[0] + d[2] * d[2]).sqrt()).to_degrees(),
    ]
}

fn dir_of(a: [f64; 2]) -> [f64; 3] {
    let (y, p) = (a[0].to_radians(), a[1].to_radians());
    [p.cos() * y.sin(), p.sin(), p.cos() * y.cos()]
}

fn screen_cam(n: [f64; 2]) -> [f64; 3] {
    let g = test_geo();
    let mm = [n[0] * g.screen_width_mm, n[1] * g.screen_height_mm];
    [
        (mm[0] - g.camera_offset_mm.x) / 1000.0,
        -(mm[1] - g.camera_offset_mm.y) / 1000.0,
        0.0,
    ]
}

/// Face pose at `distance_m` in front of the panel centre, rotated by `yaw_deg`.
fn face(distance_m: f64, yaw_deg: f64) -> M4 {
    let c = screen_cam([0.5, 0.5]);
    yaw_rot(yaw_deg, [c[0], c[1], -distance_m])
}

#[derive(Clone, Copy)]
enum Eye {
    Open,
    Closed,
    NoFace,
}

/// One raw frame of a head with pose `f` looking at `target` with an angular eye bias (deg).
fn frame(t: f64, f: &M4, target: [f64; 2], bias: [f64; 2], eye: Eye) -> RawTrackingSample {
    let mut s = RawTrackingSample {
        t_sec: t,
        face_tracked: false,
        face_transform: None,
        look_at: None,
        left_eye_transform: None,
        right_eye_transform: None,
        eye_blink_left: None,
        eye_blink_right: None,
        ambient_intensity: Some(800.0),
        ambient_color_temp: None,
        light_direction: None,
        orientation: Orientation::Portrait,
        clip_media_time_sec: None,
        stimulus_id: None,
    };
    if let Eye::NoFace = eye {
        return s;
    }
    let tgt = rigid_inverse_apply(f, screen_cam(target));
    let d = [
        tgt[0] - EYE_MID_FACE[0],
        tgt[1] - EYE_MID_FACE[1],
        tgt[2] - EYE_MID_FACE[2],
    ];
    let a = angles_of(d);
    let md = dir_of([a[0] + bias[0], a[1] + bias[1]]);
    s.face_tracked = true;
    s.face_transform = Some(*f);
    s.look_at = Some([
        EYE_MID_FACE[0] + 0.5 * md[0],
        EYE_MID_FACE[1] + 0.5 * md[1],
        EYE_MID_FACE[2] + 0.5 * md[2],
    ]);
    s.left_eye_transform = Some(translation([0.031, 0.025, 0.01]));
    s.right_eye_transform = Some(translation([-0.031, 0.025, 0.01]));
    let blink = if let Eye::Closed = eye { 0.9 } else { 0.05 };
    s.eye_blink_left = Some(blink);
    s.eye_blink_right = Some(0.05);
    s
}

/// A frame clock that never accumulates rounding error.
struct Clock {
    n: u64,
    t0: f64,
}

impl Clock {
    fn new(t0: f64) -> Self {
        Clock { n: 0, t0 }
    }
    fn tick(&mut self) -> f64 {
        let t = self.t0 + self.n as f64 / HZ;
        self.n += 1;
        t
    }
}

fn assert_close(a: f64, b: f64, eps: f64, what: &str) {
    assert!((a - b).abs() <= eps, "{what}: {a} != {b}");
}

/// Runs one target window: frames looking at the target until the pipeline reports the window
/// complete (with a safety cap), then `end_target`.
fn run_target(
    p: &mut GazePipeline,
    clock: &mut Clock,
    kind: TargetKind,
    target: [f64; 2],
    bias: [f64; 2],
) -> TargetResult {
    p.begin_target(kind, target).expect("begin_target");
    let f = face(0.40, 0.0);
    for _ in 0..600 {
        p.push(&frame(clock.tick(), &f, target, bias, Eye::Open));
        if p.target_complete() {
            break;
        }
    }
    assert!(p.target_complete(), "{kind:?} window never completed");
    p.end_target().expect("end_target")
}

fn calibrate(p: &mut GazePipeline, clock: &mut Clock, bias: [f64; 2], reason: Option<RecalReason>) {
    p.start_calibration(None, reason).unwrap();
    while let Some(TargetStatus::Pending { target_norm, .. }) = p.calibration_status() {
        let r = run_target(p, clock, TargetKind::Calibration, target_norm, bias);
        assert_eq!(r.kind, TargetKind::Calibration);
    }
    p.finish_calibration().unwrap();
}

fn validate(p: &mut GazePipeline, clock: &mut Clock, bias: [f64; 2]) -> Vec<TargetResult> {
    let layout = TrackingQualityParams::default().validation.layout;
    layout
        .into_iter()
        .map(|t| run_target(p, clock, TargetKind::Validation, t, bias))
        .collect()
}

// ------------------------------------------------------------------------------------------
// TRK-1, TRK-2, stage order
// ------------------------------------------------------------------------------------------

#[test]
fn every_frame_produces_a_sample_including_lost_frames() {
    let mut p = GazePipeline::new(config());
    let f = face(0.40, 0.0);
    let mut c = Clock::new(0.0);
    let eyes = [
        Eye::Open,
        Eye::NoFace,
        Eye::NoFace,
        Eye::Open,
        Eye::Closed,
        Eye::Open,
    ];
    let out: Vec<ProcessedSample> = eyes
        .iter()
        .map(|&e| p.push(&frame(c.tick(), &f, [0.5, 0.5], [0.0, 0.0], e)))
        .collect();
    assert_eq!(out.len(), eyes.len());
    for (s, e) in out.iter().zip(eyes) {
        match e {
            Eye::NoFace => {
                assert_eq!(s.track_state, TrackState::Lost);
                assert_eq!(s.ray_status, None);
                assert_eq!(s.raw_point_norm, None);
                assert_eq!(s.distance_cm, None, "no pose for a lost frame (trap 6)");
                assert_eq!(s.confidence, 0.0);
                assert!(!s.valid);
            }
            Eye::Closed => {
                assert_eq!(s.track_state, TrackState::Blink);
                assert_eq!(s.filtered_point_norm, None);
                assert!(!s.valid);
            }
            Eye::Open => {}
        }
        assert!(!s.gap_filled, "push never fills gaps");
    }
    // The frame right after a blink is still inside the 50 ms recovery.
    assert_eq!(out[5].track_state, TrackState::Blink);
    assert_eq!(out[0].track_state, TrackState::Tracked);
    assert!(out[0].valid);
    assert_close(
        out[0].distance_cm.unwrap(),
        40.0,
        1e-9,
        "distance from the face anchor",
    );
}

/// Calibration that maps raw x to x - 0.2 (pure shift).
fn shift_model(dx: f64) -> CalModel {
    CalModel::ScreenAffine {
        x: [1.0, 0.0, dx],
        y: [0.0, 1.0, 0.0],
    }
}

#[test]
fn validity_is_decided_on_the_calibrated_point_and_nothing_is_clamped() {
    let mut cfg = config();
    cfg.calibration = Some(shift_model(-0.2));
    let mut p = GazePipeline::new(cfg);
    let f = face(0.40, 0.0);
    // Raw gaze off the right edge (x = 1.1): calibrated 0.9 → tracked and valid.
    let s = p.push(&frame(0.0, &f, [1.1, 0.5], [0.0, 0.0], Eye::Open));
    let raw = s.raw_point_norm.unwrap();
    assert_close(raw[0], 1.1, 1e-9, "raw unclamped");
    assert_close(s.calibrated_point_norm.unwrap()[0], 0.9, 1e-9, "calibrated");
    assert_eq!(s.track_state, TrackState::Tracked);
    assert!(s.valid);
    // Raw on screen at x = 0.1: calibrated -0.1 → offscreen, invalid, unclamped.
    let s = p.push(&frame(0.1, &f, [0.1, 0.5], [0.0, 0.0], Eye::Open));
    assert_eq!(s.track_state, TrackState::Offscreen);
    assert!(!s.valid);
    assert_close(s.calibrated_point_norm.unwrap()[0], -0.1, 1e-9, "unclamped");
    assert_eq!(
        s.filtered_point_norm, None,
        "filter only on tracked samples"
    );
}

#[test]
fn calibration_is_applied_before_the_filter() {
    let mut cfg = config();
    cfg.calibration = Some(shift_model(0.1));
    let mut p = GazePipeline::new(cfg);
    let f = face(0.40, 0.0);
    let s = p.push(&frame(0.0, &f, [0.3, 0.4], [0.0, 0.0], Eye::Open));
    let cal = s.calibrated_point_norm.unwrap();
    assert_close(
        cal[0],
        s.raw_point_norm.unwrap()[0] + 0.1,
        1e-12,
        "calibrated",
    );
    // The first filtered sample passes through: the filter saw the calibrated point.
    assert_eq!(s.filtered_point_norm, Some(cal));
}

#[test]
fn filter_runs_on_tracked_samples_only_in_sample_time() {
    let mut p = GazePipeline::new(config());
    let mut reference = OneEuro2D::new(TrackingQualityParams::default().filter);
    let f = face(0.40, 0.0);
    let mut c = Clock::new(3.0);
    for i in 0..90 {
        let eye = match i {
            20..=24 => Eye::Closed,
            40..=45 => Eye::NoFace,
            _ => Eye::Open,
        };
        let target = [0.3 + 0.004 * i as f64, 0.5 + 0.1 * (i as f64 * 0.3).sin()];
        let s = p.push(&frame(c.tick(), &f, target, [0.0, 0.0], eye));
        if s.track_state == TrackState::Tracked {
            let want = reference.filter(s.calibrated_point_norm.unwrap(), s.t_sec);
            assert_eq!(s.filtered_point_norm, Some(want), "frame {i}");
        } else {
            assert_eq!(s.filtered_point_norm, None, "frame {i}");
        }
    }
}

// ------------------------------------------------------------------------------------------
// CNF-1, CNF-4 in the pipeline
// ------------------------------------------------------------------------------------------

#[test]
fn confidence_is_computed_per_sample_and_drives_valid() {
    let mut p = GazePipeline::new(config());
    let at = |p: &mut GazePipeline, t: f64, d: f64| {
        p.push(&frame(t, &face(d, 0.0), [0.5, 0.5], [0.0, 0.0], Eye::Open))
    };
    let s = at(&mut p, 0.0, 0.625);
    assert_close(s.confidence, 0.75, 1e-9, "62.5 cm");
    assert!(s.valid);
    let s = at(&mut p, 1.0, 0.65);
    assert_close(s.confidence, 0.5, 1e-9, "at the 65 cm limit");
    let s = at(&mut p, 2.0, 0.70);
    assert_eq!(s.confidence, 0.0);
    assert_eq!(
        s.track_state,
        TrackState::Tracked,
        "too far is low confidence, not a state"
    );
    assert!(!s.valid);
}

#[test]
fn head_speed_lowers_confidence() {
    let mut p = GazePipeline::new(config());
    let mut c = Clock::new(0.0);
    let mut last = None;
    // 1° of yaw per frame at 60 Hz = 60°/s → factor 1 - 0.5·30/90.
    for i in 0..10 {
        let f = face(0.45, i as f64);
        last = Some(p.push(&frame(c.tick(), &f, [0.5, 0.5], [0.0, 0.0], Eye::Open)));
    }
    let s = last.unwrap();
    assert_close(s.head_speed_deg_per_sec.unwrap(), 60.0, 1e-6, "head speed");
    assert_close(s.confidence, 1.0 - 0.5 * 30.0 / 90.0, 1e-6, "confidence");
    assert_close(s.head_yaw_rad.unwrap().to_degrees(), 9.0, 1e-9, "yaw");
}

#[test]
fn telemetry_carries_computed_confidence_and_last_known_pose_on_lost_frames() {
    let mut p = GazePipeline::new(config());
    let mut w = TelemetryWriter::new(10.0, QualityGate::default());
    let f = face(0.50, 5.0);
    let a = p.push(&frame(10.5, &f, [0.4, 0.6], [0.0, 0.0], Eye::Open));
    let ta = w.sample(&a);
    assert_eq!(ta.confidence, a.confidence);
    assert!(ta.is_valid);
    assert_close(ta.timestamp_ms, 500.0, 1e-9, "timestamp");
    assert_close(ta.distance_to_screen_cm, 50.0, 1e-9, "distance");
    let fp = a.filtered_point_norm.unwrap();
    assert_eq!((ta.x_norm, ta.y_norm), (fp[0], fp[1]));

    let lost = p.push(&frame(10.6, &f, [0.4, 0.6], [0.0, 0.0], Eye::NoFace));
    let tl = w.sample(&lost);
    assert!(!tl.is_valid);
    assert_eq!(tl.confidence, 0.0);
    // Trap 6: the last known pose and point, never zeros.
    assert_eq!(tl.distance_to_screen_cm, ta.distance_to_screen_cm);
    assert_eq!(tl.head_yaw_rad, ta.head_yaw_rad);
    assert_eq!((tl.x_norm, tl.y_norm), (ta.x_norm, ta.y_norm));
}

// ------------------------------------------------------------------------------------------
// AOI hit test (GEO-7 boxes in screen space)
// ------------------------------------------------------------------------------------------

#[test]
fn aoi_hits_use_the_filtered_point_and_clip_time_on_tracked_samples() {
    let mut p = GazePipeline::new(config());
    p.set_clip_aois(vec![
        ScreenAoi {
            name: "Social_Actor".into(),
            start_sec: 0.0,
            end_sec: 5.0,
            rect: NormBox {
                x_min: 0.0,
                y_min: 0.0,
                x_max: 0.5,
                y_max: 1.0,
            },
        },
        ScreenAoi {
            name: "Geometric_Pattern".into(),
            start_sec: 0.0,
            end_sec: 5.0,
            rect: NormBox {
                x_min: 0.5,
                y_min: 0.0,
                x_max: 1.0,
                y_max: 1.0,
            },
        },
    ]);
    let f = face(0.40, 0.0);
    let mut raw = frame(100.0, &f, [0.25, 0.5], [0.0, 0.0], Eye::Open);
    raw.clip_media_time_sec = Some(1.0);
    raw.stimulus_id = Some("geo_pref".into());
    let s = p.push(&raw);
    assert_eq!(s.active_aoi_names, vec!["Social_Actor".to_string()]);
    assert_eq!(s.stimulus_id.as_deref(), Some("geo_pref"));
    assert_eq!(s.clip_media_time_sec, Some(1.0));
    // Outside the AOI time window.
    let mut late = frame(100.1, &f, [0.25, 0.5], [0.0, 0.0], Eye::Open);
    late.clip_media_time_sec = Some(6.0);
    assert!(p.push(&late).active_aoi_names.is_empty());
    // A blink is never hit-tested.
    let mut b = frame(100.2, &f, [0.25, 0.5], [0.0, 0.0], Eye::Closed);
    b.clip_media_time_sec = Some(1.2);
    assert!(p.push(&b).active_aoi_names.is_empty());
}

// ------------------------------------------------------------------------------------------
// Targets: calibration, validation, drift checks, advice, re-calibration
// ------------------------------------------------------------------------------------------

#[test]
fn calibration_with_offscreen_raw_gaze_hits_every_target_and_validates() {
    let mut p = GazePipeline::new(config());
    let mut c = Clock::new(0.0);
    // 3° of eye-model bias: corner targets have their raw gaze off the glass (x < -0.02).
    let bias = [-3.0, 2.0];
    let probe = p.push(&frame(
        c.tick(),
        &face(0.40, 0.0),
        [0.12, 0.12],
        bias,
        Eye::Open,
    ));
    assert_eq!(
        probe.track_state,
        TrackState::Offscreen,
        "uncalibrated raw is off screen"
    );

    p.start_calibration(None, None).unwrap();
    let mut results = Vec::new();
    while let Some(TargetStatus::Pending { target_norm, .. }) = p.calibration_status() {
        results.push(run_target(
            &mut p,
            &mut c,
            TargetKind::Calibration,
            target_norm,
            bias,
        ));
    }
    assert_eq!(results.len(), 5);
    assert!(
        results.iter().all(|r| r.outcome == TargetOutcome::Hit),
        "{results:?}"
    );
    let outcome = p.finish_calibration().unwrap();
    assert_eq!(outcome.record.quality, CalibrationQuality::Good);
    assert_eq!(
        p.calibration().map(CalModel::kind),
        Some(CalModelKind::ScreenAffine)
    );

    let val = validate(&mut p, &mut c, bias);
    assert!(
        val.iter().all(|r| r.outcome == TargetOutcome::Hit),
        "{val:?}"
    );
    let rec = p.validation().unwrap();
    assert!(rec.complete);
    assert!(rec.accuracy_deg.unwrap() < 0.3, "{rec:?}");
    assert_eq!(p.advice().action, RecalAction::None);
}

#[test]
fn target_api_errors() {
    let mut p = GazePipeline::new(config());
    assert_eq!(
        p.begin_target(TargetKind::Calibration, [0.5, 0.5]),
        Err(PipelineError::NoCalibrationSession)
    );
    assert_eq!(p.end_target(), None);
    p.begin_target(TargetKind::Validation, [0.3, 0.3]).unwrap();
    assert_eq!(
        p.begin_target(TargetKind::Validation, [0.7, 0.3]),
        Err(PipelineError::TargetOpen)
    );
    p.end_target().unwrap();
    p.start_calibration(None, None).unwrap();
    assert!(matches!(
        p.begin_target(TargetKind::Calibration, [0.3, 0.3]),
        Err(PipelineError::TargetMismatch { .. })
    ));
}

#[test]
fn a_window_with_no_frames_is_missed() {
    let mut p = GazePipeline::new(config());
    p.begin_target(TargetKind::DriftCheck, [0.5, 0.5]).unwrap();
    let r = p.end_target().unwrap();
    assert_eq!(r.outcome, TargetOutcome::Missed);
    assert_eq!(r.valid_samples, 0);
}

#[test]
fn drift_checks_feed_the_monitor_and_recalibration_resets_it() {
    let mut p = GazePipeline::new(config());
    let mut c = Clock::new(0.0);
    let bias = [2.0, -1.0];
    calibrate(&mut p, &mut c, bias, None);
    validate(&mut p, &mut c, bias);
    p.set_recording(true);

    // Same bias: no drift.
    let t = p.next_check_target();
    let r = run_target(&mut p, &mut c, TargetKind::DriftCheck, t, bias);
    assert_eq!(r.kind, TargetKind::DriftCheck);
    let chk = p.drift_checks().last().unwrap().clone();
    assert!(chk.drift_deg.unwrap() < 0.3, "{chk:?}");
    assert_eq!(p.advice().action, RecalAction::None);

    // The eye model drifts by 4° → one check above hardDeg fires DRF-3.
    let t = p.next_check_target();
    run_target(
        &mut p,
        &mut c,
        TargetKind::DriftCheck,
        t,
        [bias[0] + 4.0, bias[1]],
    );
    let chk = p.drift_checks().last().unwrap().clone();
    assert!(chk.drift_deg.unwrap() > 3.0, "{chk:?}");
    let advice = p.advice();
    assert_eq!(advice.action, RecalAction::Recalibrate);
    assert!(advice.reasons.contains(&RecalReason::Drift), "{advice:?}");

    // Re-calibration with the new bias: recorded, drift reference cleared, history kept.
    let new_bias = [bias[0] + 4.0, bias[1]];
    calibrate(&mut p, &mut c, new_bias, Some(RecalReason::Drift));
    validate(&mut p, &mut c, new_bias);
    assert_eq!(p.advice().action, RecalAction::None);
    let q = p.summary();
    assert_eq!(q.recalibrations.len(), 1);
    assert_eq!(q.recalibrations[0].reason, RecalReason::Drift);
    assert_eq!(q.recalibrations[0].new_quality, CalibrationQuality::Good);
    assert_eq!(q.calibration.attempts, 2);
    assert_eq!(q.drift_checks.len(), 2);
    assert!(q.max_drift_deg.unwrap() > 3.0);
    assert!(q.validation.unwrap().accuracy_deg.unwrap() < 0.3);
}

#[test]
fn orientation_change_rotates_geometry_and_is_advised() {
    let mut p = GazePipeline::new(config());
    let mut c = Clock::new(0.0);
    calibrate(&mut p, &mut c, [0.0, 0.0], None);
    assert_eq!(p.geometry().width_mm, 100.0);
    let mut raw = frame(
        c.tick(),
        &face(0.40, 0.0),
        [0.5, 0.5],
        [0.0, 0.0],
        Eye::Open,
    );
    raw.orientation = Orientation::LandscapeLeft;
    p.push(&raw);
    assert_eq!(p.geometry().orientation, Orientation::LandscapeLeft);
    assert_eq!(p.geometry().width_mm, 200.0);
    let a = p.advice();
    assert!(
        a.reasons.contains(&RecalReason::OrientationChanged),
        "{a:?}"
    );
    assert_eq!(p.summary().device.orientation, Orientation::LandscapeLeft);
}

// ------------------------------------------------------------------------------------------
// Session summary accounting
// ------------------------------------------------------------------------------------------

/// A recorded clip at 60 Hz, head 42 cm away (5 % from a 40 cm calibration pose): 120 frames, two 3-frame blinks (filled), one 18-frame loss.
fn record_clip(p: &mut GazePipeline, c: &mut Clock, bias: [f64; 2]) -> Vec<ProcessedSample> {
    let f = face(0.42, 0.0);
    p.set_recording(true);
    let mut out = Vec::new();
    for i in 0..120 {
        let eye = match i {
            30..=32 | 70..=72 => Eye::Closed,
            90..=107 => Eye::NoFace,
            _ => Eye::Open,
        };
        out.push(p.push(&frame(c.tick(), &f, [0.4, 0.45], bias, eye)));
    }
    p.set_recording(false);
    out
}

#[test]
fn summary_accounts_loss_valid_ratio_and_confidence_over_recorded_frames() {
    let mut p = GazePipeline::new(config());
    let mut c = Clock::new(0.0);
    calibrate(&mut p, &mut c, [0.0, 0.0], None);
    validate(&mut p, &mut c, [0.0, 0.0]);
    let rec = record_clip(&mut p, &mut c, [0.0, 0.0]);
    // Frames outside the recording do not count.
    p.push(&frame(
        c.tick(),
        &face(0.42, 0.0),
        [0.4, 0.45],
        [0.0, 0.0],
        Eye::NoFace,
    ));

    let q = p.summary();
    let n = rec.len() as f64;
    let valid = rec.iter().filter(|s| s.valid).count() as f64;
    // Blink frames: 3 + 3 plus the 50 ms recovery (3 frames) after each.
    assert_eq!(
        rec.iter()
            .filter(|s| s.track_state == TrackState::Blink)
            .count(),
        12
    );
    assert_eq!(
        rec.iter()
            .filter(|s| s.track_state == TrackState::Lost)
            .count(),
        18
    );
    assert_eq!(valid, 90.0);
    let dt = 1.0 / HZ;
    assert_close(q.loss.blink_sec, 12.0 * dt, 1e-9, "blink");
    assert_close(q.loss.track_loss_sec, 18.0 * dt, 1e-9, "loss");
    assert_close(q.loss.offscreen_sec, 0.0, 0.0, "offscreen");
    // Each blink run (6 frames, bracket 7 frames = 116.7 ms) is longer than 75 ms: not filled.
    assert_close(q.loss.gap_filled_sec, 0.0, 0.0, "filled");
    assert_close(q.loss.longest_gap_sec, 19.0 * dt, 1e-9, "longest gap");
    assert_close(
        q.confidence.mean_sample,
        1.0,
        1e-12,
        "mean confidence over tracked",
    );
    assert_eq!(q.confidence.low_confidence_sample_ratio, 0.0);
    assert!(q.confidence.mean_fixation.is_some());
    assert_close(q.device.sample_rate_hz.unwrap(), HZ, 1e-6, "rate");
    assert_eq!(q.device.screen_width_mm, 100.0);
    assert_eq!(q.calibration.attempts, 1);
    assert!(q.calibration.age_sec_at_start.unwrap() > 0.0);
    // Data loss 30/119 = 25 % > fair_above_data_loss (20 %): fair, not good (QS-4).
    assert_eq!(q.verdict, TrackingVerdict::Fair, "{q:?}");
    // The pipeline's own validRatio and data loss.
    assert_close(p.valid_ratio(), valid / n, 1e-12, "validRatio");
    assert_close(p.data_loss(), 30.0 / 119.0, 1e-9, "data loss");
}

#[test]
fn short_blinks_are_filled_for_the_summary_but_stay_invalid() {
    let mut cfg = config();
    // A 0 ms recovery makes each blink run 3 frames (bracket 66.7 ms ≤ 75 ms).
    cfg.params.track_state.blink_recovery_ms = 0.0;
    let mut p = GazePipeline::new(cfg);
    let mut c = Clock::new(0.0);
    let rec = record_clip(&mut p, &mut c, [0.0, 0.0]);
    let q = p.summary();
    assert_close(q.loss.gap_filled_sec, 6.0 / HZ, 1e-9, "6 filled frames");
    let before = p.valid_ratio();
    assert_close(
        before,
        96.0 / 120.0,
        1e-12,
        "filled frames never count as valid",
    );
    let filled = p.gap_fill(&rec);
    assert_eq!(filled.iter().filter(|s| s.gap_filled).count(), 6);
    assert_eq!(
        filled,
        gapfill::fill_processed(&rec, p.geometry(), &p.config().params.gap_fill),
        "gap fill uses the session geometry (trap 12)"
    );
}

#[test]
fn summary_conditions_match_the_summary_module() {
    let mut p = GazePipeline::new(config());
    let mut c = Clock::new(0.0);
    let rec = record_clip(&mut p, &mut c, [1.0, 0.0]);
    let q = p.summary();
    assert_eq!(q.conditions, summary::conditions(&rec, None));
    assert_eq!(q.device.sample_rate_hz, summary::sample_rate_hz(&rec));
    // Skipped calibration: recorded as such and caps the verdict.
    assert_eq!(q.calibration.quality, CalibrationQuality::Skipped);
    assert_eq!(q.calibration.attempts, 0);
}

/// CNF-1 pose-shift factor: screen models only, relative to the calibration pose distance.
#[test]
fn distance_shift_from_the_calibration_pose_lowers_confidence() {
    let mut p = GazePipeline::new(config());
    let mut c = Clock::new(0.0);
    calibrate(&mut p, &mut c, [0.0, 0.0], None); // pose 40 cm
    let at = |p: &mut GazePipeline, c: &mut Clock, d: f64| {
        p.push(&frame(
            c.tick(),
            &face(d, 0.0),
            [0.5, 0.5],
            [0.0, 0.0],
            Eye::Open,
        ))
    };
    // 48 cm = 20 % shift = poseShiftDistancePct: exactly 0.5 (distance itself is comfortable).
    assert_close(at(&mut p, &mut c, 0.48).confidence, 0.5, 1e-9, "20 % shift");
    // 46 cm = 15 %: halfway between comfort (10 %) and limit.
    assert_close(
        at(&mut p, &mut c, 0.46).confidence,
        0.75,
        1e-9,
        "15 % shift",
    );
    // Without a screen model the factor does not apply.
    p.set_calibration(Some(CalModel::identity(CalModelKind::AngularAffine)));
    assert_eq!(at(&mut p, &mut c, 0.48).confidence, 1.0);
}

#[test]
fn nothing_recorded_is_unusable() {
    let p = GazePipeline::new(config());
    let q = p.summary();
    assert_eq!(q.verdict, TrackingVerdict::Unusable);
    assert_eq!(p.valid_ratio(), 0.0);
    assert_eq!(p.data_loss(), 1.0);
}

// ------------------------------------------------------------------------------------------
// PIPE-5 determinism and the pipeline golden
// ------------------------------------------------------------------------------------------

/// A scripted session: calibration, validation, a recorded clip with blinks and loss, two
/// drift checks, then the summary. Returns every output as JSON.
fn scripted_session() -> Value {
    let mut p = GazePipeline::new(config());
    let mut c = Clock::new(1000.0);
    let bias = [1.5, -0.8];
    p.start_calibration(None, None).unwrap();
    let mut targets = Vec::new();
    while let Some(TargetStatus::Pending { target_norm, .. }) = p.calibration_status() {
        targets.push(run_target(
            &mut p,
            &mut c,
            TargetKind::Calibration,
            target_norm,
            bias,
        ));
    }
    let outcome = p.finish_calibration().unwrap();
    targets.extend(validate(&mut p, &mut c, bias));
    let clip = record_clip(&mut p, &mut c, bias);
    for _ in 0..2 {
        let t = p.next_check_target();
        targets.push(run_target(&mut p, &mut c, TargetKind::DriftCheck, t, bias));
    }
    json!({
        "calibrationRecord": outcome.record,
        "targets": targets,
        "clip": clip,
        "gapFilledClip": p.gap_fill(&clip),
        "advice": p.advice(),
        "summary": p.summary(),
    })
}

#[test]
fn two_runs_are_byte_identical_after_jcs() {
    let a = serde_jcs::to_vec(&scripted_session()).unwrap();
    let b = serde_jcs::to_vec(&scripted_session()).unwrap();
    assert_eq!(a, b);
}

#[test]
fn golden_scripted_session() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden/gaze/pipeline/scripted_session_v1.json");
    let actual = scripted_session();
    if std::env::var("ASD_WRITE_GOLDEN").as_deref() == Ok("1") {
        let doc = json!({
            "schema": "asd-golden/1",
            "function": "pipeline::GazePipeline",
            "case": "scripted_session_v1",
            "derivation": "Generated by tests/gaze_pipeline.rs::scripted_session (WP-G.3) with ASD_WRITE_GOLDEN=1: a synthetic head 40-48 cm in front of a 100 x 200 mm panel with an angular eye bias of (1.5, -0.8) deg runs a 5-point calibration, a 4-target validation, a recorded 120-frame clip with two blinks and an 18-frame loss, and two drift checks. Pins the stage order, the target statistics and the summary accounting; the invariants are asserted independently by the other tests in that file. Golden files are immutable: change = new file.",
            "expected": actual,
        });
        std::fs::write(&path, serde_json::to_string_pretty(&doc).unwrap() + "\n").unwrap();
    }
    let doc: Value = serde_json::from_str(
        &std::fs::read_to_string(&path).expect("golden file (run with ASD_WRITE_GOLDEN=1)"),
    )
    .unwrap();
    assert_eq!(doc["schema"], "asd-golden/1");
    assert_json_close(&doc["expected"], &actual, "$");
}

/// Structural equality with floats to 1e-9 (JSON text round-trips doubles exactly, so this
/// only absorbs cross-platform libm differences in sin/atan2).
fn assert_json_close(want: &Value, got: &Value, at: &str) {
    match (want, got) {
        (Value::Number(a), Value::Number(b)) => {
            let (a, b) = (a.as_f64().unwrap(), b.as_f64().unwrap());
            assert!((a - b).abs() <= 1e-9 * a.abs().max(1.0), "{at}: {a} != {b}");
        }
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len(), "{at}: length");
            for (i, (x, y)) in a.iter().zip(b).enumerate() {
                assert_json_close(x, y, &format!("{at}[{i}]"));
            }
        }
        (Value::Object(a), Value::Object(b)) => {
            let ka: Vec<_> = a.keys().collect();
            let kb: Vec<_> = b.keys().collect();
            assert_eq!(ka, kb, "{at}: keys");
            for (k, x) in a {
                assert_json_close(x, &b[k], &format!("{at}.{k}"));
            }
        }
        _ => assert_eq!(want, got, "{at}"),
    }
}

// ------------------------------------------------------------------------------------------
// Config
// ------------------------------------------------------------------------------------------

#[test]
fn config_for_model_uses_the_geometry_table_and_round_trips() {
    let cfg = PipelineConfig::for_model("iPhone17,3", Orientation::Portrait);
    assert_eq!(cfg.geometry_source, GeometrySource::Table);
    let unknown = PipelineConfig::for_model("iPhone99,9", Orientation::Portrait);
    assert_eq!(unknown.geometry_source, GeometrySource::IdiomDefault);
    let text = serde_json::to_string(&cfg).unwrap();
    let back: PipelineConfig = serde_json::from_str(&text).unwrap();
    assert_eq!(back, cfg);
    // Partial config: params and gate default.
    let partial = json!({
        "modelIdentifier": "x",
        "geometrySource": "table",
        "geometry": test_geo(),
        "orientation": "portrait",
    });
    let cfg: PipelineConfig = serde_json::from_value(partial).unwrap();
    assert_eq!(cfg.quality_gate, QualityGate::default());
    assert_eq!(cfg.params, TrackingQualityParams::default());
}

// ------------------------------------------------------------------------------------------
// PIPE-4 host timing (the device budget is measured through the FFI in WP-G.9)
// ------------------------------------------------------------------------------------------

#[test]
#[ignore = "timing; run with --release --ignored"]
fn push_cost_on_the_host() {
    let mut cfg = config();
    cfg.calibration = Some(shift_model(0.01));
    let mut p = GazePipeline::new(cfg);
    p.set_recording(true);
    let f = face(0.45, 0.0);
    let frames: Vec<RawTrackingSample> = (0..36_000)
        .map(|i| {
            let t = i as f64 / HZ;
            let eye = if i % 300 < 6 { Eye::Closed } else { Eye::Open };
            frame(
                t,
                &f,
                [0.5 + 0.2 * (t * 0.7).sin(), 0.5 + 0.2 * (t * 0.3).cos()],
                [0.5, 0.5],
                eye,
            )
        })
        .collect();
    let mut ns: Vec<u128> = Vec::with_capacity(frames.len());
    for raw in &frames {
        let start = std::time::Instant::now();
        std::hint::black_box(p.push(raw));
        ns.push(start.elapsed().as_nanos());
    }
    ns.sort_unstable();
    let p99 = ns[ns.len() * 99 / 100] as f64 / 1e6;
    let start = std::time::Instant::now();
    std::hint::black_box(p.summary());
    let summary_ms = start.elapsed().as_secs_f64() * 1e3;
    println!(
        "push p50 {:.4} ms, p99 {:.4} ms; summary over 10 min {:.2} ms",
        ns[ns.len() / 2] as f64 / 1e6,
        p99,
        summary_ms
    );
    assert!(p99 <= 0.2, "PIPE-4 budget on the host: p99 {p99} ms");
}

// ------------------------------------------------------------------------------------------
// Audit regressions (H1, M1, M2, L1, L3)
// ------------------------------------------------------------------------------------------

/// Like [`run_target`], with the head at `distance_m` and the eyes in `eye` state. Windows
/// that never collect enough close on the frame clock (`maxTargetWaitMs` for calibration).
fn run_target_as(
    p: &mut GazePipeline,
    clock: &mut Clock,
    kind: TargetKind,
    target: [f64; 2],
    bias: [f64; 2],
    distance_m: f64,
    eye: Eye,
) -> TargetResult {
    p.begin_target(kind, target).expect("begin_target");
    let f = face(distance_m, 0.0);
    for _ in 0..600 {
        p.push(&frame(clock.tick(), &f, target, bias, eye));
        if p.target_complete() {
            break;
        }
    }
    assert!(p.target_complete(), "{kind:?} window never completed");
    p.end_target().expect("end_target")
}

fn calibrate_as(
    p: &mut GazePipeline,
    clock: &mut Clock,
    bias: [f64; 2],
    distance_m: f64,
    eye: Eye,
    reason: Option<RecalReason>,
) -> CalibrationOutcome {
    p.start_calibration(None, reason).unwrap();
    while let Some(TargetStatus::Pending { target_norm, .. }) = p.calibration_status() {
        run_target_as(
            p,
            clock,
            TargetKind::Calibration,
            target_norm,
            bias,
            distance_m,
            eye,
        );
    }
    p.finish_calibration().unwrap()
}

fn validate_as(
    p: &mut GazePipeline,
    clock: &mut Clock,
    bias: [f64; 2],
    distance_m: f64,
    eye: Eye,
) -> Vec<TargetResult> {
    let layout = TrackingQualityParams::default().validation.layout;
    layout
        .into_iter()
        .map(|t| run_target_as(p, clock, TargetKind::Validation, t, bias, distance_m, eye))
        .collect()
}

/// H1: a re-calibration after the head moved more than `poseShiftDistancePct` away from the
/// previous calibration pose must not judge its own samples against that old pose (the
/// samples of a new calibration define the new pose).
#[test]
fn recalibration_after_a_large_distance_shift_is_as_good_as_a_fresh_one() {
    let bias = [1.0, -0.5];
    let mut fresh = GazePipeline::new(config());
    let mut cf = Clock::new(0.0);
    let fresh_outcome = calibrate_as(&mut fresh, &mut cf, bias, 0.62, Eye::Open, None);
    assert_eq!(fresh_outcome.record.usable_points, 5);

    let mut p = GazePipeline::new(config());
    let mut c = Clock::new(0.0);
    let first = calibrate_as(&mut p, &mut c, bias, 0.45, Eye::Open, None);
    assert_eq!(first.record.quality, CalibrationQuality::Good);
    validate_as(&mut p, &mut c, bias, 0.45, Eye::Open);
    let again = calibrate_as(
        &mut p,
        &mut c,
        bias,
        0.62,
        Eye::Open,
        Some(RecalReason::PoseShift),
    );
    assert_eq!(again.record.usable_points, 5, "{:?}", again.record);
    assert_eq!(again.record.quality, fresh_outcome.record.quality);
    assert_ne!(again.record.quality, CalibrationQuality::Failed);
    assert_close(
        again.record.pose.unwrap().distance_cm,
        62.0,
        1e-6,
        "new calibration pose",
    );
    let q = p.summary();
    assert_eq!(q.calibration.quality, fresh_outcome.record.quality);
    assert_eq!(
        q.recalibrations[0].new_quality,
        fresh_outcome.record.quality
    );
}

/// M1: a failed re-calibration leaves the applied calibration, its pose, its validation and
/// the drift reference in place; the failed attempt is only in `recalibrations[]`.
#[test]
fn failed_recalibration_keeps_the_applied_calibration_and_its_validation() {
    let bias = [1.5, -0.8];
    let mut p = GazePipeline::new(config());
    let mut c = Clock::new(0.0);
    let good = calibrate_as(&mut p, &mut c, bias, 0.40, Eye::Open, None);
    assert_eq!(good.record.quality, CalibrationQuality::Good);
    validate_as(&mut p, &mut c, bias, 0.40, Eye::Open);
    let before = p.summary();
    let model_before = p.calibration().cloned();
    let reference_before = p.drift_reference_deg();
    assert!(reference_before.is_some());

    // The child leaves: every target of the re-calibration is lost.
    let failed = calibrate_as(
        &mut p,
        &mut c,
        bias,
        0.40,
        Eye::NoFace,
        Some(RecalReason::OperatorRequest),
    );
    assert_eq!(failed.record.quality, CalibrationQuality::Failed);

    assert_eq!(
        p.calibration().cloned(),
        model_before,
        "model stays applied"
    );
    assert_eq!(p.drift_reference_deg(), reference_before);
    let q = p.summary();
    assert_eq!(q.calibration.quality, CalibrationQuality::Good);
    assert_eq!(q.calibration.model, before.calibration.model);
    assert_eq!(q.calibration.pose, before.calibration.pose);
    assert_eq!(q.calibration.usable_points, 5);
    assert_eq!(q.calibration.residual_deg, before.calibration.residual_deg);
    assert_eq!(q.calibration.attempts, 2, "attempts counts every attempt");
    assert_eq!(q.validation, before.validation, "validation still applies");
    assert_eq!(q.verdict, before.verdict);
    assert_eq!(q.recalibrations.len(), 1);
    assert_eq!(q.recalibrations[0].new_quality, CalibrationQuality::Failed);
    assert_eq!(q.recalibrations[0].reason, RecalReason::OperatorRequest);
    // QS-5 `calibratedDistanceCm` reads this pose: still the applied calibration's.
    assert_close(
        q.calibration.pose.unwrap().distance_cm,
        40.0,
        1e-6,
        "calibrated distance",
    );
    // Advice is about the applied calibration: nothing to do.
    assert_eq!(p.advice().action, RecalAction::None, "{:?}", p.advice());
}

/// M1 corner: a failed *first* calibration is still the session's calibration record.
#[test]
fn failed_first_calibration_is_recorded_and_advised() {
    let mut p = GazePipeline::new(config());
    let mut c = Clock::new(0.0);
    let failed = calibrate_as(&mut p, &mut c, [0.0, 0.0], 0.40, Eye::NoFace, None);
    assert_eq!(failed.record.quality, CalibrationQuality::Failed);
    assert!(p.calibration().is_none());
    let q = p.summary();
    assert_eq!(q.calibration.quality, CalibrationQuality::Failed);
    assert_eq!(q.calibration.attempts, 1);
    let a = p.advice();
    assert_eq!(a.action, RecalAction::Recalibrate);
    assert!(a.reasons.contains(&RecalReason::CalibrationQuality));
}

/// M2: a validation whose every target was lost has no accuracy and sets no drift
/// reference. It must advise a re-calibration, the drift checks that follow measure no drift,
/// and the verdict cannot be `good` from the drift-check fallback (QS-2) alone.
#[test]
fn validation_with_every_target_lost_advises_recalibration_and_caps_the_verdict() {
    let bias = [1.0, -0.5];
    let mut p = GazePipeline::new(config());
    let mut c = Clock::new(0.0);
    calibrate_as(&mut p, &mut c, bias, 0.40, Eye::Open, None);
    let val = validate_as(&mut p, &mut c, bias, 0.40, Eye::NoFace);
    assert!(val.iter().all(|r| r.outcome == TargetOutcome::Missed));
    let rec = p.validation().unwrap();
    assert!(!rec.complete);
    assert_eq!(rec.accuracy_deg, None);
    assert_eq!(p.drift_reference_deg(), None, "no drift reference");

    let a = p.advice();
    assert_eq!(a.action, RecalAction::Recalibrate, "{a:?}");
    assert!(
        a.reasons.contains(&RecalReason::ValidationAccuracy),
        "{a:?}"
    );

    // A clean recording with two hit drift checks.
    let f = face(0.40, 0.0);
    p.set_recording(true);
    for _ in 0..120 {
        p.push(&frame(c.tick(), &f, [0.4, 0.45], bias, Eye::Open));
    }
    p.set_recording(false);
    for _ in 0..2 {
        let t = p.next_check_target();
        let r = run_target(&mut p, &mut c, TargetKind::DriftCheck, t, bias);
        assert_eq!(r.outcome, TargetOutcome::Hit);
    }
    let q = p.summary();
    assert!(q
        .drift_checks
        .iter()
        .all(|d| !d.missed && d.drift_deg.is_none()));
    assert!(q.estimated_accuracy_deg.is_some(), "QS-2 check fallback");
    assert_eq!(q.verdict, TrackingVerdict::Fair, "{q:?}");
    assert!(p
        .advice()
        .reasons
        .contains(&RecalReason::ValidationAccuracy));
}

/// M2: an incomplete validation (2 of 4 hit) with a good accuracy still warns.
#[test]
fn incomplete_validation_warns() {
    let bias = [1.0, -0.5];
    let mut p = GazePipeline::new(config());
    let mut c = Clock::new(0.0);
    calibrate_as(&mut p, &mut c, bias, 0.40, Eye::Open, None);
    let layout = TrackingQualityParams::default().validation.layout;
    for (i, t) in layout.into_iter().enumerate() {
        let eye = if i < 2 { Eye::Open } else { Eye::NoFace };
        run_target_as(&mut p, &mut c, TargetKind::Validation, t, bias, 0.40, eye);
    }
    let rec = p.validation().unwrap();
    assert!(!rec.complete);
    assert!(rec.accuracy_deg.unwrap() < 0.5);
    let a = p.advice();
    assert_eq!(a.action, RecalAction::Warn, "{a:?}");
    assert_eq!(a.reasons, vec![RecalReason::ValidationAccuracy]);
}

/// L3: an orientation change while a calibration runs rejects that calibration (its
/// geometry no longer matches the screen): the attempt is `failed` and not applied.
#[test]
fn orientation_change_during_calibration_rejects_it() {
    let mut p = GazePipeline::new(config());
    let mut c = Clock::new(0.0);
    p.start_calibration(None, None).unwrap();
    let mut n = 0;
    while let Some(TargetStatus::Pending { target_norm, .. }) = p.calibration_status() {
        p.begin_target(TargetKind::Calibration, target_norm)
            .unwrap();
        let f = face(0.40, 0.0);
        for _ in 0..600 {
            let mut raw = frame(c.tick(), &f, target_norm, [0.0, 0.0], Eye::Open);
            if n >= 2 {
                raw.orientation = Orientation::PortraitUpsideDown;
            }
            p.push(&raw);
            if p.target_complete() {
                break;
            }
        }
        p.end_target().unwrap();
        n += 1;
    }
    let outcome = p.finish_calibration().unwrap();
    assert_eq!(outcome.record.quality, CalibrationQuality::Failed);
    assert!(
        p.calibration().is_none(),
        "a rejected calibration is not applied"
    );
    assert_eq!(p.advice().action, RecalAction::Recalibrate);
}

/// L1: frames before the first face read as "no data", not as a blink.
#[test]
fn telemetry_before_the_first_face_is_not_a_blink() {
    let mut p = GazePipeline::new(config());
    let gate = QualityGate::default();
    let mut w = TelemetryWriter::new(0.0, gate);
    let lost = p.push(&frame(
        0.0,
        &face(0.4, 0.0),
        [0.5, 0.5],
        [0.0, 0.0],
        Eye::NoFace,
    ));
    let t = w.sample(&lost);
    assert!(!t.is_valid);
    assert_eq!(t.confidence, 0.0);
    let e = w.evaluate(&t);
    assert!(!e.is_blink, "{:?}", e.failure_reasons);
    assert!(!e.is_valid);
}
