//! owner: WP-G.1
//!
//! GEO-5 / GEO-6: analytic raycast cases. Each case places the eye at a known camera-space
//! position and builds the look-at point on the straight line from the eye to a chosen point
//! of the screen plane, so the expected hit is known by construction (no Swift oracle: the
//! Swift raycaster clamps before calibration (G-F1) and accepts an off-plane hit for t > 2.5
//! (G-F3); those behaviours are intentionally not reproduced).
//!
//! Camera frame (`gaze::raycast::CAMERA_AXES`): metres, device-fixed, portrait-native,
//! +x towards the right edge, +y towards the camera (top) edge, the viewer at z < 0,
//! screen plane z = 0. Native screen frame: mm from the active area's top-left, y down,
//! so a camera-space point (X, Y, 0) is native mm (cx + 1000 X, cy - 1000 Y).

use asd_core::gaze::geometry::{idiom_default, lookup};
use asd_core::gaze::raycast::{
    raycast, raycast_parts, EyeOrigin, GRAZING_MIN_ANGLE_DEG, RAY_T_MAX, RAY_T_MIN,
};
use asd_core::types::tracking::{
    DeviceGeometry, DeviceIdiom, Orientation, RawTrackingSample, RayStatus,
};

const EPS: f64 = 1e-9;

type M4 = [f64; 16];

/// Column-major 4x4 (simd / ARKit layout): translation at indices 12, 13, 14.
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

/// Rotation of +90 degrees about z (face x -> camera +y, face y -> camera -x) plus translation.
fn roll90(t: [f64; 3]) -> M4 {
    let mut m = translation(t);
    m[0] = 0.0; // col0 = (0, 1, 0)
    m[1] = 1.0;
    m[4] = -1.0; // col1 = (-1, 0, 0)
    m[5] = 0.0;
    m
}

fn test_geo() -> DeviceGeometry {
    let mut g = idiom_default(DeviceIdiom::Phone);
    g.screen_width_mm = 100.0;
    g.screen_height_mm = 200.0;
    g.camera_offset_mm.x = 50.0;
    g.camera_offset_mm.y = -8.0;
    g
}

/// Camera-space point (metres) of a native-frame screen point (mm).
fn native_to_cam(g: &DeviceGeometry, p: [f64; 2]) -> [f64; 3] {
    [
        (p[0] - g.camera_offset_mm.x) / 1000.0,
        -(p[1] - g.camera_offset_mm.y) / 1000.0,
        0.0,
    ]
}

/// Look-at in the face frame of an identity-rotation face at `face_t`, on the ray from `eye`
/// to `target` at fraction `f` (so the hit parameter is t = 1 / f).
fn look_at_identity(face_t: [f64; 3], eye: [f64; 3], target: [f64; 3], f: f64) -> [f64; 3] {
    let p = [
        eye[0] + f * (target[0] - eye[0]),
        eye[1] + f * (target[1] - eye[1]),
        eye[2] + f * (target[2] - eye[2]),
    ];
    [p[0] - face_t[0], p[1] - face_t[1], p[2] - face_t[2]]
}

/// Eye transforms symmetric about the face origin: midpoint = face origin.
fn eyes() -> (M4, M4) {
    (
        translation([0.031, 0.0, 0.0]),
        translation([-0.031, 0.0, 0.0]),
    )
}

fn assert_point(hit: Option<[f64; 2]>, expected: [f64; 2], eps: f64) {
    let p = hit.expect("ok ray must produce a point");
    assert!(
        (p[0] - expected[0]).abs() <= eps && (p[1] - expected[1]).abs() <= eps,
        "{p:?} != {expected:?}"
    );
}

#[test]
fn gaze_raycast_constants_are_the_documented_range() {
    assert_eq!(RAY_T_MIN, 0.1);
    assert_eq!(RAY_T_MAX, 2.5);
    assert_eq!(GRAZING_MIN_ANGLE_DEG, 5.0);
}

