//! owner: WP-G.1
//!
//! Gaze raycast onto the screen plane (GEO-5).
//!
//! The ray starts at the eye origin (midpoint of the two eye transforms; the face-anchor
//! origin plus 3 cm along the face's own up axis when the eye transforms are absent), passes
//! through the look-at point, and is intersected with the screen plane. The hit is returned
//! in screen-normalised space **without clamping**, together with the eye position in the
//! oriented screen frame and a [`RayStatus`]; only `ok` yields a point.
//!
//! Intentional differences from the Swift `ScreenRaycaster.projectGazeToScreen` (dev@1f43026),
//! which is kept only as a reference until it is deleted (ADR-0007 D1):
//! - no clamping to [0,1] (G-F1): clamping is a drawing concern, calibration needs the
//!   unclamped point and validity is decided after calibration (TRK-2);
//! - a hit parameter outside [`RAY_T_MIN`, `RAY_T_MAX`] is `outOfRange` with **no** point;
//!   Swift substituted the 3D look-at point and could mark it on-screen (G-F3);
//! - a backward or grazing ray has no point (Swift also substituted the look-at point);
//! - screen size and camera offset come from the per-model table and the interface
//!   orientation (G-F4, G-F5) instead of two idiom constants in portrait;
//! - the fallback eye origin moves 3 cm along the face's up axis, not the camera's +y, so it
//!   stays correct when the head rolls or the device is in landscape;
//! - the grazing test is an angle ([`GRAZING_MIN_ANGLE_DEG`]) rather than `rayDir.z > 0.05 m`,
//!   which depended on the length of the look-at vector;
//! - eye transforms are "absent" when either is missing or the left one has a zero
//!   translation (the Swift test), and the distance is not floored at 10 cm.

use super::geometry::OrientedGeometry;
use crate::types::tracking::{RawTrackingSample, RayStatus};

/// Lowest accepted hit parameter `t` (hit = eye + t (lookAt - eye)); parity with Swift.
pub const RAY_T_MIN: f64 = 0.1;
/// Highest accepted hit parameter (acceptance scenario 2: `t > 2.5` is `outOfRange`).
pub const RAY_T_MAX: f64 = 2.5;
/// A ray within this angle of the screen plane is `grazing`. At 5 degrees from 30 cm the hit
/// would be more than 3 m away, so no real on-screen gaze is lost.
pub const GRAZING_MIN_ANGLE_DEG: f64 = 5.0;
/// Offset of the fallback eye origin along the face's up axis, metres.
pub const FALLBACK_EYE_UP_M: f64 = 0.03;

/// Axis convention of the camera frame for one capture source (PLT-3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraAxes {
    /// Sign that maps camera +x onto the native screen's +x (towards the right edge in portrait).
    pub x_to_native_right: f64,
    /// Sign that maps camera +y onto the native screen's "up" (towards the camera edge).
    pub y_to_native_up: f64,
    /// Sign of camera z on the viewer's side of the screen plane.
    pub z_viewer_side: f64,
}

/// ARKit face tracking, as the legacy Swift raycaster interprets it: metres, device-fixed and
/// portrait-native, +x towards the right edge, +y towards the camera (top) edge, the viewer
/// at z < 0, the screen plane at z = 0 (camera coplanar with the glass).
///
/// This is the one place to change if the WP-G.8 bench corners check (trap 3) shows ARKit
/// delivers a mirrored or sensor-landscape frame: flip a sign here, the tests follow.
pub const CAMERA_AXES: CameraAxes = CameraAxes {
    x_to_native_right: 1.0,
    y_to_native_up: 1.0,
    z_viewer_side: -1.0,
};

/// How the eye origin was obtained.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EyeOrigin {
    EyeMidpoint,
    FaceCentreFallback,
}

/// Raycast result.
///
/// Invariant: `status == Ok` implies `point_norm`, `eye_pos_mm` and `t` are all `Some`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RayHit {
    pub status: RayStatus,
    /// Screen-normalised hit in the current orientation, unclamped. `Some` only for `ok`.
    pub point_norm: Option<[f64; 2]>,
    /// Eye origin in the oriented screen frame, mm; z = distance in front of the glass.
    /// `None` only when the input is not finite.
    pub eye_pos_mm: Option<[f64; 3]>,
    pub eye_origin: EyeOrigin,
    /// Hit parameter, when the ray points at the screen plane.
    pub t: Option<f64>,
}

