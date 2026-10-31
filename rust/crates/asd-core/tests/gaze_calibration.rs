//! owner: WP-G.2
//!
//! CAL-2 .. CAL-8: calibration models, robust target aggregation, leave-one-out quality in
//! degrees, model selection and the `CalibrationSession` state machine.
//!
//! Every expected value is built by hand in this file: synthetic point sets pushed through a
//! known transform (affine, homography, 2nd-order polynomial, angular bias and gain), planted
//! outliers, and degenerate layouts. The screen-affine cases are the cases of the Swift
//! `GazeCalibrationProfileTests` except `testApplyClampsToValidRange`, which is intentionally
//! not ported: the Rust models never clamp (trap 1, G-F1).
//!
//! Camera frame of the synthetic head (`gaze::raycast::CAMERA_AXES`): metres, portrait
//! native, +x right, +y towards the camera edge, viewer at z < 0, screen plane z = 0. A
//! native screen point (mm) is camera point ((x - cx) / 1000, -(y - cy) / 1000, 0).

use std::path::PathBuf;

use asd_core::gaze::calibration::{
    self, band, calibrate, is_eligible, min_points, oriented_to_native_mm, robust_aggregate,
    AngularPair, CalError, CalModel, CalPoint, CalSample, CalibrationSession, SampleGate,
    TargetStatus, COMPLEXITY_ORDER,
};
use asd_core::gaze::geometry::{angle_deg, idiom_default, OrientedGeometry};
use asd_core::gaze::raycast::raycast_parts;
use asd_core::types::tracking::{
    CalModelKind, CalibrationParams, CalibrationQuality, DeviceGeometry, DeviceIdiom, Orientation,
    ProcessedSample, RawTrackingSample, RayStatus, TargetKind, TargetOutcome, TrackState,
};
use serde_json::Value;

type M4 = [f64; 16];

// ------------------------------------------------------------------------------------------
// Helpers
// ------------------------------------------------------------------------------------------

/// A 100 x 200 mm panel, camera 8 mm above the top edge at the centre.
fn test_geo() -> DeviceGeometry {
    let mut g = idiom_default(DeviceIdiom::Phone);
    g.screen_width_mm = 100.0;
    g.screen_height_mm = 200.0;
    g.camera_offset_mm.x = 50.0;
    g.camera_offset_mm.y = -8.0;
    g
}

fn portrait() -> OrientedGeometry {
    test_geo().rotate(Orientation::Portrait)
}

/// Eye 400 mm in front of the panel centre (oriented frame, mm).
const EYE_CENTRE: [f64; 3] = [50.0, 100.0, 400.0];

fn pt(target: [f64; 2], raw: [f64; 2]) -> CalPoint {
    CalPoint {
        target_norm: target,
        raw_norm: raw,
        eye_pos_mm: EYE_CENTRE,
        angular: None,
    }
}

fn swift_targets() -> Vec<[f64; 2]> {
    vec![
        [0.50, 0.50],
        [0.22, 0.24],
        [0.78, 0.24],
        [0.22, 0.76],
        [0.78, 0.76],
    ]
}

fn layout5() -> Vec<[f64; 2]> {
    CalibrationParams::default().layout5
}

fn layout9() -> Vec<[f64; 2]> {
    CalibrationParams::default().layout9
}

fn assert_close(a: f64, b: f64, eps: f64, what: &str) {
    assert!((a - b).abs() <= eps, "{what}: {a} != {b} (eps {eps})");
}

