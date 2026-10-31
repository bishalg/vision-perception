//! owner: WP-G.4
//!
//! Validation target results and the validation summary (VAL-2..VAL-4). Expected values are
//! hand-built from closed forms (`atan(offset / distance)`) on a geometry whose eye sits
//! straight in front of the target, so every angle reduces to one `atan`.

use std::path::PathBuf;

use asd_core::gaze::geometry::OrientedGeometry;
use asd_core::gaze::validation::{
    accuracy_action, signed_offset_deg, summarize, target_result, TargetWindow,
};
use asd_core::types::tracking::{
    Orientation, ProcessedSample, RecalAction, TargetKind, TargetOutcome, TargetResult, TrackState,
    ValidationParams,
};
use serde_json::Value;

const EPS: f64 = 1e-12;

/// 100 mm x 200 mm portrait panel (norm 0.01 = 1 mm in x, 2 mm in y).
fn geo() -> OrientedGeometry {
    OrientedGeometry {
        orientation: Orientation::Portrait,
        width_mm: 100.0,
        height_mm: 200.0,
        camera_mm: [50.0, -5.0],
        camera_native_mm: [50.0, -5.0],
        native_width_mm: 100.0,
        native_height_mm: 200.0,
    }
}

/// Eye 500 mm in front of the panel centre (norm (0.5, 0.5)).
const EYE: [f64; 3] = [50.0, 100.0, 500.0];

fn deg(mm: f64) -> f64 {
    (mm / 500.0).atan().to_degrees()
}