impl RayHit {
    /// Perpendicular eye distance from the screen plane, cm.
    pub fn distance_cm(&self) -> Option<f64> {
        self.eye_pos_mm.map(|e| e[2] / 10.0)
    }
}

/// Raycast of one raw sample. `None` when the sample carries no face transform or look-at
/// point (no face anchor); the track-state machine decides what such a frame is.
pub fn raycast(sample: &RawTrackingSample, geo: &OrientedGeometry) -> Option<RayHit> {
    let face = sample.face_transform.as_ref()?;
    let look_at = sample.look_at.as_ref()?;
    Some(raycast_parts(
        face,
        look_at,
        sample.left_eye_transform.as_ref(),
        sample.right_eye_transform.as_ref(),
        geo,
    ))
}

/// Column-major 4x4 times a point (w = 1).
fn transform_point(m: &[f64; 16], p: [f64; 3]) -> [f64; 3] {
    [
        m[0] * p[0] + m[4] * p[1] + m[8] * p[2] + m[12],
        m[1] * p[0] + m[5] * p[1] + m[9] * p[2] + m[13],
        m[2] * p[0] + m[6] * p[1] + m[10] * p[2] + m[14],
    ]
}

fn translation_of(m: &[f64; 16]) -> [f64; 3] {
    [m[12], m[13], m[14]]
}

fn finite3(p: [f64; 3]) -> bool {
    p.iter().all(|v| v.is_finite())
}

/// Raycast from the individual ARKit values. `face` is the face anchor in the camera frame;
/// `look_at` and the eye transforms are in the face frame.
pub fn raycast_parts(
    face: &[f64; 16],
    look_at: &[f64; 3],
    left_eye: Option<&[f64; 16]>,
    right_eye: Option<&[f64; 16]>,
    geo: &OrientedGeometry,
) -> RayHit {
    let axes = CAMERA_AXES;

    let (eye_local, eye_origin) = match (left_eye, right_eye) {
        (Some(l), Some(r)) if translation_of(l) != [0.0, 0.0, 0.0] => {
            let (a, b) = (translation_of(l), translation_of(r));
            (
                [
                    (a[0] + b[0]) / 2.0,
                    (a[1] + b[1]) / 2.0,
                    (a[2] + b[2]) / 2.0,
                ],
                EyeOrigin::EyeMidpoint,
            )
        }
        _ => ([0.0, FALLBACK_EYE_UP_M, 0.0], EyeOrigin::FaceCentreFallback),
    };
    let origin = transform_point(face, eye_local);
    let target = transform_point(face, *look_at);

    let no_point = |status, eye_pos_mm, t| RayHit {
        status,
        point_norm: None,
        eye_pos_mm,
        eye_origin,
        t,
    };

    if !finite3(origin) || !finite3(target) {
        return no_point(RayStatus::OutOfRange, None, None);
    }

    // Camera metres (on or off the plane) -> oriented screen millimetres.
    let to_oriented_mm = |p: [f64; 3]| {
        let native = [
            geo.camera_native_mm[0] + axes.x_to_native_right * 1000.0 * p[0],
            geo.camera_native_mm[1] - axes.y_to_native_up * 1000.0 * p[1],
        ];
        geo.native_to_oriented_mm(native)
    };
    let eye_xy = to_oriented_mm(origin);
    let eye_pos_mm = Some([
        eye_xy[0],
        eye_xy[1],
        axes.z_viewer_side * 1000.0 * origin[2],
    ]);

    let d = [
        target[0] - origin[0],
        target[1] - origin[1],
        target[2] - origin[2],
    ];
    let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    // Sine of the ray's angle to the screen plane, positive towards the screen.
    let towards = -axes.z_viewer_side * d[2];
    let min_sin = GRAZING_MIN_ANGLE_DEG.to_radians().sin();
    if len == 0.0 || towards.abs() < min_sin * len {
        return no_point(RayStatus::Grazing, eye_pos_mm, None);
    }
    if towards < 0.0 {
        return no_point(RayStatus::Backward, eye_pos_mm, None);
    }

    let t = -origin[2] / d[2];
    if !(RAY_T_MIN..=RAY_T_MAX).contains(&t) {
        return no_point(RayStatus::OutOfRange, eye_pos_mm, Some(t));
    }
    let hit = [origin[0] + t * d[0], origin[1] + t * d[1], 0.0];
    let hit_mm = to_oriented_mm(hit);
    RayHit {
        status: RayStatus::Ok,
        point_norm: Some(geo.mm_to_norm(hit_mm)),
        eye_pos_mm,
        eye_origin,
        t: Some(t),
    }
}