fn assert_pt(a: [f64; 2], b: [f64; 2], eps: f64, what: &str) {
    assert!(
        (a[0] - b[0]).abs() <= eps && (a[1] - b[1]).abs() <= eps,
        "{what}: {a:?} != {b:?}"
    );
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

/// Rotation about the camera y axis by `deg` (column-major) plus translation.
fn yaw_rot(deg: f64, t: [f64; 3]) -> M4 {
    let (s, c) = deg.to_radians().sin_cos();
    let mut m = translation(t);
    // columns: x' = (c, 0, -s), y' = (0, 1, 0), z' = (s, 0, c)
    m[0] = c;
    m[2] = -s;
    m[8] = s;
    m[10] = c;
    m
}

/// Inverse of a rigid column-major transform applied to a point.
fn rigid_inverse_apply(m: &M4, p: [f64; 3]) -> [f64; 3] {
    let d = [p[0] - m[12], p[1] - m[13], p[2] - m[14]];
    // R^T d: rows of R^T are the columns of R.
    [
        m[0] * d[0] + m[1] * d[1] + m[2] * d[2],
        m[4] * d[0] + m[5] * d[1] + m[6] * d[2],
        m[8] * d[0] + m[9] * d[1] + m[10] * d[2],
    ]
}

/// Eye transforms in the face frame; midpoint (0, 0.025, 0.01).
const EYE_MID_FACE: [f64; 3] = [0.0, 0.025, 0.01];
fn eyes() -> (M4, M4) {
    (
        translation([0.031, 0.025, 0.01]),
        translation([-0.031, 0.025, 0.01]),
    )
}

/// Yaw/pitch (deg) of a face-frame direction: yaw = atan2(x, z), pitch = atan2(y, sqrt(x²+z²))
/// (trap 4).
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

/// Camera point of a portrait screen-normalised point on `test_geo`.
fn screen_cam(g: &DeviceGeometry, n: [f64; 2]) -> [f64; 3] {
    let mm = [n[0] * g.screen_width_mm, n[1] * g.screen_height_mm];
    [
        (mm[0] - g.camera_offset_mm.x) / 1000.0,
        -(mm[1] - g.camera_offset_mm.y) / 1000.0,
        0.0,
    ]
}

/// The synthetic user's eye model error: measured = gain * true + bias, per axis (deg).
#[derive(Clone, Copy)]
struct EyeError {
    gain: [f64; 2],
    bias: [f64; 2],
}

/// A raw ARKit-like frame of a head with pose `face` looking at screen point `target`, with the
/// user's eye-model error applied to the eye-in-head direction.
fn synth_frame(face: &M4, target: [f64; 2], err: EyeError, t_sec: f64) -> RawTrackingSample {
    let g = test_geo();
    let tgt_face = rigid_inverse_apply(face, screen_cam(&g, target));
    let d = [
        tgt_face[0] - EYE_MID_FACE[0],
        tgt_face[1] - EYE_MID_FACE[1],
        tgt_face[2] - EYE_MID_FACE[2],
    ];
    let true_a = angles_of(d);
    let meas = [
        err.gain[0] * true_a[0] + err.bias[0],
        err.gain[1] * true_a[1] + err.bias[1],
    ];
    let md = dir_of(meas);
    let look_at = [
        EYE_MID_FACE[0] + 0.5 * md[0],
        EYE_MID_FACE[1] + 0.5 * md[1],
        EYE_MID_FACE[2] + 0.5 * md[2],
    ];
    let (le, re) = eyes();
    RawTrackingSample {
        t_sec,
        face_tracked: true,
        face_transform: Some(*face),
        look_at: Some(look_at),
        left_eye_transform: Some(le),
        right_eye_transform: Some(re),
        eye_blink_left: Some(0.0),
        eye_blink_right: Some(0.0),
        ambient_intensity: None,
        ambient_color_temp: None,
        light_direction: None,
        orientation: Orientation::Portrait,
        clip_media_time_sec: None,
        stimulus_id: None,
    }
}

/// Calibration point of a synthetic frame (raycast + angular pair).
fn synth_point(face: &M4, target: [f64; 2], err: EyeError, geo: &OrientedGeometry) -> CalPoint {
    let s = synth_frame(face, target, err, 0.0);
    let hit = raycast_parts(
        s.face_transform.as_ref().unwrap(),
        s.look_at.as_ref().unwrap(),
        s.left_eye_transform.as_ref(),
        s.right_eye_transform.as_ref(),
        geo,
    );
    assert_eq!(hit.status, RayStatus::Ok);
    CalPoint {
        target_norm: target,
        raw_norm: hit.point_norm.unwrap(),
        eye_pos_mm: hit.eye_pos_mm.unwrap(),
        angular: calibration::angular_pair(&s, geo, target),
    }
}

/// Face 40 cm in front of the panel centre, identity rotation (camera metres).
fn face_at_centre(dz_m: f64, dx_m: f64) -> M4 {
    let c = screen_cam(&test_geo(), [0.5, 0.5]);
    translation([c[0] + dx_m, c[1], -0.40 + dz_m])
}

fn processed(t: f64, state: TrackState, raw: Option<[f64; 2]>, conf: f64) -> ProcessedSample {
    ProcessedSample {
        t_sec: t,
        track_state: state,
        ray_status: raw.map(|_| RayStatus::Ok),
        raw_point_norm: raw,
        calibrated_point_norm: raw,
        filtered_point_norm: None,
        eye_pos_mm: raw.map(|_| EYE_CENTRE),
        distance_cm: raw.map(|_| 40.0),
        head_yaw_rad: raw.map(|_| 0.1),
        head_pitch_rad: raw.map(|_| -0.05),
        head_roll_rad: raw.map(|_| 0.0),
        head_speed_deg_per_sec: raw.map(|_| 5.0),
        eye_speed_deg_per_sec: None,
        left_eye_openness: None,
        right_eye_openness: None,
        ambient_intensity: None,
        confidence: conf,
        valid: state == TrackState::Tracked && conf >= 0.5,
        gap_filled: false,
        active_aoi_names: vec![],
        stimulus_id: None,
        clip_media_time_sec: None,
    }
}

fn gate() -> SampleGate {
    SampleGate {
        min_confidence: 0.5,
        max_head_speed_deg_per_sec: 120.0,
    }
}

// ------------------------------------------------------------------------------------------
// screenAffine: parity with GazeCalibrationProfileTests (except clamping)
// ------------------------------------------------------------------------------------------

#[test]
fn screen_affine_identity_is_passthrough() {
    let m = CalModel::identity(CalModelKind::ScreenAffine);
    assert_pt(
        m.apply_point([0.31, 0.72]).unwrap(),
        [0.31, 0.72],
        1e-12,
        "id",
    );
}

#[test]
fn screen_affine_corrects_constant_offset_bias() {
    let pts: Vec<_> = swift_targets()
        .into_iter()
        .map(|t| pt(t, [t[0] + 0.12, t[1] - 0.08]))
        .collect();
    let m = CalModel::fit(CalModelKind::ScreenAffine, &pts).unwrap();
    for p in &pts {
        assert_pt(
            m.apply_point(p.raw_norm).unwrap(),
            p.target_norm,
            1e-6,
            "offset",
        );
    }
    let CalModel::ScreenAffine { x, y } = m else {
        panic!("kind")
    };
    // x' = x - 0.12, y' = y + 0.08
    for (a, b) in x.iter().zip([1.0, 0.0, -0.12]) {
        assert_close(*a, b, 1e-9, "x coeff");
    }
    for (a, b) in y.iter().zip([0.0, 1.0, 0.08]) {
        assert_close(*a, b, 1e-9, "y coeff");
    }
}

#[test]
fn screen_affine_corrects_scale_and_offset() {
    let pts: Vec<_> = swift_targets()
        .into_iter()
        .map(|t| {
            pt(
                t,
                [
                    0.5 + (t[0] - 0.5) * 1.3 + 0.03,
                    0.5 + (t[1] - 0.5) * 1.3 - 0.02,
                ],
            )
        })
        .collect();
    let m = CalModel::fit(CalModelKind::ScreenAffine, &pts).unwrap();
    for p in &pts {
        assert_pt(
            m.apply_point(p.raw_norm).unwrap(),
            p.target_norm,
            1e-6,
            "scale",
        );
    }
}

#[test]
fn screen_affine_fails_with_too_few_points() {
    let pts = vec![pt([0.5, 0.5], [0.5, 0.5]), pt([0.2, 0.2], [0.2, 0.2])];
    assert_eq!(
        CalModel::fit(CalModelKind::ScreenAffine, &pts),
        Err(CalError::TooFewPoints { needed: 3, got: 2 })
    );
}

#[test]
fn screen_affine_fails_with_mismatched_counts() {
    let raw = [[0.5, 0.5], [0.2, 0.2], [0.8, 0.2]];
    let targets = [[0.5, 0.5], [0.2, 0.2]];
    assert_eq!(
        CalPoint::from_pairs(&raw, &targets, EYE_CENTRE),
        Err(CalError::MismatchedCounts { raw: 3, targets: 2 })
    );
}

#[test]
fn screen_affine_fails_with_degenerate_identical_points() {
    let pts: Vec<_> = swift_targets()
        .into_iter()
        .map(|t| pt(t, [0.5, 0.5]))
        .collect();
    assert_eq!(
        CalModel::fit(CalModelKind::ScreenAffine, &pts),
        Err(CalError::Degenerate)
    );
}

#[test]
fn screen_affine_fails_with_collinear_points() {
    let pts: Vec<_> = [[0.1, 0.1], [0.5, 0.5], [0.9, 0.9], [0.3, 0.3]]
        .into_iter()
        .map(|t| pt(t, t))
        .collect();
    assert_eq!(
        CalModel::fit(CalModelKind::ScreenAffine, &pts),
        Err(CalError::Degenerate)
    );
}

#[test]
fn screen_affine_does_not_clamp() {
    // Swift testApplyClampsToValidRange expected 1.0; the Rust model extrapolates (trap 1).
    let pts: Vec<_> = swift_targets()
        .into_iter()
        .map(|t| pt([2.0 * t[0] - 0.5, t[1]], t))
        .collect();
    let m = CalModel::fit(CalModelKind::ScreenAffine, &pts).unwrap();
    assert_pt(m.apply_point([0.95, 0.5]).unwrap(), [1.4, 0.5], 1e-9, "x");
    assert_pt(m.apply_point([-0.1, 1.2]).unwrap(), [-0.7, 1.2], 1e-9, "x");
}

#[test]
fn from_pairs_builds_points() {
    let pts = CalPoint::from_pairs(&[[0.1, 0.2]], &[[0.3, 0.4]], EYE_CENTRE).unwrap();
    assert_eq!(pts, vec![pt([0.3, 0.4], [0.1, 0.2])]);
}

// ------------------------------------------------------------------------------------------
// Homography and poly2
// ------------------------------------------------------------------------------------------

/// A known projective map with real perspective.
fn h_apply(p: [f64; 2]) -> [f64; 2] {
    let (x, y) = (p[0], p[1]);
    let w = 0.3 * x - 0.2 * y + 1.0;
    [
        (1.1 * x + 0.05 * y + 0.02) / w,
        (-0.04 * x + 0.9 * y + 0.07) / w,
    ]
}

#[test]
fn homography_is_exact_on_four_points_and_generalises() {
    // targets = H(raw): fit maps raw -> target.
    let raws = [[0.12, 0.12], [0.88, 0.12], [0.12, 0.88], [0.88, 0.88]];
    let pts: Vec<_> = raws.iter().map(|&r| pt(h_apply(r), r)).collect();
    let m = CalModel::fit(CalModelKind::ScreenHomography, &pts).unwrap();
    for p in &pts {
        assert_pt(
            m.apply_point(p.raw_norm).unwrap(),
            p.target_norm,
            1e-9,
            "fit pt",
        );
    }
    // A homography is fixed by four points: every other point follows exactly.
    for r in [[0.5, 0.5], [0.3, 0.7], [-0.2, 1.1]] {
        assert_pt(m.apply_point(r).unwrap(), h_apply(r), 1e-9, "other");
    }
}

#[test]
fn homography_exact_on_nine_points_least_squares() {
    let pts: Vec<_> = layout9().into_iter().map(|r| pt(h_apply(r), r)).collect();
    let m = CalModel::fit(CalModelKind::ScreenHomography, &pts).unwrap();
    assert_pt(
        m.apply_point([0.4, 0.6]).unwrap(),
        h_apply([0.4, 0.6]),
        1e-9,
        "9",
    );
}

#[test]
fn homography_three_collinear_of_four_is_degenerate() {
    let raws = [[0.1, 0.1], [0.5, 0.5], [0.9, 0.9], [0.9, 0.1]];
    let pts: Vec<_> = raws.iter().map(|&r| pt(r, r)).collect();
    assert_eq!(
        CalModel::fit(CalModelKind::ScreenHomography, &pts),
        Err(CalError::Degenerate)
    );
}

fn poly_apply(p: [f64; 2]) -> [f64; 2] {
    let (x, y) = (p[0], p[1]);
    [
        0.01 + 0.9 * x + 0.05 * y + 0.2 * x * y - 0.1 * x * x + 0.03 * y * y,
        -0.02 + 0.04 * x + 1.1 * y - 0.15 * x * y + 0.05 * x * x - 0.12 * y * y,
    ]
}

#[test]
fn poly2_is_exact_on_six_points() {
    let mut raws = layout5();
    raws.push([0.5, 0.12]);
    let pts: Vec<_> = raws.iter().map(|&r| pt(poly_apply(r), r)).collect();
    let m = CalModel::fit(CalModelKind::ScreenPoly2, &pts).unwrap();
    for r in raws.iter().copied().chain([[0.3, 0.6], [0.7, 0.2]]) {
        assert_pt(m.apply_point(r).unwrap(), poly_apply(r), 1e-9, "poly");
    }
}

#[test]
fn poly2_is_exact_on_nine_points() {
    let pts: Vec<_> = layout9()
        .into_iter()
        .map(|r| pt(poly_apply(r), r))
        .collect();
    let m = CalModel::fit(CalModelKind::ScreenPoly2, &pts).unwrap();
    assert_pt(
        m.apply_point([0.33, 0.66]).unwrap(),
        poly_apply([0.33, 0.66]),
        1e-9,
        "9",
    );
}

#[test]
fn poly2_with_five_points_is_too_few_and_on_a_conic_is_degenerate() {
    let pts: Vec<_> = layout5().into_iter().map(|r| pt(r, r)).collect();
    assert_eq!(
        CalModel::fit(CalModelKind::ScreenPoly2, &pts),
        Err(CalError::TooFewPoints { needed: 6, got: 5 })
    );
    // Six points on the unit circle around (0.5, 0.5) lie on one conic: x² + y² is a
    // combination of the other monomials, so the design matrix is singular.
    let circle: Vec<_> = (0..6)
        .map(|i| {
            let a = i as f64 * std::f64::consts::PI / 3.0;
            let r = [0.5 + 0.3 * a.cos(), 0.5 + 0.3 * a.sin()];
            pt(r, r)
        })
        .collect();
    assert_eq!(
        CalModel::fit(CalModelKind::ScreenPoly2, &circle),
        Err(CalError::Degenerate)
    );
}

#[test]
fn model_json_roundtrip_is_tagged_camel_case() {
    let pts: Vec<_> = layout9().into_iter().map(|r| pt(h_apply(r), r)).collect();
    for kind in COMPLEXITY_ORDER {
        if kind == CalModelKind::AngularAffine {
            continue;
        }
        let m = CalModel::fit(kind, &pts).unwrap();
        let v = serde_json::to_value(&m).unwrap();
        assert_eq!(v["kind"], serde_json::to_value(kind).unwrap());
        let back: CalModel = serde_json::from_value(v).unwrap();
        assert_eq!(back, m);
    }
}

// ------------------------------------------------------------------------------------------
// angularAffine
// ------------------------------------------------------------------------------------------

const ERR: EyeError = EyeError {
    gain: [1.15, 0.9],
    bias: [5.0, -4.0],
};

#[test]
fn oriented_to_native_inverts_rotation_in_every_orientation() {
    let g = test_geo();
    for o in Orientation::ALL {
        let og = g.rotate(o);
        for p in [[0.0, 0.0], [12.0, 170.0], [100.0, 200.0], [-5.0, 230.0]] {
            let back = oriented_to_native_mm(&og, og.native_to_oriented_mm(p));
            assert_pt(back, p, 1e-12, "roundtrip");
        }
    }
}

#[test]
fn gaze_angles_follow_trap_4() {
    // look-at straight ahead of the eye midpoint: yaw = pitch = 0.
    let face = face_at_centre(0.0, 0.0);
    let mut s = synth_frame(
        &face,
        [0.5, 0.5],
        EyeError {
            gain: [1.0, 1.0],
            bias: [0.0, 0.0],
        },
        0.0,
    );
    s.look_at = Some([
        EYE_MID_FACE[0] + 0.2,
        EYE_MID_FACE[1] + 0.1,
        EYE_MID_FACE[2] + 0.2,
    ]);
    let a = calibration::gaze_angles_deg(&s).unwrap();
    assert_close(a[0], 45.0, 1e-12, "yaw = atan2(x, z)");
    // pitch = atan2(0.1, sqrt(0.2² + 0.2²)) = atan(1 / (2√2))
    assert_close(
        a[1],
        (1.0 / (2.0 * 2f64.sqrt())).atan().to_degrees(),
        1e-12,
        "pitch",
    );
}

#[test]
fn target_angles_account_for_head_rotation() {
    // Head turned 10° about y: the target straight ahead in camera space appears at -10° yaw
    // in the face frame (hand-derived with the rigid inverse).
    let g = test_geo();
    let geo = portrait();
    let c = screen_cam(&g, [0.5, 0.5]);
    let face = yaw_rot(10.0, [c[0], c[1], -0.4]);
    let s = synth_frame(&face, [0.5, 0.5], ERR, 0.0);
    let got = calibration::target_angles_deg(&s, &geo, [0.5, 0.5]).unwrap();
    let tgt_face = rigid_inverse_apply(&face, c);
    let want = angles_of([
        tgt_face[0] - EYE_MID_FACE[0],
        tgt_face[1] - EYE_MID_FACE[1],
        tgt_face[2] - EYE_MID_FACE[2],
    ]);
    assert_pt(got, want, 1e-9, "target angles");
    assert!(
        got[0] < -9.0 && got[0] > -11.0,
        "about -10 deg, got {}",
        got[0]
    );
}

#[test]
fn angular_affine_recovers_bias_and_gain_exactly() {
    let geo = portrait();
    let face = face_at_centre(0.0, 0.0);
    let pts: Vec<_> = layout5()
        .into_iter()
        .map(|t| synth_point(&face, t, ERR, &geo))
        .collect();
    let m = CalModel::fit(CalModelKind::AngularAffine, &pts).unwrap();
    let CalModel::AngularAffine { yaw, pitch } = m.clone() else {
        panic!("kind")
    };
    // true = (measured - bias) / gain
    let want_yaw = [1.0 / 1.15, 0.0, -5.0 / 1.15];
    let want_pitch = [0.0, 1.0 / 0.9, 4.0 / 0.9];
    for (got, want) in yaw.iter().zip(want_yaw) {
        assert_close(*got, want, 1e-9, "yaw coeff");
    }
    for (got, want) in pitch.iter().zip(want_pitch) {
        assert_close(*got, want, 1e-9, "pitch coeff");
    }
    let r = calibration::evaluate(CalModelKind::AngularAffine, &pts, &geo).unwrap();
    assert!(r.residual_deg < 1e-9, "residual {}", r.residual_deg);
    assert!(r.loo_cv_deg.unwrap() < 1e-9);
}

#[test]
fn angular_identity_apply_sample_equals_raycast() {
    let geo = portrait();
    let face = yaw_rot(7.0, [0.01, -0.09, -0.45]);
    let s = synth_frame(&face, [0.3, 0.6], ERR, 0.0);
    let id = CalModel::identity(CalModelKind::AngularAffine);
    let a = id.apply_sample(&s, &geo).unwrap();
    let b = raycast_parts(
        s.face_transform.as_ref().unwrap(),
        s.look_at.as_ref().unwrap(),
        s.left_eye_transform.as_ref(),
        s.right_eye_transform.as_ref(),
        &geo,
    );
    assert_pt(a.point_norm.unwrap(), b.point_norm.unwrap(), 1e-12, "id");
}

#[test]
fn angular_affine_survives_10cm_head_move_screen_affine_does_not() {
    let geo = portrait();
    let cal_face = face_at_centre(0.0, 0.0);
    let pts: Vec<_> = layout5()
        .into_iter()
        .map(|t| synth_point(&cal_face, t, ERR, &geo))
        .collect();
    let ang = CalModel::fit(CalModelKind::AngularAffine, &pts).unwrap();
    let scr = CalModel::fit(CalModelKind::ScreenAffine, &pts).unwrap();

    // Head moves 10 cm closer after calibration (40 -> 30 cm).
    let val_face = face_at_centre(0.10, 0.0);
    let mut ang_err: Vec<f64> = vec![];
    let mut scr_err: Vec<f64> = vec![];
    for t in [[0.30, 0.30], [0.70, 0.30], [0.30, 0.70], [0.70, 0.70]] {
        let s = synth_frame(&val_face, t, ERR, 0.0);
        let hit = ang.apply_sample(&s, &geo).unwrap();
        let eye = hit.eye_pos_mm.unwrap();
        let p_ang = hit.point_norm.unwrap();
        let p_scr = scr.apply_sample(&s, &geo).unwrap().point_norm.unwrap();
        let to3 = |p: [f64; 2]| [p[0] * geo.width_mm, p[1] * geo.height_mm, 0.0];
        ang_err.push(angle_deg(to3(p_ang), to3(t), eye));
        scr_err.push(angle_deg(to3(p_scr), to3(t), eye));
    }
    ang_err.sort_by(f64::total_cmp);
    scr_err.sort_by(f64::total_cmp);
    let med = |v: &[f64]| (v[1] + v[2]) / 2.0;
    assert!(med(&ang_err) < 0.3, "angular {ang_err:?}");
    assert!(med(&scr_err) > 1.0, "screen {scr_err:?}");
}

#[test]
fn angular_requires_angular_data() {
    let pts: Vec<_> = layout5().into_iter().map(|r| pt(r, r)).collect();
    assert_eq!(
        CalModel::fit(CalModelKind::AngularAffine, &pts),
        Err(CalError::MissingAngularData)
    );
    // angles only on screen models: apply_point on an angular model is None.
    assert!(CalModel::identity(CalModelKind::AngularAffine)
        .apply_point([0.5, 0.5])
        .is_none());
}

#[test]
fn angular_pair_absent_without_face() {
    let geo = portrait();
    let mut s = synth_frame(&face_at_centre(0.0, 0.0), [0.5, 0.5], ERR, 0.0);
    s.face_transform = None;
    assert!(calibration::angular_pair(&s, &geo, [0.5, 0.5]).is_none());
}

// ------------------------------------------------------------------------------------------
// Eligibility, residual, LOO, bands, condition number
// ------------------------------------------------------------------------------------------

#[test]
fn min_points_and_eligibility() {
    use CalModelKind::*;
    assert_eq!(min_points(ScreenAffine), 3);
    assert_eq!(min_points(AngularAffine), 3);
    assert_eq!(min_points(ScreenHomography), 4);
    assert_eq!(min_points(ScreenPoly2), 6);
    let p = CalibrationParams::default();
    // 5 points: only the affine models; 9 points: all four.
    assert!(is_eligible(ScreenAffine, 5, &p));
    assert!(is_eligible(AngularAffine, 5, &p));
    assert!(!is_eligible(ScreenHomography, 5, &p));
    assert!(is_eligible(ScreenHomography, 6, &p));
    assert!(!is_eligible(ScreenPoly2, 7, &p));
    assert!(is_eligible(ScreenPoly2, 8, &p));
    for k in COMPLEXITY_ORDER {
        assert!(is_eligible(k, 9, &p));
        assert!(!is_eligible(k, 4, &p) || k == ScreenAffine || k == AngularAffine);
    }
    assert!(!is_eligible(ScreenAffine, 4, &p));
}

#[test]
fn point_error_is_visual_angle_in_degrees() {
    // Identity model, eye 400 mm in front of the centre; raw 7 mm right of the target at the
    // centre: error = atan(7 / 400).
    let geo = portrait();
    let p = pt([0.5, 0.5], [0.5 + 7.0 / 100.0, 0.5]);
    let id = CalModel::identity(CalModelKind::ScreenAffine);
    let e = id.point_error_deg(&p, &geo).unwrap();
    assert_close(e, (7.0f64 / 400.0).atan().to_degrees(), 1e-12, "deg");
    // 400 mm tan(1°) on a 200 mm tall panel = 1°.
    let dy = 400.0 * 1f64.to_radians().tan() / 200.0;
    let q = pt([0.5, 0.5], [0.5, 0.5 + dy]);
    assert_close(id.point_error_deg(&q, &geo).unwrap(), 1.0, 1e-12, "1 deg");
}

#[test]
fn exact_data_has_zero_residual_and_loo() {
    let geo = portrait();
    let pts: Vec<_> = layout5()
        .into_iter()
        .map(|t| {
            pt(
                t,
                [
                    0.9 * t[0] + 0.1 * t[1] + 0.05,
                    -0.05 * t[0] + 1.1 * t[1] - 0.02,
                ],
            )
        })
        .collect();
    let r = calibration::evaluate(CalModelKind::ScreenAffine, &pts, &geo).unwrap();
    assert!(r.residual_deg < 1e-9);
    assert!(r.loo_cv_deg.unwrap() < 1e-9);
    assert_eq!(r.usable_points, 5);
    assert!(r.condition_number >= 1.0 && r.condition_number < 10.0);
}

#[test]
fn loo_is_median_of_left_out_errors_and_residual_is_median_of_fit_errors() {
    // One displaced raw point: hand check of the fold that leaves it out, brute-force check
    // of the median definition for the others.
    let geo = portrait();
    let mut pts: Vec<_> = layout5().into_iter().map(|t| pt(t, t)).collect();
    pts[1].raw_norm = [0.12 + 0.03, 0.12];
    let r = calibration::evaluate(CalModelKind::ScreenAffine, &pts, &geo).unwrap();

    let mut folds = vec![];
    for i in 0..pts.len() {
        let rest: Vec<_> = pts
            .iter()
            .enumerate()
            .filter(|(j, _)| *j != i)
            .map(|(_, p)| p.clone())
            .collect();
        let m = CalModel::fit(CalModelKind::ScreenAffine, &rest).unwrap();
        folds.push(m.point_error_deg(&pts[i], &geo).unwrap());
    }
    // Leaving out the displaced point, the other four are exact: the model is the identity
    // and the left-out error is the 3 mm displacement seen from the eye.
    let p_mm = [0.15 * 100.0, 0.12 * 200.0, 0.0];
    let q_mm = [0.12 * 100.0, 0.12 * 200.0, 0.0];
    assert_close(folds[1], angle_deg(p_mm, q_mm, EYE_CENTRE), 1e-9, "fold 1");
    folds.sort_by(f64::total_cmp);
    assert_close(r.loo_cv_deg.unwrap(), folds[2], 1e-12, "median fold");

    let mut fit_err: Vec<f64> = pts
        .iter()
        .map(|p| r.model.point_error_deg(p, &geo).unwrap())
        .collect();
    fit_err.sort_by(f64::total_cmp);
    assert_close(r.residual_deg, fit_err[2], 1e-12, "median residual");
    assert!(r.loo_cv_deg.unwrap() > r.residual_deg);
}

#[test]
fn quality_bands_at_each_threshold_edge() {
    let p = CalibrationParams::default();
    let b = |loo: f64| band(Some(loo), 5, 10.0, &p);
    assert_eq!(b(0.0), CalibrationQuality::Good);
    assert_eq!(b(2.5), CalibrationQuality::Good);
    assert_eq!(b(2.500_000_1), CalibrationQuality::Fair);
    assert_eq!(b(3.5), CalibrationQuality::Fair);
    assert_eq!(b(3.500_000_1), CalibrationQuality::Poor);
    assert_eq!(b(5.0), CalibrationQuality::Poor);
    assert_eq!(b(5.000_000_1), CalibrationQuality::Failed);
    assert_eq!(band(None, 5, 10.0, &p), CalibrationQuality::Failed);
    assert_eq!(
        band(Some(f64::NAN), 5, 10.0, &p),
        CalibrationQuality::Failed
    );
    // usablePoints < 4 fails; 4 does not.
    assert_eq!(band(Some(1.0), 3, 10.0, &p), CalibrationQuality::Failed);
    assert_eq!(band(Some(1.0), 4, 10.0, &p), CalibrationQuality::Good);
    // conditionNumber > 1e6 fails; exactly 1e6 does not.
    assert_eq!(band(Some(1.0), 5, 1e6, &p), CalibrationQuality::Good);
    assert_eq!(
        band(Some(1.0), 5, 1.000_001e6, &p),
        CalibrationQuality::Failed
    );
}

#[test]
fn condition_number_guard_fails_a_nearly_collinear_layout() {
    // Raw points spread 1e-8 in y: the fit still solves, the condition number trips the guard.
    let geo = portrait();
    let pts: Vec<_> = [0.1, 0.3, 0.5, 0.7, 0.9]
        .iter()
        .enumerate()
        .map(|(i, &x)| {
            let wiggle = if i % 2 == 0 { 1e-8 } else { -1e-8 };
            pt([x, 0.5 + wiggle * 1e6], [x, 0.5 + wiggle])
        })
        .collect();
    let r = calibration::evaluate(CalModelKind::ScreenAffine, &pts, &geo).unwrap();
    assert!(r.condition_number > 1e6, "cond {}", r.condition_number);
    let fit = calibrate(&pts, &CalibrationParams::default(), &geo).unwrap();
    assert_eq!(fit.quality, CalibrationQuality::Failed);
}

// ------------------------------------------------------------------------------------------
// Model selection (CAL-6)
// ------------------------------------------------------------------------------------------

fn auto_params() -> CalibrationParams {
    CalibrationParams {
        auto_select: true,
        ..CalibrationParams::default()
    }
}

/// Projective map with perspective strength `k`.
fn persp(k: f64) -> impl Fn([f64; 2]) -> [f64; 2] {
    move |p: [f64; 2]| {
        let w = 1.0 + k * (p[0] - 0.5);
        [0.5 + (p[0] - 0.5) / w, 0.5 + (p[1] - 0.5) / w]
    }
}

#[test]
fn default_model_only_without_auto_select() {
    let geo = portrait();
    let f = persp(0.6);
    let pts: Vec<_> = layout9().into_iter().map(|r| pt(f(r), r)).collect();
    let fit = calibrate(&pts, &CalibrationParams::default(), &geo).unwrap();
    assert_eq!(fit.selected.model.kind(), CalModelKind::ScreenAffine);
    assert_eq!(fit.candidates.len(), 1);
}

#[test]
fn auto_select_picks_the_better_model_when_gain_is_large() {
    let geo = portrait();
    let f = persp(0.6);
    let pts: Vec<_> = layout9().into_iter().map(|r| pt(f(r), r)).collect();
    let fit = calibrate(&pts, &auto_params(), &geo).unwrap();
    let aff = fit
        .candidates
        .iter()
        .find(|c| c.model.kind() == CalModelKind::ScreenAffine)
        .unwrap();
    assert!(aff.loo_cv_deg.unwrap() > 0.3 + 1e-6, "{:?}", aff.loo_cv_deg);
    assert_eq!(fit.selected.model.kind(), CalModelKind::ScreenHomography);
    assert!(fit.selected.loo_cv_deg.unwrap() < 1e-9);
    // Angular has no data here and is not a candidate; the other three are.
    assert_eq!(fit.candidates.len(), 3);
}

#[test]
fn auto_select_picks_the_simpler_model_when_gain_is_below_threshold() {
    let geo = portrait();
    let f = persp(0.02);
    let pts: Vec<_> = layout9().into_iter().map(|r| pt(f(r), r)).collect();
    let fit = calibrate(&pts, &auto_params(), &geo).unwrap();
    let aff = fit
        .candidates
        .iter()
        .find(|c| c.model.kind() == CalModelKind::ScreenAffine)
        .unwrap();
    let hom = fit
        .candidates
        .iter()
        .find(|c| c.model.kind() == CalModelKind::ScreenHomography)
        .unwrap();
    let gain = aff.loo_cv_deg.unwrap() - hom.loo_cv_deg.unwrap();
    assert!(gain > 0.0 && gain < 0.3, "gain {gain}");
    assert_eq!(fit.selected.model.kind(), CalModelKind::ScreenAffine);
}

#[test]
fn auto_select_with_five_points_considers_only_affine_models() {
    let geo = portrait();
    let face = face_at_centre(0.0, 0.0);
    let pts: Vec<_> = layout5()
        .into_iter()
        .map(|t| synth_point(&face, t, ERR, &geo))
        .collect();
    let fit = calibrate(&pts, &auto_params(), &geo).unwrap();
    let kinds: Vec<_> = fit.candidates.iter().map(|c| c.model.kind()).collect();
    assert_eq!(
        kinds,
        vec![CalModelKind::ScreenAffine, CalModelKind::AngularAffine]
    );
}

#[test]
fn calibrate_fails_when_default_cannot_fit() {
    let geo = portrait();
    let pts: Vec<_> = layout5().into_iter().map(|t| pt(t, [0.5, 0.5])).collect();
    assert_eq!(
        calibrate(&pts, &CalibrationParams::default(), &geo).map(|_| ()),
        Err(CalError::Degenerate)
    );
}

// ------------------------------------------------------------------------------------------
// Robust per-target aggregation (CAL-3)
// ------------------------------------------------------------------------------------------

fn cs(raw: [f64; 2]) -> CalSample {
    CalSample {
        t_sec: 0.0,
        raw_norm: raw,
        eye_pos_mm: EYE_CENTRE,
        distance_cm: Some(40.0),
        head_yaw_deg: Some(1.0),
        head_pitch_deg: Some(-2.0),
        angular: None,
    }
}

#[test]
fn mad_rejection_drops_a_planted_outlier() {
    // Deterministic noise of ±0.004 around (0.4, 0.6) plus one blink spike at (0.95, 0.1).
    let offs = [-0.004, -0.002, 0.0, 0.002, 0.004];
    let mut s: Vec<_> = (0..25)
        .map(|i| cs([0.4 + offs[i % 5], 0.6 + offs[(i / 5) % 5]]))
        .collect();
    s.push(cs([0.95, 0.1]));
    let a = robust_aggregate(&s, 3.0).unwrap();
    assert_eq!(a.kept, 25);
    assert_pt(a.raw_median_norm, [0.4, 0.6], 1e-12, "median");
    assert_eq!(a.eye_pos_mm_median, EYE_CENTRE);
}

#[test]
fn mad_rejection_keeps_inliers_and_reports_angular_medians() {
    let mut s: Vec<_> = [0.1, 0.2, 0.3, 0.4, 0.5]
        .iter()
        .map(|&x| {
            let mut c = cs([x, 0.5]);
            c.angular = Some(AngularPair {
                measured_deg: [x * 10.0, 1.0],
                target_deg: [x * 20.0, 2.0],
            });
            c
        })
        .collect();
    // MAD = 0.1 in x; 3 MAD = 0.3: 0.1 .. 0.5 all within 0.3 of 0.3 -> all kept.
    let a = robust_aggregate(&s, 3.0).unwrap();
    assert_eq!(a.kept, 5);
    assert_pt(a.raw_median_norm, [0.3, 0.5], 1e-12, "median");
    let ang = a.angular.unwrap();
    assert_pt(ang.measured_deg, [3.0, 1.0], 1e-12, "meas");
    assert_pt(ang.target_deg, [6.0, 2.0], 1e-12, "target");
    // One sample without angular data: the target has no angular pair.
    s[0].angular = None;
    assert!(robust_aggregate(&s, 3.0).unwrap().angular.is_none());
    assert!(robust_aggregate(&[], 3.0).is_none());
}

// ------------------------------------------------------------------------------------------
// CalibrationSession (CAL-2, CAL-3, CAL-8)
// ------------------------------------------------------------------------------------------

const DT: f64 = 1.0 / 60.0;

fn session(layout: Vec<[f64; 2]>) -> CalibrationSession {
    CalibrationSession::new(layout, CalibrationParams::default(), gate(), portrait())
}

/// Pushes frames from `t0` at 60 Hz until the target closes; returns (result, frames used).
fn run_until_closed(
    s: &mut CalibrationSession,
    t0: f64,
    mut frame: impl FnMut(f64, usize) -> ProcessedSample,
) -> (asd_core::types::tracking::TargetResult, usize) {
    for i in 1..2000 {
        let t = t0 + i as f64 * DT;
        let p = frame(t, i);
        if let Some(r) = s.push(&p, None) {
            return (r, i);
        }
    }
    panic!("target never closed");
}

#[test]
fn session_hit_after_settle_plus_collect_with_outlier_rejected() {
    let mut s = session(vec![[0.3, 0.4]]);
    assert_eq!(
        s.status(),
        TargetStatus::Pending {
            index: 0,
            target_norm: [0.3, 0.4],
            attempt: 1
        }
    );
    s.begin_target(10.0).unwrap();
    let (r, n) = run_until_closed(&mut s, 10.0, |t, i| {
        let raw = if i == 60 { [0.9, 0.9] } else { [0.35, 0.42] };
        processed(t, TrackState::Tracked, Some(raw), 0.9)
    });
    // 1600 ms of tracked time at 60 Hz = 96 frames; 400 ms settle = 24 frames dropped.
    assert_eq!(n, 96);
    assert_eq!(r.kind, TargetKind::Calibration);
    assert_eq!(r.outcome, TargetOutcome::Hit);
    assert_eq!(r.attempt, 1);
    assert_eq!(r.total_samples, 96);
    assert_eq!(r.valid_samples, 96 - 24 - 1);
    assert_pt(r.raw_median_norm.unwrap(), [0.35, 0.42], 1e-12, "median");
    assert_close(r.data_loss, 0.0, 1e-12, "loss");
    assert_close(r.started_at_sec, 10.0, 0.0, "start");
    assert_close(r.ended_at_sec, 10.0 + 96.0 * DT, 1e-9, "end");
    assert_eq!(r.eye_pos_mm_median, Some(EYE_CENTRE));
    assert_eq!(s.status(), TargetStatus::Complete);
}

#[test]
fn session_window_pauses_while_tracking_is_lost() {
    let mut s = session(vec![[0.5, 0.5]]);
    s.begin_target(0.0).unwrap();
    // Frames 31..=90 (1 s) lost; the window resumes and needs 60 more tracked intervals
    // plus the one interval after the loss that is not counted.
    let (r, n) = run_until_closed(&mut s, 0.0, |t, i| {
        if (31..=90).contains(&i) {
            processed(t, TrackState::Lost, None, 0.0)
        } else {
            processed(t, TrackState::Tracked, Some([0.5, 0.5]), 0.9)
        }
    });
    // Active intervals: 1..=30 (30) and 92..=n (n - 91); need 96.
    assert_eq!(n, 91 + 66);
    assert_eq!(r.outcome, TargetOutcome::Hit);
    // Not tracked: the 60 lost frames' intervals plus the resume interval (61 of n).
    assert_close(r.data_loss, 61.0 / n as f64, 1e-9, "loss");
}

#[test]
fn session_gate_rejects_low_confidence_and_fast_head() {
    let mut s = session(vec![[0.5, 0.5]]);
    s.begin_target(0.0).unwrap();
    let (r, _) = run_until_closed(&mut s, 0.0, |t, i| {
        let mut p = processed(t, TrackState::Tracked, Some([0.5, 0.5]), 0.9);
        if i % 2 == 0 {
            p.confidence = 0.49;
            p.raw_point_norm = Some([0.0, 0.0]);
        }
        if i % 3 == 0 {
            p.head_speed_deg_per_sec = Some(121.0);
            p.raw_point_norm = Some([1.0, 1.0]);
        }
        p
    });
    // Frames 25..=96 after the settle: 72; odd and not multiple of 3 -> 24 kept.
    assert_eq!(r.valid_samples, 24);
    assert_pt(r.raw_median_norm.unwrap(), [0.5, 0.5], 0.0, "only gated-in");
}

#[test]
fn session_starved_target_needs_retry_then_missing() {
    let mut s = session(vec![[0.12, 0.12], [0.88, 0.12]]);
    s.begin_target(0.0).unwrap();
    let lost = |t: f64, _i: usize| processed(t, TrackState::Lost, None, 0.0);
    let (r1, n1) = run_until_closed(&mut s, 0.0, lost);
    assert_eq!(r1.outcome, TargetOutcome::NeedsRetry);
    assert_eq!(r1.attempt, 1);
    assert_eq!(r1.valid_samples, 0);
    assert!(r1.raw_median_norm.is_none());
    // Timeout at maxTargetWaitMs = 4000 ms of frame time: frame 240.
    assert_eq!(n1, 240);
    assert_close(r1.data_loss, 1.0, 1e-12, "loss");
    assert_eq!(
        s.status(),
        TargetStatus::Pending {
            index: 0,
            target_norm: [0.12, 0.12],
            attempt: 2
        }
    );
    // A push without a running target is ignored.
    assert!(s.push(&lost(5.0, 0), None).is_none());
    s.begin_target(5.0).unwrap();
    // Enough frames but too few gated-in samples (19 < 20).
    let (r2, _) = run_until_closed(&mut s, 5.0, |t, i| {
        let ok = i > 24 && i <= 24 + 19;
        processed(
            t,
            TrackState::Tracked,
            Some([0.1, 0.1]),
            if ok { 0.9 } else { 0.1 },
        )
    });
    assert_eq!(r2.outcome, TargetOutcome::Missed);
    assert_eq!(r2.attempt, 2);
    assert_eq!(r2.valid_samples, 19);
    assert_eq!(
        s.status(),
        TargetStatus::Pending {
            index: 1,
            target_norm: [0.88, 0.12],
            attempt: 1
        }
    );
}

#[test]
fn session_begin_twice_or_after_completion_is_an_error() {
    let mut s = session(vec![[0.5, 0.5]]);
    s.begin_target(0.0).unwrap();
    assert!(s.begin_target(0.1).is_err());
    let _ = run_until_closed(&mut s, 0.0, |t, _| {
        processed(t, TrackState::Tracked, Some([0.5, 0.5]), 0.9)
    });
    assert!(s.begin_target(3.0).is_err());
}

#[test]
fn session_abort_target_closes_with_current_samples() {
    let mut s = session(vec![[0.5, 0.5]]);
    s.begin_target(0.0).unwrap();
    for i in 1..=30 {
        let t = i as f64 * DT;
        assert!(s
            .push(
                &processed(t, TrackState::Tracked, Some([0.5, 0.5]), 0.9),
                None
            )
            .is_none());
    }
    let r = s.end_target(0.5).unwrap();
    // Only 6 samples after the settle: NeedsRetry.
    assert_eq!(r.outcome, TargetOutcome::NeedsRetry);
    assert_eq!(r.valid_samples, 6);
    assert!(s.end_target(0.6).is_none());
}

#[test]
fn session_finish_fits_and_records_quality() {
    let layout = layout5();
    let mut s = session(layout.clone());
    let mut t0 = 0.0;
    for (k, tgt) in layout.iter().enumerate() {
        let raw = [tgt[0] * 0.9 + 0.07, tgt[1] * 1.1 - 0.03];
        s.begin_target(t0).unwrap();
        let (r, n) = run_until_closed(&mut s, t0, |t, _| {
            processed(t, TrackState::Tracked, Some(raw), 0.9)
        });
        assert_eq!(r.outcome, TargetOutcome::Hit, "target {k}");
        t0 += n as f64 * DT + 0.5;
    }
    assert_eq!(s.status(), TargetStatus::Complete);
    let out = s.finish();
    let rec = &out.record;
    assert_eq!(rec.model, Some(CalModelKind::ScreenAffine));
    assert_eq!(rec.layout_points, 5);
    assert_eq!(rec.usable_points, 5);
    assert_eq!(rec.missing_points, 0);
    assert_eq!(rec.quality, CalibrationQuality::Good);
    assert!(rec.residual_deg.unwrap() < 1e-9);
    assert!(rec.loo_cv_deg.unwrap() < 1e-9);
    assert!(rec.condition_number.unwrap() >= 1.0);
    assert_eq!(rec.attempts, 1);
    let pose = rec.pose.unwrap();
    assert_close(pose.distance_cm, 40.0, 1e-12, "dist");
    assert_close(pose.yaw_deg, 0.1f64.to_degrees(), 1e-9, "yaw");
    assert_close(pose.pitch_deg, -0.05f64.to_degrees(), 1e-9, "pitch");
    assert!(out.fit.is_ok());
    // Hit targets carry the calibrated median and their residual.
    for r in &out.targets {
        assert_pt(
            r.calibrated_median_norm.unwrap(),
            [r.x_norm, r.y_norm],
            1e-9,
            "cal",
        );
        assert!(r.accuracy_deg.unwrap() < 1e-9);
    }
}

#[test]
fn session_with_a_missing_target_fits_the_rest() {
    let layout = layout5();
    let mut s = session(layout.clone());
    let mut t0 = 0.0;
    let mut k = 0;
    while let TargetStatus::Pending { index, .. } = s.status() {
        let tgt = layout[index];
        s.begin_target(t0).unwrap();
        let (_, n) = run_until_closed(&mut s, t0, |t, _| {
            if index == 2 {
                processed(t, TrackState::Lost, None, 0.0)
            } else {
                processed(
                    t,
                    TrackState::Tracked,
                    Some([tgt[0] + 0.05, tgt[1] - 0.04]),
                    0.9,
                )
            }
        });
        t0 += n as f64 * DT;
        k += 1;
    }
    assert_eq!(k, 6, "target 2 shown twice");
    let out = s.finish();
    assert_eq!(out.record.usable_points, 4);
    assert_eq!(out.record.missing_points, 1);
    assert_eq!(out.record.model, Some(CalModelKind::ScreenAffine));
    assert_eq!(out.record.quality, CalibrationQuality::Good);
    // The missing target was never replaced by its own position (G-F11).
    let missed: Vec<_> = out
        .targets
        .iter()
        .filter(|r| r.outcome == TargetOutcome::Missed)
        .collect();
    assert_eq!(missed.len(), 1);
    assert!(missed[0].raw_median_norm.is_none());
    assert_eq!(out.targets.len(), 6);
}

#[test]
fn session_with_too_few_points_fails() {
    let layout = vec![[0.5, 0.5], [0.12, 0.12], [0.88, 0.12]];
    let mut s = session(layout.clone());
    let mut t0 = 0.0;
    while let TargetStatus::Pending { index, .. } = s.status() {
        let tgt = layout[index];
        s.begin_target(t0).unwrap();
        let (_, n) = run_until_closed(&mut s, t0, |t, _| {
            processed(t, TrackState::Tracked, Some(tgt), 0.9)
        });
        t0 += n as f64 * DT;
    }
    let out = s.finish();
    assert_eq!(out.record.usable_points, 3);
    assert_eq!(out.record.quality, CalibrationQuality::Failed);
}

#[test]
fn session_with_no_points_fails_without_model() {
    let s = session(vec![[0.5, 0.5]]);
    let out = s.finish();
    assert_eq!(out.record.model, None);
    assert_eq!(out.record.quality, CalibrationQuality::Failed);
    assert_eq!(out.record.usable_points, 0);
    assert_eq!(out.record.missing_points, 1);
    assert!(out.record.residual_deg.is_none());
    assert!(out.record.pose.is_none());
}

#[test]
fn session_collects_angular_pairs_from_raw_frames() {
    let geo = portrait();
    let face = face_at_centre(0.0, 0.0);
    let params = CalibrationParams {
        default_model: CalModelKind::AngularAffine,
        ..CalibrationParams::default()
    };
    let layout = layout5();
    let mut s = CalibrationSession::new(layout.clone(), params, gate(), geo);
    let mut t0 = 0.0;
    while let TargetStatus::Pending { index, .. } = s.status() {
        let tgt = layout[index];
        s.begin_target(t0).unwrap();
        let mut closed = None;
        for i in 1..200 {
            let t = t0 + i as f64 * DT;
            let raw = synth_frame(&face, tgt, ERR, t);
            let hit = raycast_parts(
                raw.face_transform.as_ref().unwrap(),
                raw.look_at.as_ref().unwrap(),
                raw.left_eye_transform.as_ref(),
                raw.right_eye_transform.as_ref(),
                &geo,
            );
            let mut p = processed(t, TrackState::Tracked, hit.point_norm, 0.9);
            p.eye_pos_mm = hit.eye_pos_mm;
            if let Some(r) = s.push(&p, Some(&raw)) {
                closed = Some((r, i));
                break;
            }
        }
        let (r, n) = closed.unwrap();
        assert_eq!(r.outcome, TargetOutcome::Hit);
        t0 += n as f64 * DT;
    }
    let out = s.finish();
    assert_eq!(out.record.model, Some(CalModelKind::AngularAffine));
    assert!(out.record.loo_cv_deg.unwrap() < 1e-9);
    assert_eq!(out.record.quality, CalibrationQuality::Good);
}

// ------------------------------------------------------------------------------------------
// Golden vectors (hand-known expectations)
// ------------------------------------------------------------------------------------------

#[test]
fn golden_calibration_vectors() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden/gaze/calibration");
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    files.sort();
    assert!(!files.is_empty());
    let geo = portrait();
    for f in files {
        let doc: Value = serde_json::from_str(&std::fs::read_to_string(&f).unwrap()).unwrap();
        assert_eq!(doc["schema"], "asd-golden/1");
        assert_eq!(doc["function"], "calibrate");
        for v in doc["vectors"].as_array().unwrap() {
            let name = v["name"].as_str().unwrap();
            let points: Vec<CalPoint> = serde_json::from_value(v["input"]["points"].clone())
                .unwrap_or_else(|e| panic!("{name}: points: {e}"));
            let params: CalibrationParams =
                serde_json::from_value(v["input"]["params"].clone()).unwrap();
            let exp = &v["expected"];
            let fit = calibrate(&points, &params, &geo).unwrap();
            let kind: CalModelKind = serde_json::from_value(exp["model"].clone()).unwrap();
            assert_eq!(fit.selected.model.kind(), kind, "{name}");
            let q: CalibrationQuality = serde_json::from_value(exp["quality"].clone()).unwrap();
            assert_eq!(fit.quality, q, "{name}");
            let tol = exp["tolerance"].as_f64().unwrap();
            assert_close(
                fit.selected.residual_deg,
                exp["residualDeg"].as_f64().unwrap(),
                tol,
                name,
            );
            assert_close(
                fit.selected.loo_cv_deg.unwrap(),
                exp["looCvDeg"].as_f64().unwrap(),
                tol,
                name,
            );
            for probe in exp["probes"].as_array().unwrap() {
                let raw: [f64; 2] = serde_json::from_value(probe["raw"].clone()).unwrap();
                let want: [f64; 2] = serde_json::from_value(probe["calibrated"].clone()).unwrap();
                let got = fit.selected.model.apply_point(raw).unwrap();
                assert_pt(got, want, tol, name);
            }
        }
    }
}