#[test]
fn gaze_raycast_corners_and_centre_portrait_two_geometries() {
    let iphone16 = lookup("iPhone17,3").geometry;
    for g in [test_geo(), iphone16] {
        let o = g.rotate(Orientation::Portrait);
        let (w, h) = (g.screen_width_mm, g.screen_height_mm);
        // Eye 10 cm below the camera, 50 cm in front, slightly right.
        let face_t = [0.012, -0.1, -0.5];
        let (le, re) = eyes();
        for (target_mm, expected) in [
            ([w / 2.0, h / 2.0], [0.5, 0.5]),
            ([0.0, 0.0], [0.0, 0.0]),
            ([w, 0.0], [1.0, 0.0]),
            ([0.0, h], [0.0, 1.0]),
            ([w, h], [1.0, 1.0]),
        ] {
            let target = native_to_cam(&g, target_mm);
            for f in [1.0, 0.5, 0.8] {
                let la = look_at_identity(face_t, face_t, target, f);
                let hit = raycast_parts(&translation(face_t), &la, Some(&le), Some(&re), &o);
                assert_eq!(hit.status, RayStatus::Ok, "{target_mm:?} f={f}");
                assert_point(hit.point_norm, expected, 1e-9);
                assert!((hit.t.unwrap() - 1.0 / f).abs() < 1e-9);
            }
        }
    }
}

#[test]
fn gaze_raycast_eye_position_in_screen_frame() {
    // Eye (camera space) (0.012, -0.1, -0.5) on the 100 x 200 panel, camera (50, -8):
    // native (50 + 12, -8 + 100) = (62, 92) mm, 500 mm in front.
    let g = test_geo();
    let face_t = [0.012, -0.1, -0.5];
    let (le, re) = eyes();
    let la = look_at_identity(face_t, face_t, native_to_cam(&g, [50.0, 100.0]), 1.0);
    let hit = raycast_parts(
        &translation(face_t),
        &la,
        Some(&le),
        Some(&re),
        &g.rotate(Orientation::Portrait),
    );
    let e = hit.eye_pos_mm.unwrap();
    assert!((e[0] - 62.0).abs() < EPS && (e[1] - 92.0).abs() < EPS && (e[2] - 500.0).abs() < EPS);
    assert_eq!(hit.eye_origin, EyeOrigin::EyeMidpoint);
    assert!((hit.distance_cm().unwrap() - 50.0).abs() < EPS);

    // Same eye in landscapeLeft (x' = H - y, y' = x): (200 - 92, 62) = (108, 62).
    let hit = raycast_parts(
        &translation(face_t),
        &la,
        Some(&le),
        Some(&re),
        &g.rotate(Orientation::LandscapeLeft),
    );
    let e = hit.eye_pos_mm.unwrap();
    assert!((e[0] - 108.0).abs() < EPS && (e[1] - 62.0).abs() < EPS && (e[2] - 500.0).abs() < EPS);
}

#[test]
fn gaze_raycast_every_orientation_maps_native_hits() {
    // Native targets on the 100 x 200 panel and their hand-computed oriented normalised hits:
    //   portrait            (x/100, y/200)
    //   portraitUpsideDown  ((100-x)/100, (200-y)/200)
    //   landscapeLeft       ((200-y)/200, x/100)       (oriented panel 200 x 100)
    //   landscapeRight      (y/200, (100-x)/100)
    let g = test_geo();
    let face_t = [0.0, -0.1, -0.45];
    let (le, re) = eyes();
    let cases: [(Orientation, [f64; 2], [f64; 2]); 8] = [
        (Orientation::Portrait, [20.0, 150.0], [0.2, 0.75]),
        (Orientation::Portrait, [0.0, 0.0], [0.0, 0.0]),
        (Orientation::PortraitUpsideDown, [20.0, 150.0], [0.8, 0.25]),
        (Orientation::PortraitUpsideDown, [0.0, 0.0], [1.0, 1.0]),
        (Orientation::LandscapeLeft, [20.0, 150.0], [0.25, 0.2]),
        (Orientation::LandscapeLeft, [0.0, 0.0], [1.0, 0.0]),
        (Orientation::LandscapeRight, [20.0, 150.0], [0.75, 0.8]),
        (Orientation::LandscapeRight, [0.0, 0.0], [0.0, 1.0]),
    ];
    for (o, native, expected) in cases {
        let la = look_at_identity(face_t, face_t, native_to_cam(&g, native), 0.7);
        let hit = raycast_parts(
            &translation(face_t),
            &la,
            Some(&le),
            Some(&re),
            &g.rotate(o),
        );
        assert_eq!(hit.status, RayStatus::Ok);
        assert_point(hit.point_norm, expected, 1e-9);
    }
}