fn sample(t: f64, cal: [f64; 2], filt: [f64; 2]) -> ProcessedSample {
    ProcessedSample {
        t_sec: t,
        track_state: TrackState::Tracked,
        ray_status: None,
        raw_point_norm: Some(cal),
        calibrated_point_norm: Some(cal),
        filtered_point_norm: Some(filt),
        eye_pos_mm: Some(EYE),
        distance_cm: Some(50.0),
        head_yaw_rad: Some(0.0),
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

fn lost(t: f64) -> ProcessedSample {
    ProcessedSample {
        track_state: TrackState::Lost,
        raw_point_norm: None,
        calibrated_point_norm: None,
        filtered_point_norm: None,
        eye_pos_mm: None,
        confidence: 0.0,
        valid: false,
        ..sample(t, [0.0, 0.0], [0.0, 0.0])
    }
}

fn window(min_samples: u32) -> TargetWindow {
    TargetWindow {
        kind: TargetKind::Validation,
        target_norm: [0.5, 0.5],
        started_at_sec: 10.0,
        ended_at_sec: 11.0,
        settle_ms: 400.0,
        min_samples,
        attempt: 1,
    }
}

/// `n` samples every 50 ms from `t0`, all at `p`.
fn constant(t0: f64, n: usize, p: [f64; 2]) -> Vec<ProcessedSample> {
    (0..n).map(|i| sample(t0 + 0.05 * i as f64, p, p)).collect()
}

#[test]
fn window_constructors_take_params() {
    let v = ValidationParams::default();
    let w = TargetWindow::validation([0.3, 0.7], 1.0, 2.4, &v);
    assert_eq!(w.kind, TargetKind::Validation);
    assert_eq!(w.settle_ms, 400.0);
    assert_eq!(w.min_samples, 12);
    let d = asd_core::types::tracking::DriftParams::default();
    let w = TargetWindow::drift_check([0.5, 0.5], 1.0, 2.2, &d);
    assert_eq!(w.kind, TargetKind::DriftCheck);
    assert_eq!(w.settle_ms, 300.0);
    assert_eq!(w.min_samples, 10);
}

#[test]
fn accuracy_is_angle_to_median_after_settle() {
    // Settle samples (t < 10.4) sit far away and must be ignored.
    let mut s = constant(10.0, 8, [0.9, 0.9]); // 10.00 .. 10.35
                                               // Post-settle: 12 samples, median x at 0.5 + 0.10 (10 mm right); one outlier far left.
    let mut post = constant(10.4, 11, [0.6, 0.5]);
    post.push(sample(10.95, [0.0, 0.5], [0.0, 0.5]));
    s.extend(post);
    let r = target_result(&window(12), &s, &geo());
    assert_eq!(r.outcome, TargetOutcome::Hit);
    assert_eq!(r.valid_samples, 12);
    assert_eq!(r.total_samples, 12, "settle frames are not counted");
    assert_eq!(r.calibrated_median_norm, Some([0.6, 0.5]));
    assert_eq!(r.eye_pos_mm_median, Some(EYE));
    let acc = r.accuracy_deg.unwrap();
    assert!((acc - deg(10.0)).abs() < EPS, "{acc}");
    let off = r.offset_deg.unwrap();
    assert!((off[0] - deg(10.0)).abs() < EPS);
    assert!(off[1].abs() < EPS);
}

#[test]
fn offset_vector_signs_are_x_right_y_down() {
    // Gaze 10 mm left (x -0.1) and 20 mm up (y -0.1).
    let off = signed_offset_deg(&geo(), [0.5, 0.5], [0.4, 0.4], EYE);
    assert!((off[0] + deg(10.0)).abs() < EPS);
    assert!((off[1] + deg(20.0)).abs() < EPS);
}

#[test]
fn rms_s2s_and_sd_on_alternating_sequence() {
    // Filtered x alternates 0.49 / 0.51 (+-1 mm around the centre): every consecutive
    // angle is the angle subtended by 2 mm across the centre = 2 * atan(1/500).
    // Calibrated (unfiltered) alternates 0.47 / 0.53: 2 * atan(3/500).
    let s: Vec<_> = (0..12)
        .map(|i| {
            let sign = if i % 2 == 0 { -1.0 } else { 1.0 };
            sample(
                10.4 + 0.05 * i as f64,
                [0.5 + sign * 0.03, 0.5],
                [0.5 + sign * 0.01, 0.5],
            )
        })
        .collect();
    let r = target_result(&window(12), &s, &geo());
    let step = 2.0 * deg(1.0);
    let step_raw = 2.0 * deg(3.0);
    assert!((r.precision_rms_deg.unwrap() - step).abs() < EPS);
    assert!((r.precision_rms_raw_deg.unwrap() - step_raw).abs() < EPS);
    // Centroid of the filtered points is the centre; each point is atan(1/500) from it on x.
    assert!((r.sd_deg.unwrap() - deg(1.0)).abs() < EPS);
}

#[test]
fn rms_s2s_known_three_step_sequence() {
    // Steps of 0 mm, 3 mm, 4 mm along x across the centre-ish region.
    // x norm: 0.50, 0.50, 0.53, 0.49 ... RMS of the three angles, each computed exactly.
    let xs = [0.50, 0.50, 0.53, 0.49];
    let s: Vec<_> = xs
        .iter()
        .enumerate()
        .map(|(i, &x)| sample(10.4 + 0.05 * i as f64, [x, 0.5], [x, 0.5]))
        .collect();
    let r = target_result(&window(1), &s, &geo());
    let a1 = 0.0;
    let a2 = deg(3.0); // 50 -> 53 mm, eye in front of 50
    let a3 = deg(3.0) + deg(1.0); // 53 -> 49 mm straddles the foot of the eye
    let rms = ((a1 * a1 + a2 * a2 + a3 * a3) / 3.0f64).sqrt();
    assert!((r.precision_rms_deg.unwrap() - rms).abs() < EPS);
}

#[test]
fn missed_below_sample_minimum_keeps_counts_and_drops_accuracy() {
    let s = constant(10.4, 11, [0.5, 0.5]);
    let r = target_result(&window(12), &s, &geo());
    assert_eq!(r.outcome, TargetOutcome::Missed);
    assert_eq!(r.valid_samples, 11);
    assert_eq!(r.accuracy_deg, None);
    assert_eq!(r.offset_deg, None);
    assert_eq!(r.precision_rms_deg, None);
    assert_eq!(r.sd_deg, None);
    // Exactly at the minimum is a hit.
    let s = constant(10.4, 12, [0.5, 0.5]);
    assert_eq!(
        target_result(&window(12), &s, &geo()).outcome,
        TargetOutcome::Hit
    );
}

#[test]
fn invalid_and_gap_filled_samples_do_not_count() {
    let mut s = constant(10.4, 12, [0.5, 0.5]);
    s[3].valid = false;
    s[4].gap_filled = true;
    s[5].calibrated_point_norm = None;
    let r = target_result(&window(12), &s, &geo());
    assert_eq!(r.valid_samples, 9);
    assert_eq!(r.outcome, TargetOutcome::Missed);
}

#[test]
fn data_loss_is_share_of_frame_time_not_tracked() {
    // Post-settle window 10.4 .. 11.0, frames every 0.1 s at 10.4 .. 10.9.
    // Frames at 10.6 and 10.7 are lost: 0.2 s of 0.6 s.
    let mut s = Vec::new();
    for i in 0..6 {
        let t = 10.4 + 0.1 * i as f64;
        if i == 2 || i == 3 {
            s.push(lost(t));
        } else {
            s.push(sample(t, [0.5, 0.5], [0.5, 0.5]));
        }
    }
    let r = target_result(&window(1), &s, &geo());
    assert!((r.data_loss - 0.2 / 0.6).abs() < 1e-9, "{}", r.data_loss);
}

#[test]
fn window_without_frames_is_missed_with_full_loss() {
    let r = target_result(&window(12), &[], &geo());
    assert_eq!(r.outcome, TargetOutcome::Missed);
    assert_eq!(r.valid_samples, 0);
    assert_eq!(r.total_samples, 0);
    assert_eq!(r.data_loss, 1.0);
    assert_eq!(r.calibrated_median_norm, None);
    let json = serde_json::to_value(&r).unwrap();
    for key in ["accuracyDeg", "offsetDeg", "precisionRmsDeg", "sdDeg"] {
        assert!(json.get(key).is_none(), "{key} must be absent");
    }
}

#[test]
fn off_screen_point_is_not_clamped() {
    // Gaze 0.2 left of the glass edge: x = -0.2 -> 70 mm left of the target at 0.5.
    let s = constant(10.4, 12, [-0.2, 0.5]);
    let r = target_result(&window(12), &s, &geo());
    assert_eq!(r.calibrated_median_norm, Some([-0.2, 0.5]));
    assert!((r.accuracy_deg.unwrap() - deg(70.0)).abs() < EPS);
}

fn hit(acc: f64, prec: f64, raw: f64, loss: f64) -> TargetResult {
    TargetResult {
        kind: TargetKind::Validation,
        x_norm: 0.3,
        y_norm: 0.3,
        started_at_sec: 0.0,
        ended_at_sec: 1.4,
        attempt: 1,
        outcome: TargetOutcome::Hit,
        valid_samples: 40,
        total_samples: 42,
        data_loss: loss,
        raw_median_norm: None,
        calibrated_median_norm: Some([0.3, 0.3]),
        eye_pos_mm_median: Some(EYE),
        accuracy_deg: Some(acc),
        offset_deg: Some([acc, 0.0]),
        precision_rms_deg: Some(prec),
        precision_rms_raw_deg: Some(raw),
        sd_deg: Some(prec / 2.0),
    }
}

fn missed() -> TargetResult {
    TargetResult {
        outcome: TargetOutcome::Missed,
        valid_samples: 5,
        accuracy_deg: None,
        offset_deg: None,
        precision_rms_deg: None,
        precision_rms_raw_deg: None,
        sd_deg: None,
        calibrated_median_norm: None,
        data_loss: 0.8,
        ..hit(0.0, 0.0, 0.0, 0.0)
    }
}

#[test]
fn summary_medians_over_hit_targets_complete_with_three_of_four() {
    let p = ValidationParams::default();
    let t = vec![
        hit(1.0, 0.2, 0.5, 0.0),
        hit(3.0, 0.4, 0.9, 0.1),
        missed(),
        hit(2.0, 0.3, 0.7, 0.1),
    ];
    let v = summarize(&t, &p).unwrap();
    assert!(v.complete);
    assert_eq!(v.accuracy_deg, Some(2.0));
    assert_eq!(v.precision_rms_deg, Some(0.3));
    assert_eq!(v.precision_rms_raw_deg, Some(0.7));
    assert!((v.data_loss - 0.25).abs() < EPS);
    assert_eq!(v.targets.len(), 4);
    assert!(v.targets[2].missed);
    assert_eq!(v.targets[2].accuracy_deg, None);
    assert_eq!(v.targets[2].valid_samples, 5);
    assert_eq!(v.targets[0].sd_deg, Some(0.1));
}

#[test]
fn summary_incomplete_with_two_of_four_still_recorded() {
    let p = ValidationParams::default();
    let t = vec![
        hit(1.0, 0.2, 0.5, 0.0),
        missed(),
        missed(),
        hit(2.0, 0.4, 0.6, 0.0),
    ];
    let v = summarize(&t, &p).unwrap();
    assert!(!v.complete);
    // Even count: mean of the middle two.
    assert_eq!(v.accuracy_deg, Some(1.5));
    assert!((v.precision_rms_deg.unwrap() - 0.3).abs() < EPS);
}

#[test]
fn summary_with_no_hit_has_absent_accuracy() {
    let p = ValidationParams::default();
    let v = summarize(&[missed(), missed(), missed(), missed()], &p).unwrap();
    assert!(!v.complete);
    let json = serde_json::to_value(&v).unwrap();
    for key in ["accuracyDeg", "precisionRmsDeg", "precisionRmsRawDeg"] {
        assert!(json.get(key).is_none(), "{key} must be absent, got {json}");
    }
    assert!(
        summarize(&[], &p).is_none(),
        "no targets -> no validation record"
    );
}

#[test]
fn val4_action_at_each_boundary() {
    let p = ValidationParams::default();
    assert_eq!(accuracy_action(None, &p), RecalAction::None);
    assert_eq!(accuracy_action(Some(3.5), &p), RecalAction::None);
    assert_eq!(accuracy_action(Some(3.5001), &p), RecalAction::Warn);
    assert_eq!(accuracy_action(Some(5.0), &p), RecalAction::Warn);
    assert_eq!(accuracy_action(Some(5.0001), &p), RecalAction::Recalibrate);
    assert_eq!(
        accuracy_action(Some(f64::NAN), &p),
        RecalAction::Recalibrate
    );
}

// ---------------------------------------------------------------------------------------
// Goldens: tests/golden/gaze/validation/*.json (`summarize`, exact arithmetic).
// ---------------------------------------------------------------------------------------

fn golden_files(dir: &str) -> Vec<(String, Value)> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden/gaze")
        .join(dir);
    let mut out: Vec<_> = std::fs::read_dir(&root)
        .unwrap_or_else(|e| panic!("{}: {e}", root.display()))
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .map(|p| {
            let doc: Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
            (p.file_name().unwrap().to_string_lossy().into_owned(), doc)
        })
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

#[test]
fn golden_validation_summaries() {
    let files = golden_files("validation");
    assert!(files.len() >= 2);
    for (name, doc) in files {
        assert_eq!(doc["function"], "validation::summarize", "{name}");
        let params: ValidationParams = serde_json::from_value(doc["input"]["params"].clone())
            .unwrap_or_else(|e| panic!("{name}: params: {e}"));
        let targets: Vec<TargetResult> = serde_json::from_value(doc["input"]["targets"].clone())
            .unwrap_or_else(|e| panic!("{name}: targets: {e}"));
        let actual = serde_json::to_value(summarize(&targets, &params)).unwrap();
        assert_eq!(actual, doc["expected"], "{name}");
    }
}