#[test]
fn gaze_raycast_off_screen_hit_is_not_clamped() {
    // 2 cm left of the glass on a 100 mm panel: x = -20 / 100 = -0.2 (acceptance 2).
    let g = test_geo();
    let face_t = [0.0, -0.1, -0.5];
    let (le, re) = eyes();
    let la = look_at_identity(face_t, face_t, native_to_cam(&g, [-20.0, 230.0]), 0.6);
    let hit = raycast_parts(
        &translation(face_t),
        &la,
        Some(&le),
        Some(&re),
        &g.rotate(Orientation::Portrait),
    );
    assert_eq!(hit.status, RayStatus::Ok);
    assert_point(hit.point_norm, [-0.2, 1.15], 1e-9);
}

#[test]
fn gaze_raycast_t_out_of_range_produces_no_point() {
    let g = test_geo();
    let o = g.rotate(Orientation::Portrait);
    let face_t = [0.0, -0.1, -0.5];
    let (le, re) = eyes();
    let target = native_to_cam(&g, [50.0, 100.0]);

    // f = 0.3 -> t = 3.33 > 2.5 (G-F3: Swift used the 3D look-at point here).
    let la = look_at_identity(face_t, face_t, target, 0.3);
    let hit = raycast_parts(&translation(face_t), &la, Some(&le), Some(&re), &o);
    assert_eq!(hit.status, RayStatus::OutOfRange);
    assert_eq!(hit.point_norm, None);
    assert!(hit.eye_pos_mm.is_some(), "eye position is still known");

    // f = 20 -> t = 0.05 < 0.1.
    let la = look_at_identity(face_t, face_t, target, 20.0);
    let hit = raycast_parts(&translation(face_t), &la, Some(&le), Some(&re), &o);
    assert_eq!(hit.status, RayStatus::OutOfRange);
    assert_eq!(hit.point_norm, None);

    // f = 1 / 2.4 -> t = 2.4, inside the range.
    let la = look_at_identity(face_t, face_t, target, 1.0 / 2.4);
    let hit = raycast_parts(&translation(face_t), &la, Some(&le), Some(&re), &o);
    assert_eq!(hit.status, RayStatus::Ok);
    assert_point(hit.point_norm, [0.5, 0.5], 1e-9);
}

#[test]
fn gaze_raycast_backward_ray() {
    // Look-at 10 cm further from the screen than the eye: the ray points away from it.
    let o = test_geo().rotate(Orientation::Portrait);
    let face_t = [0.0, -0.1, -0.5];
    let (le, re) = eyes();
    let hit = raycast_parts(
        &translation(face_t),
        &[0.0, 0.0, -0.1],
        Some(&le),
        Some(&re),
        &o,
    );
    assert_eq!(hit.status, RayStatus::Backward);
    assert_eq!(hit.point_norm, None);
    assert_eq!(hit.t, None);
}

#[test]
fn gaze_raycast_grazing_ray() {
    let o = test_geo().rotate(Orientation::Portrait);
    let face_t = [0.0, -0.1, -0.5];
    let (le, re) = eyes();
    // atan(0.001 / 0.1) = 0.57 deg towards the screen: below the 5 degree limit.
    let hit = raycast_parts(
        &translation(face_t),
        &[0.1, 0.0, 0.001],
        Some(&le),
        Some(&re),
        &o,
    );
    assert_eq!(hit.status, RayStatus::Grazing);
    assert_eq!(hit.point_norm, None);
    // Parallel to the screen and zero-length direction are grazing too.
    for la in [[0.0, 0.1, 0.0], [0.0, 0.0, 0.0]] {
        let hit = raycast_parts(&translation(face_t), &la, Some(&le), Some(&re), &o);
        assert_eq!(hit.status, RayStatus::Grazing, "{la:?}");
    }
    // atan(0.1 / 0.1) = 45 deg is fine (hit at t = 5 is out of range, not grazing).
    let hit = raycast_parts(
        &translation(face_t),
        &[0.1, 0.0, 0.1],
        Some(&le),
        Some(&re),
        &o,
    );
    assert_eq!(hit.status, RayStatus::OutOfRange);
}

#[test]
fn gaze_raycast_eye_midpoint_uses_both_eye_transforms() {
    // Eyes at face-local (0.032, 0, 0.02) and (-0.028, 0.004, 0.02): midpoint (0.002, 0.002, 0.02).
    // Face at (0, -0.1, -0.5), identity rotation -> eye (0.002, -0.098, -0.48) m
    // -> native (50 + 2, -8 + 98) = (52, 90) mm, 480 mm in front.
    let g = test_geo();
    let face_t = [0.0, -0.1, -0.5];
    let le = translation([0.032, 0.0, 0.02]);
    let re = translation([-0.028, 0.004, 0.02]);
    let eye = [0.002, -0.098, -0.48];
    let la = look_at_identity(face_t, eye, native_to_cam(&g, [10.0, 40.0]), 0.9);
    let hit = raycast_parts(
        &translation(face_t),
        &la,
        Some(&le),
        Some(&re),
        &g.rotate(Orientation::Portrait),
    );
    let e = hit.eye_pos_mm.unwrap();
    assert!((e[0] - 52.0).abs() < EPS && (e[1] - 90.0).abs() < EPS && (e[2] - 480.0).abs() < EPS);
    assert_point(hit.point_norm, [0.1, 0.2], 1e-9);
}

#[test]
fn gaze_raycast_missing_eye_transforms_use_face_centre_plus_3cm_up() {
    // Identity face at (0, -0.1, -0.5): eye origin = (0, -0.07, -0.5) -> native (50, 62) mm.
    let g = test_geo();
    let o = g.rotate(Orientation::Portrait);
    let face_t = [0.0, -0.1, -0.5];
    let eye = [0.0, -0.07, -0.5];
    let la = look_at_identity(face_t, eye, native_to_cam(&g, [80.0, 20.0]), 1.0);
    let zero = translation([0.0, 0.0, 0.0]);
    for (l, r) in [
        (None, None),
        (Some(&zero), Some(&zero)),
        (Some(&zero), None),
    ] {
        let hit = raycast_parts(&translation(face_t), &la, l, r, &o);
        assert_eq!(hit.eye_origin, EyeOrigin::FaceCentreFallback);
        let e = hit.eye_pos_mm.unwrap();
        assert!(
            (e[0] - 50.0).abs() < EPS && (e[1] - 62.0).abs() < EPS && (e[2] - 500.0).abs() < EPS
        );
        assert_point(hit.point_norm, [0.8, 0.1], 1e-9);
    }
}

#[test]
fn gaze_raycast_fallback_up_is_face_up_not_camera_up() {
    // Face rolled +90 degrees about z: face +y is camera -x, so "3 cm up" moves the eye origin
    // to camera (tx - 0.03, ty, tz). Face at (0.01, -0.1, -0.5) -> eye (-0.02, -0.1, -0.5)
    // -> native (50 - 20, -8 + 100) = (30, 92) mm.
    let g = test_geo();
    let face_t = [0.01, -0.1, -0.5];
    let face = roll90(face_t);
    let eye = [-0.02, -0.1, -0.5];
    let target = native_to_cam(&g, [70.0, 180.0]);
    // Look-at in the face frame: R^T (p - T) with R^T v = (v.y, -v.x, v.z).
    let p = [
        eye[0] + 0.75 * (target[0] - eye[0]),
        eye[1] + 0.75 * (target[1] - eye[1]),
        eye[2] + 0.75 * (target[2] - eye[2]),
    ];
    let v = [p[0] - face_t[0], p[1] - face_t[1], p[2] - face_t[2]];
    let la = [v[1], -v[0], v[2]];
    let hit = raycast_parts(&face, &la, None, None, &g.rotate(Orientation::Portrait));
    let e = hit.eye_pos_mm.unwrap();
    assert!((e[0] - 30.0).abs() < EPS && (e[1] - 92.0).abs() < EPS && (e[2] - 500.0).abs() < EPS);
    assert_eq!(hit.status, RayStatus::Ok);
    assert_point(hit.point_norm, [0.7, 0.9], 1e-9);
}

#[test]
fn gaze_raycast_rotated_face_transforms_eye_transforms() {
    // Rolled face at (0, -0.1, -0.5); eyes at face-local (0.01, 0.03, 0) and (0.01, -0.03, 0)
    // -> midpoint face-local (0.01, 0, 0) -> camera (0, 0.01, 0) offset
    // -> eye (0, -0.09, -0.5) -> native (50, 82) mm.
    let g = test_geo();
    let face_t = [0.0, -0.1, -0.5];
    let le = translation([0.01, 0.03, 0.0]);
    let re = translation([0.01, -0.03, 0.0]);
    let target = native_to_cam(&g, [25.0, 50.0]);
    let v = [
        target[0] - face_t[0],
        target[1] - face_t[1],
        target[2] - face_t[2],
    ];
    let la = [v[1], -v[0], v[2]];
    let hit = raycast_parts(
        &roll90(face_t),
        &la,
        Some(&le),
        Some(&re),
        &g.rotate(Orientation::Portrait),
    );
    let e = hit.eye_pos_mm.unwrap();
    assert!((e[0] - 50.0).abs() < EPS && (e[1] - 82.0).abs() < EPS && (e[2] - 500.0).abs() < EPS);
    assert_eq!(hit.eye_origin, EyeOrigin::EyeMidpoint);
    assert_point(hit.point_norm, [0.25, 0.25], 1e-9);
}

#[test]
fn gaze_raycast_non_finite_input_never_panics_and_gives_no_point() {
    let o = test_geo().rotate(Orientation::Portrait);
    let mut face = translation([0.0, -0.1, -0.5]);
    face[12] = f64::NAN;
    let hit = raycast_parts(&face, &[0.0, 0.0, 0.3], None, None, &o);
    assert_eq!(hit.status, RayStatus::OutOfRange);
    assert_eq!(hit.point_norm, None);
    assert_eq!(hit.eye_pos_mm, None);
}

fn sample(face: Option<M4>, look_at: Option<[f64; 3]>) -> RawTrackingSample {
    RawTrackingSample {
        t_sec: 1.0,
        face_tracked: face.is_some(),
        face_transform: face,
        look_at,
        left_eye_transform: None,
        right_eye_transform: None,
        eye_blink_left: Some(0.1),
        eye_blink_right: Some(0.1),
        ambient_intensity: None,
        ambient_color_temp: None,
        light_direction: None,
        orientation: Orientation::Portrait,
        clip_media_time_sec: None,
        stimulus_id: None,
    }
}

#[test]
fn gaze_raycast_sample_without_face_has_no_hit() {
    let o = test_geo().rotate(Orientation::Portrait);
    assert!(raycast(&sample(None, None), &o).is_none());
    assert!(raycast(&sample(Some(translation([0.0, -0.1, -0.5])), None), &o).is_none());
    let hit = raycast(
        &sample(Some(translation([0.0, -0.1, -0.5])), Some([0.0, 0.0, 0.3])),
        &o,
    )
    .expect("face present");
    assert_eq!(hit.status, RayStatus::Ok);
}
