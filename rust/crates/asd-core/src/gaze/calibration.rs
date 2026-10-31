//! owner: WP-G.2
//!
//! Calibration (technical.md CAL-2 .. CAL-8, ADR-0007 D5/D6):
//! - four models behind one enum ([`CalModel`], CAL-4), fitted by least squares on a
//!   Hartley-normalised design matrix and applied **without clamping** (trap 1);
//! - robust per-target aggregation ([`robust_aggregate`], CAL-3): per-axis median, samples
//!   beyond `k x MAD` rejected, median of the rest;
//! - quality in degrees of visual angle ([`evaluate`], [`band`], CAL-5): median residual,
//!   median leave-one-out error, condition number;
//! - model selection ([`calibrate`], CAL-6);
//! - the [`CalibrationSession`] state machine (CAL-2, CAL-3, CAL-8): sample gating on
//!   tracking quality, settle and collect windows in frame time that pause while tracking is
//!   lost, retry once then missing. Timing and acceptance only; the UI is native.
//!
//! Pure maths: no I/O, no wall clock, no randomness (PIPE-5).
//!
//! `angularAffine` works on the eye-in-head gaze direction (trap 4): the direction from the
//! eye midpoint to the look-at point, both in the face-anchor frame, as
//! `yaw = atan2(x, z)`, `pitch = atan2(y, sqrt(x² + z²))`. The model maps measured angles to
//! the angles of the direction from the same eye origin to the target, so the correction is
//! independent of where the head is; it is applied before the raycast
//! ([`CalModel::apply_sample`]). Its error for a point is the angle between the corrected and
//! the target direction, which equals the visual angle between their screen hits because both
//! rays leave the same eye.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::geometry::{angle_between_norm_deg, angle_deg, OrientedGeometry};
use super::raycast::{raycast, raycast_parts, RayHit, CAMERA_AXES, FALLBACK_EYE_UP_M};
use crate::quality::QualityGate;
use crate::types::tracking::{
    CalModelKind, CalibrationParams, CalibrationPose, CalibrationQuality, CalibrationRecord,
    ConfidenceParams, Orientation, ProcessedSample, RawTrackingSample, RayStatus, TargetKind,
    TargetOutcome, TargetResult, TrackState,
};

/// Models from the simplest to the most complex (CAL-6 "simplest eligible model"). The two
/// affine models have the same parameter count; `screenAffine` (today's model) ranks first.
pub const COMPLEXITY_ORDER: [CalModelKind; 4] = [
    CalModelKind::ScreenAffine,
    CalModelKind::AngularAffine,
    CalModelKind::ScreenHomography,
    CalModelKind::ScreenPoly2,
];

/// Smallest singular value relative to the largest below which a design matrix is singular
/// (the fit fails with [`CalError::Degenerate`]). Ill-conditioned but solvable layouts fit and
/// are failed by the `maxConditionNumber` quality guard instead (CAL-5).
const RANK_TOL: f64 = 1e-10;

/// Tolerance for time-window edges, ms (frame clocks accumulate rounding error).
const EPS_MS: f64 = 1e-6;

/// Minimum usable points to fit a model at all (CAL-4).
pub fn min_points(kind: CalModelKind) -> usize {
    match kind {
        CalModelKind::ScreenAffine | CalModelKind::AngularAffine => 3,
        CalModelKind::ScreenHomography => 4,
        CalModelKind::ScreenPoly2 => 6,
    }
}

/// Free parameters of a model (CAL-4).
pub fn parameter_count(kind: CalModelKind) -> usize {
    match kind {
        CalModelKind::ScreenAffine | CalModelKind::AngularAffine => 6,
        CalModelKind::ScreenHomography => 8,
        CalModelKind::ScreenPoly2 => 12,
    }
}

/// A model is eligible with at least `minPoints + eligibilityMarginPoints` usable points
/// (CAL-4: 5 points allow only the affine models, 9 points allow all four).
pub fn is_eligible(kind: CalModelKind, usable_points: usize, params: &CalibrationParams) -> bool {
    usable_points >= min_points(kind) + params.eligibility_margin_points as usize
}

/// Why a fit or a calibration could not be produced.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CalError {
    #[error("too few calibration points: {needed} needed, {got} given")]
    TooFewPoints { needed: usize, got: usize },
    #[error("{raw} raw points but {targets} target points")]
    MismatchedCounts { raw: usize, targets: usize },
    /// Identical or collinear points (or points on one conic for `screenPoly2`).
    #[error("degenerate calibration layout (singular design matrix)")]
    Degenerate,
    /// `angularAffine` needs the eye-in-head angles of every point.
    #[error("angularAffine needs angular data for every point")]
    MissingAngularData,
    #[error("non-finite calibration input or result")]
    NonFinite,
}

/// Eye-in-head gaze angles of one target (degrees, face frame): what the tracker measured and
/// what would have hit the target.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AngularPair {
    /// `[yaw, pitch]` of the measured look-at direction.
    pub measured_deg: [f64; 2],
    /// `[yaw, pitch]` of the direction from the eye origin to the target.
    pub target_deg: [f64; 2],
}

/// One calibration point: the robust aggregate of one target (CAL-3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalPoint {
    /// Where the target was shown (screen-normalised).
    pub target_norm: [f64; 2],
    /// Robust median of the uncalibrated raycast hits (screen-normalised, unclamped).
    pub raw_norm: [f64; 2],
    /// Median eye position, oriented screen frame, mm (angles are measured from here).
    pub eye_pos_mm: [f64; 3],
    /// Absent when the samples carried no face pose (then `angularAffine` cannot use it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub angular: Option<AngularPair>,
}

impl CalPoint {
    /// Points from paired raw and target arrays with one eye position (the shape of the Swift
    /// `GazeCalibrationProfile.fit(rawPoints:targetPoints:)`).
    pub fn from_pairs(
        raw: &[[f64; 2]],
        targets: &[[f64; 2]],
        eye_pos_mm: [f64; 3],
    ) -> Result<Vec<CalPoint>, CalError> {
        if raw.len() != targets.len() {
            return Err(CalError::MismatchedCounts {
                raw: raw.len(),
                targets: targets.len(),
            });
        }
        Ok(raw
            .iter()
            .zip(targets)
            .map(|(r, t)| CalPoint {
                target_norm: *t,
                raw_norm: *r,
                eye_pos_mm,
                angular: None,
            })
            .collect())
    }
}

// ------------------------------------------------------------------------------------------
// Models (CAL-4)
// ------------------------------------------------------------------------------------------

/// A fitted calibration model. Serialised with a `kind` tag (the [`CalModelKind`] name) so the
/// native apps can store it as an opaque JSON blob bound to the session (CAL-1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum CalModel {
    /// `x' = x[0] x + x[1] y + x[2]`, `y' = y[0] x + y[1] y + y[2]` (Swift `a..f`).
    ScreenAffine { x: [f64; 3], y: [f64; 3] },
    /// `yaw' = yaw[0] yaw + yaw[1] pitch + yaw[2]`, same for pitch; degrees, face frame.
    AngularAffine { yaw: [f64; 3], pitch: [f64; 3] },
    /// Row-major 3x3 `H`: `(u, v, w) = H (x, y, 1)`, result `(u / w, v / w)`.
    ScreenHomography { h: [f64; 9] },
    /// Terms `1, x̂, ŷ, x̂ŷ, x̂², ŷ²` per axis on the normalised input
    /// `x̂ = (x - center[0]) * scale`, `ŷ = (y - center[1]) * scale`.
    ScreenPoly2 {
        center: [f64; 2],
        scale: f64,
        x: [f64; 6],
        y: [f64; 6],
    },
}

impl CalModel {
    /// The pass-through model of a kind.
    pub fn identity(kind: CalModelKind) -> CalModel {
        match kind {
            CalModelKind::ScreenAffine => CalModel::ScreenAffine {
                x: [1.0, 0.0, 0.0],
                y: [0.0, 1.0, 0.0],
            },
            CalModelKind::AngularAffine => CalModel::AngularAffine {
                yaw: [1.0, 0.0, 0.0],
                pitch: [0.0, 1.0, 0.0],
            },
            CalModelKind::ScreenHomography => CalModel::ScreenHomography {
                h: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
            },
            CalModelKind::ScreenPoly2 => CalModel::ScreenPoly2 {
                center: [0.0, 0.0],
                scale: 1.0,
                x: [0.0, 1.0, 0.0, 0.0, 0.0, 0.0],
                y: [0.0, 0.0, 1.0, 0.0, 0.0, 0.0],
            },
        }
    }

    pub fn kind(&self) -> CalModelKind {
        match self {
            CalModel::ScreenAffine { .. } => CalModelKind::ScreenAffine,
            CalModel::AngularAffine { .. } => CalModelKind::AngularAffine,
            CalModel::ScreenHomography { .. } => CalModelKind::ScreenHomography,
            CalModel::ScreenPoly2 { .. } => CalModelKind::ScreenPoly2,
        }
    }

    /// Least-squares fit mapping raw to target (CAL-4).
    pub fn fit(kind: CalModelKind, points: &[CalPoint]) -> Result<CalModel, CalError> {
        fit_with_condition(kind, points).map(|(m, _)| m)
    }

    /// Applies a screen model to a raw screen-normalised point. No clamping. `None` for
    /// `angularAffine` (it needs the face pose: [`CalModel::apply_sample`]) and for a
    /// non-finite result (a homography point at infinity).
    pub fn apply_point(&self, raw: [f64; 2]) -> Option<[f64; 2]> {
        let [x, y] = raw;
        let out = match self {
            CalModel::ScreenAffine { x: a, y: b } => {
                [a[0] * x + a[1] * y + a[2], b[0] * x + b[1] * y + b[2]]
            }
            CalModel::AngularAffine { .. } => return None,
            CalModel::ScreenHomography { h } => {
                let w = h[6] * x + h[7] * y + h[8];
                [
                    (h[0] * x + h[1] * y + h[2]) / w,
                    (h[3] * x + h[4] * y + h[5]) / w,
                ]
            }
            CalModel::ScreenPoly2 {
                center,
                scale,
                x: cx,
                y: cy,
            } => {
                let t = poly2_terms([(x - center[0]) * scale, (y - center[1]) * scale]);
                [dot(cx, &t), dot(cy, &t)]
            }
        };
        finite2(out).then_some(out)
    }

    /// Applies `angularAffine` to measured `[yaw, pitch]` (degrees). `None` for screen models.
    pub fn apply_angles(&self, measured_deg: [f64; 2]) -> Option<[f64; 2]> {
        match self {
            CalModel::AngularAffine { yaw, pitch } => {
                let [a, b] = measured_deg;
                let out = [
                    yaw[0] * a + yaw[1] * b + yaw[2],
                    pitch[0] * a + pitch[1] * b + pitch[2],
                ];
                finite2(out).then_some(out)
            }
            _ => None,
        }
    }

    /// Calibrated raycast of one raw frame, for every model kind (the pipeline's path).
    ///
    /// Screen models raycast (GEO-5) and map the hit. `angularAffine` corrects the eye-in-head
    /// direction first, keeping the look-at distance, and raycasts the corrected ray, so the
    /// status (`backward`, `grazing`, `outOfRange`) is that of the corrected ray. `None` when
    /// the frame has no face anchor. Invariant of [`RayHit`] kept: `ok` implies a point.
    pub fn apply_sample(
        &self,
        sample: &RawTrackingSample,
        geo: &OrientedGeometry,
    ) -> Option<RayHit> {
        match self {
            CalModel::AngularAffine { .. } => {
                let face = sample.face_transform.as_ref()?;
                let look_at = sample.look_at.as_ref()?;
                let left = sample.left_eye_transform.as_ref();
                let right = sample.right_eye_transform.as_ref();
                let eye = eye_origin_face(left, right);
                let d = sub3(*look_at, eye);
                let len = norm3(d);
                let corrected = if len > 0.0 && len.is_finite() {
                    let a = self.apply_angles(angles_of_dir(d))?;
                    let u = dir_of_angles(a);
                    [
                        eye[0] + len * u[0],
                        eye[1] + len * u[1],
                        eye[2] + len * u[2],
                    ]
                } else {
                    *look_at
                };
                Some(raycast_parts(face, &corrected, left, right, geo))
            }
            _ => {
                let mut hit = raycast(sample, geo)?;
                if hit.status == RayStatus::Ok {
                    match hit.point_norm.and_then(|p| self.apply_point(p)) {
                        Some(p) => hit.point_norm = Some(p),
                        None => {
                            hit.status = RayStatus::OutOfRange;
                            hit.point_norm = None;
                        }
                    }
                }
                Some(hit)
            }
        }
    }

    /// Visual-angle error (degrees) of the model on one point: for screen models the angle
    /// between the calibrated point and the target seen from the point's eye position; for
    /// `angularAffine` the angle between the corrected and the target direction. `None` when
    /// the model cannot be applied to the point.
    pub fn point_error_deg(&self, p: &CalPoint, geo: &OrientedGeometry) -> Option<f64> {
        let e = match self {
            CalModel::AngularAffine { .. } => {
                let pair = p.angular?;
                let c = self.apply_angles(pair.measured_deg)?;
                angle_deg(dir_of_angles(c), dir_of_angles(pair.target_deg), [0.0; 3])
            }
            _ => {
                let q = self.apply_point(p.raw_norm)?;
                angle_between_norm_deg(geo, q, p.target_norm, p.eye_pos_mm)
            }
        };
        e.is_finite().then_some(e)
    }
}

/// Fit plus the condition number of the normalised design matrix (CAL-5).
pub fn fit_with_condition(
    kind: CalModelKind,
    points: &[CalPoint],
) -> Result<(CalModel, f64), CalError> {
    let needed = min_points(kind);
    if points.len() < needed {
        return Err(CalError::TooFewPoints {
            needed,
            got: points.len(),
        });
    }
    let finite = points
        .iter()
        .all(|p| finite2(p.raw_norm) && finite2(p.target_norm));
    if !finite {
        return Err(CalError::NonFinite);
    }
    let raw: Vec<[f64; 2]> = points.iter().map(|p| p.raw_norm).collect();
    let tgt: Vec<[f64; 2]> = points.iter().map(|p| p.target_norm).collect();
    match kind {
        CalModelKind::ScreenAffine => {
            let (x, y, cond) = fit_affine(&raw, &tgt)?;
            Ok((CalModel::ScreenAffine { x, y }, cond))
        }
        CalModelKind::AngularAffine => {
            let pairs: Option<Vec<AngularPair>> = points.iter().map(|p| p.angular).collect();
            let pairs = pairs.ok_or(CalError::MissingAngularData)?;
            if !pairs
                .iter()
                .all(|a| finite2(a.measured_deg) && finite2(a.target_deg))
            {
                return Err(CalError::NonFinite);
            }
            let meas: Vec<[f64; 2]> = pairs.iter().map(|a| a.measured_deg).collect();
            let want: Vec<[f64; 2]> = pairs.iter().map(|a| a.target_deg).collect();
            let (yaw, pitch, cond) = fit_affine(&meas, &want)?;
            Ok((CalModel::AngularAffine { yaw, pitch }, cond))
        }
        CalModelKind::ScreenHomography => {
            let (h, cond) = fit_homography(&raw, &tgt)?;
            Ok((CalModel::ScreenHomography { h }, cond))
        }
        CalModelKind::ScreenPoly2 => {
            let n = Norm2::of(&raw)?;
            let rows: Vec<Vec<f64>> = raw
                .iter()
                .map(|&p| poly2_terms(n.apply(p)).to_vec())
                .collect();
            let tx: Vec<f64> = tgt.iter().map(|t| t[0]).collect();
            let ty: Vec<f64> = tgt.iter().map(|t| t[1]).collect();
            let (sol, cond) = lstsq(&Mat::from_rows(&rows), &[&tx, &ty])?;
            let mut x = [0.0; 6];
            let mut y = [0.0; 6];
            x.copy_from_slice(&sol[0]);
            y.copy_from_slice(&sol[1]);
            Ok((
                CalModel::ScreenPoly2 {
                    center: [n.cx, n.cy],
                    scale: n.s,
                    x,
                    y,
                },
                cond,
            ))
        }
    }
}

fn poly2_terms(p: [f64; 2]) -> [f64; 6] {
    let [x, y] = p;
    [1.0, x, y, x * y, x * x, y * y]
}

/// Affine `dst ≈ A src + b` on Hartley-normalised `src`, returned denormalised.
fn fit_affine(src: &[[f64; 2]], dst: &[[f64; 2]]) -> Result<([f64; 3], [f64; 3], f64), CalError> {
    let n = Norm2::of(src)?;
    let rows: Vec<Vec<f64>> = src
        .iter()
        .map(|&p| {
            let q = n.apply(p);
            vec![q[0], q[1], 1.0]
        })
        .collect();
    let dx: Vec<f64> = dst.iter().map(|t| t[0]).collect();
    let dy: Vec<f64> = dst.iter().map(|t| t[1]).collect();
    let (sol, cond) = lstsq(&Mat::from_rows(&rows), &[&dx, &dy])?;
    let denorm = |c: &[f64]| {
        let a = c[0] * n.s;
        let b = c[1] * n.s;
        [a, b, c[2] - a * n.cx - b * n.cy]
    };
    Ok((denorm(&sol[0]), denorm(&sol[1]), cond))
}

/// Normalised DLT (Hartley): both point sets normalised, `h` = right singular vector of the
/// smallest singular value, denormalised as `H = T_dst⁻¹ Ĥ T_src`. Condition number is
/// `σ_max / σ_8` (the ninth singular value is the null space).
fn fit_homography(src: &[[f64; 2]], dst: &[[f64; 2]]) -> Result<([f64; 9], f64), CalError> {
    let ns = Norm2::of(src)?;
    let nd = Norm2::of(dst)?;
    let mut rows: Vec<Vec<f64>> = Vec::with_capacity(2 * src.len() + 1);
    for (s, d) in src.iter().zip(dst) {
        let [x, y] = ns.apply(*s);
        let [u, v] = nd.apply(*d);
        rows.push(vec![0.0, 0.0, 0.0, -x, -y, -1.0, v * x, v * y, v]);
        rows.push(vec![x, y, 1.0, 0.0, 0.0, 0.0, -u * x, -u * y, -u]);
    }
    while rows.len() < 9 {
        rows.push(vec![0.0; 9]);
    }
    let svd = Svd::of(&Mat::from_rows(&rows));
    let mut order: Vec<usize> = (0..9).collect();
    order.sort_by(|&a, &b| svd.sigma[b].total_cmp(&svd.sigma[a]));
    let smax = svd.sigma[order[0]];
    let s8 = svd.sigma[order[7]];
    if !gt(smax, 0.0) || !gt(s8, RANK_TOL * smax) {
        return Err(CalError::Degenerate);
    }
    let hn: Vec<f64> = (0..9).map(|r| svd.v.get(r, order[8])).collect();
    // H = T_dst^-1 * Hn * T_src
    let t_src = [
        ns.s,
        0.0,
        -ns.s * ns.cx,
        0.0,
        ns.s,
        -ns.s * ns.cy,
        0.0,
        0.0,
        1.0,
    ];
    let t_dst_inv = [
        1.0 / nd.s,
        0.0,
        nd.cx,
        0.0,
        1.0 / nd.s,
        nd.cy,
        0.0,
        0.0,
        1.0,
    ];
    let mut h = mat3_mul(&t_dst_inv, &mat3_mul(&hn, &t_src));
    let scale = if h[8].abs() > 1e-12 {
        h[8]
    } else {
        h.iter().map(|v| v * v).sum::<f64>().sqrt()
    };
    for v in &mut h {
        *v /= scale;
    }
    if !h.iter().all(|v| v.is_finite()) {
        return Err(CalError::NonFinite);
    }
    Ok((h, smax / s8))
}

fn mat3_mul(a: &[f64], b: &[f64]) -> [f64; 9] {
    let mut out = [0.0; 9];
    for r in 0..3 {
        for c in 0..3 {
            out[r * 3 + c] = (0..3).map(|k| a[r * 3 + k] * b[k * 3 + c]).sum();
        }
    }
    out
}

// ------------------------------------------------------------------------------------------
// Eye-in-head angles (trap 4)
// ------------------------------------------------------------------------------------------

/// Eye origin in the face frame, by the same rule as the raycast (GEO-5): midpoint of the
/// eye transforms, or the face origin plus [`FALLBACK_EYE_UP_M`] along the face's up axis
/// when either is missing or the left one has a zero translation.
fn eye_origin_face(left: Option<&[f64; 16]>, right: Option<&[f64; 16]>) -> [f64; 3] {
    match (left, right) {
        (Some(l), Some(r)) if [l[12], l[13], l[14]] != [0.0, 0.0, 0.0] => [
            (l[12] + r[12]) / 2.0,
            (l[13] + r[13]) / 2.0,
            (l[14] + r[14]) / 2.0,
        ],
        _ => [0.0, FALLBACK_EYE_UP_M, 0.0],
    }
}

/// `[yaw, pitch]` in degrees of a face-frame direction (trap 4).
fn angles_of_dir(d: [f64; 3]) -> [f64; 2] {
    [
        d[0].atan2(d[2]).to_degrees(),
        d[1].atan2((d[0] * d[0] + d[2] * d[2]).sqrt()).to_degrees(),
    ]
}

/// Unit face-frame direction of `[yaw, pitch]` degrees (inverse of [`angles_of_dir`]).
fn dir_of_angles(a: [f64; 2]) -> [f64; 3] {
    let (sy, cy) = a[0].to_radians().sin_cos();
    let (sp, cp) = a[1].to_radians().sin_cos();
    [cp * sy, sp, cp * cy]
}

/// Measured eye-in-head `[yaw, pitch]` (degrees) of a raw frame: the direction from the eye
/// origin to the look-at point, face frame. `None` without a look-at point or for a
/// zero-length direction.
pub fn gaze_angles_deg(sample: &RawTrackingSample) -> Option<[f64; 2]> {
    let look_at = sample.look_at?;
    let eye = eye_origin_face(
        sample.left_eye_transform.as_ref(),
        sample.right_eye_transform.as_ref(),
    );
    let d = sub3(look_at, eye);
    let n = norm3(d);
    (n > 0.0 && n.is_finite()).then(|| angles_of_dir(d))
}

/// `[yaw, pitch]` (degrees, face frame) of the direction from the frame's eye origin to a
/// screen target. `None` without a face transform or with a singular one.
pub fn target_angles_deg(
    sample: &RawTrackingSample,
    geo: &OrientedGeometry,
    target_norm: [f64; 2],
) -> Option<[f64; 2]> {
    let face = sample.face_transform.as_ref()?;
    let axes = CAMERA_AXES;
    let native = oriented_to_native_mm(geo, geo.norm_to_mm(target_norm));
    // Inverse of the raycast's camera-metres -> native-mm mapping, on the plane z = 0.
    let cam = [
        (native[0] - geo.camera_native_mm[0]) / (1000.0 * axes.x_to_native_right),
        (geo.camera_native_mm[1] - native[1]) / (1000.0 * axes.y_to_native_up),
        0.0,
    ];
    let p = inverse_transform_point(face, cam)?;
    let eye = eye_origin_face(
        sample.left_eye_transform.as_ref(),
        sample.right_eye_transform.as_ref(),
    );
    let d = sub3(p, eye);
    let n = norm3(d);
    (n > 0.0 && n.is_finite()).then(|| angles_of_dir(d))
}

/// Measured and target angles of one frame for one target (input of `angularAffine`).
pub fn angular_pair(
    sample: &RawTrackingSample,
    geo: &OrientedGeometry,
    target_norm: [f64; 2],
) -> Option<AngularPair> {
    Some(AngularPair {
        measured_deg: gaze_angles_deg(sample)?,
        target_deg: target_angles_deg(sample, geo, target_norm)?,
    })
}

/// Oriented-frame millimetres back to the portrait native frame (inverse of
/// `OrientedGeometry::native_to_oriented_mm`).
pub fn oriented_to_native_mm(geo: &OrientedGeometry, p: [f64; 2]) -> [f64; 2] {
    let (w, h) = (geo.native_width_mm, geo.native_height_mm);
    let [x, y] = p;
    match geo.orientation {
        Orientation::Portrait => [x, y],
        Orientation::PortraitUpsideDown => [w - x, h - y],
        Orientation::LandscapeLeft => [y, h - x],
        Orientation::LandscapeRight => [w - y, x],
    }
}

/// `M⁻¹ p` for a column-major 4x4 affine transform (general 3x3 inverse, so a scaled
/// transform still works). `None` when the linear part is singular.
fn inverse_transform_point(m: &[f64; 16], p: [f64; 3]) -> Option<[f64; 3]> {
    // Linear part L with columns (m0, m1, m2), (m4, m5, m6), (m8, m9, m10).
    let (a, b, c) = (m[0], m[4], m[8]);
    let (d, e, f) = (m[1], m[5], m[9]);
    let (g, h, i) = (m[2], m[6], m[10]);
    let det = a * (e * i - f * h) - b * (d * i - f * g) + c * (d * h - e * g);
    if !gt(det.abs(), 1e-12) {
        return None;
    }
    let q = [p[0] - m[12], p[1] - m[13], p[2] - m[14]];
    let inv = [
        [(e * i - f * h), -(b * i - c * h), (b * f - c * e)],
        [-(d * i - f * g), (a * i - c * g), -(a * f - c * d)],
        [(d * h - e * g), -(a * h - b * g), (a * e - b * d)],
    ];
    let out = [
        (inv[0][0] * q[0] + inv[0][1] * q[1] + inv[0][2] * q[2]) / det,
        (inv[1][0] * q[0] + inv[1][1] * q[1] + inv[1][2] * q[2]) / det,
        (inv[2][0] * q[0] + inv[2][1] * q[1] + inv[2][2] * q[2]) / det,
    ];
    finite3(out).then_some(out)
}

// ------------------------------------------------------------------------------------------
// Quality (CAL-5) and selection (CAL-6)
// ------------------------------------------------------------------------------------------

/// A fitted model with its quality numbers (CAL-5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelReport {
    pub model: CalModel,
    pub usable_points: u32,
    /// Median visual-angle residual on the calibration points.
    pub residual_deg: f64,
    /// Median leave-one-out error. Absent when the median is not finite (too many folds
    /// could not be fitted).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loo_cv_deg: Option<f64>,
    /// Condition number of the normalised design matrix.
    pub condition_number: f64,
}

/// Fits `kind` and measures it: residual, leave-one-out error, condition number.
///
/// A leave-one-out fold that cannot be fitted (for example a 4-point set whose remaining
/// three points are collinear) counts as an infinite error for its point: the point cannot
/// be predicted from the others. The median then stays finite while most folds fit.
pub fn evaluate(
    kind: CalModelKind,
    points: &[CalPoint],
    geo: &OrientedGeometry,
) -> Result<ModelReport, CalError> {
    let (model, cond) = fit_with_condition(kind, points)?;
    let mut fit_err: Vec<f64> = points
        .iter()
        .map(|p| model.point_error_deg(p, geo).unwrap_or(f64::INFINITY))
        .collect();
    let residual = median(&mut fit_err).ok_or(CalError::NonFinite)?;
    if !residual.is_finite() {
        return Err(CalError::NonFinite);
    }
    let mut folds: Vec<f64> = (0..points.len())
        .map(|i| {
            let rest: Vec<CalPoint> = points
                .iter()
                .enumerate()
                .filter(|(j, _)| *j != i)
                .map(|(_, p)| p.clone())
                .collect();
            CalModel::fit(kind, &rest)
                .ok()
                .and_then(|m| m.point_error_deg(&points[i], geo))
                .unwrap_or(f64::INFINITY)
        })
        .collect();
    let loo = median(&mut folds).filter(|v| v.is_finite());
    Ok(ModelReport {
        model,
        usable_points: points.len() as u32,
        residual_deg: residual,
        loo_cv_deg: loo,
        condition_number: cond,
    })
}

/// Quality band from the leave-one-out error (CAL-5): upper bounds inclusive; `failed` also
/// when `usablePoints < minUsablePoints`, the condition number exceeds `maxConditionNumber`,
/// or the error is absent or not finite.
pub fn band(
    loo_cv_deg: Option<f64>,
    usable_points: u32,
    condition_number: f64,
    params: &CalibrationParams,
) -> CalibrationQuality {
    if usable_points < params.min_usable_points
        || !le(condition_number, params.max_condition_number)
    {
        return CalibrationQuality::Failed;
    }
    let b = &params.bands;
    match loo_cv_deg {
        Some(l) if l <= b.good_max_deg => CalibrationQuality::Good,
        Some(l) if l <= b.fair_max_deg => CalibrationQuality::Fair,
        Some(l) if l <= b.poor_max_deg => CalibrationQuality::Poor,
        _ => CalibrationQuality::Failed,
    }
}

/// Outcome of [`calibrate`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalibrationFit {
    pub selected: ModelReport,
    pub quality: CalibrationQuality,
    /// Every model that was fitted, in [`COMPLEXITY_ORDER`].
    pub candidates: Vec<ModelReport>,
}

/// Fits and selects the calibration model (CAL-6).
///
/// The configured `defaultModel` is always fitted. With `autoSelect`, every eligible model is
/// fitted too (`angularAffine` only when every point has angular data) and the one with the
/// lowest LOO-CV error wins, unless it beats the simplest eligible model by less than
/// `minImprovementDeg`, in which case the simplest is used. Without an eligible model that
/// has a LOO-CV error, the default is used. Fails only when no model could be fitted (the
/// default's error is returned).
pub fn calibrate(
    points: &[CalPoint],
    params: &CalibrationParams,
    geo: &OrientedGeometry,
) -> Result<CalibrationFit, CalError> {
    let n = points.len();
    let default = params.default_model;
    let has_angular = !points.is_empty() && points.iter().all(|p| p.angular.is_some());
    let considered = |k: CalModelKind| {
        k == default
            || (params.auto_select
                && is_eligible(k, n, params)
                && (k != CalModelKind::AngularAffine || has_angular))
    };

    let mut candidates: Vec<ModelReport> = Vec::new();
    let mut default_err = None;
    for k in COMPLEXITY_ORDER.into_iter().filter(|&k| considered(k)) {
        match evaluate(k, points, geo) {
            Ok(r) => candidates.push(r),
            Err(e) if k == default => default_err = Some(e),
            Err(_) => {}
        }
    }

    let default_report = candidates.iter().find(|c| c.model.kind() == default);
    let selected = if params.auto_select {
        let eligible: Vec<&ModelReport> = candidates
            .iter()
            .filter(|c| is_eligible(c.model.kind(), n, params) && c.loo_cv_deg.is_some())
            .collect();
        match (eligible.first(), best_by_loo(&eligible)) {
            (Some(simplest), Some(best)) => {
                let gain = simplest.loo_cv_deg.unwrap_or(f64::INFINITY)
                    - best.loo_cv_deg.unwrap_or(f64::INFINITY);
                if gain < params.min_improvement_deg {
                    Some(*simplest)
                } else {
                    Some(best)
                }
            }
            _ => default_report.or(candidates.first()),
        }
    } else {
        default_report
    };

    let Some(selected) = selected.cloned() else {
        return Err(default_err.unwrap_or(CalError::TooFewPoints {
            needed: min_points(default),
            got: n,
        }));
    };
    let quality = band(
        selected.loo_cv_deg,
        selected.usable_points,
        selected.condition_number,
        params,
    );
    Ok(CalibrationFit {
        selected,
        quality,
        candidates,
    })
}

/// Lowest LOO-CV error; ties go to the simpler (earlier) model.
fn best_by_loo<'a>(reports: &[&'a ModelReport]) -> Option<&'a ModelReport> {
    let mut best: Option<&ModelReport> = None;
    for r in reports {
        let l = r.loo_cv_deg.unwrap_or(f64::INFINITY);
        if best.is_none_or(|b| l < b.loo_cv_deg.unwrap_or(f64::INFINITY)) {
            best = Some(r);
        }
    }
    best
}

// ------------------------------------------------------------------------------------------
// Robust per-target aggregation (CAL-3)
// ------------------------------------------------------------------------------------------

/// One accepted calibration sample (after the CAL-2 gate).
#[derive(Debug, Clone, PartialEq)]
pub struct CalSample {
    pub t_sec: f64,
    /// Unfiltered, uncalibrated raycast hit (screen-normalised, unclamped).
    pub raw_norm: [f64; 2],
    pub eye_pos_mm: [f64; 3],
    pub distance_cm: Option<f64>,
    pub head_yaw_deg: Option<f64>,
    pub head_pitch_deg: Option<f64>,
    pub angular: Option<AngularPair>,
}

/// Robust aggregate of one target's samples.
#[derive(Debug, Clone, PartialEq)]
pub struct TargetAggregate {
    /// Samples kept after the MAD rejection.
    pub kept: usize,
    /// Indices (into the input) of the kept samples.
    pub kept_indices: Vec<usize>,
    pub raw_median_norm: [f64; 2],
    pub eye_pos_mm_median: [f64; 3],
    /// Medians of the angular pairs; absent unless every kept sample has one.
    pub angular: Option<AngularPair>,
}

/// CAL-3: per-axis median of the raw points; samples farther than `k x MAD` from it on either
/// axis are rejected (MAD unscaled, as the spec states it; a MAD of 0 rejects every sample
/// off the median); the result is the median of the rest. `None` for no samples.
pub fn robust_aggregate(samples: &[CalSample], mad_k: f64) -> Option<TargetAggregate> {
    if samples.is_empty() {
        return None;
    }
    let mut xs: Vec<f64> = samples.iter().map(|s| s.raw_norm[0]).collect();
    let mut ys: Vec<f64> = samples.iter().map(|s| s.raw_norm[1]).collect();
    let mx = median(&mut xs)?;
    let my = median(&mut ys)?;
    let mut dx: Vec<f64> = samples.iter().map(|s| (s.raw_norm[0] - mx).abs()).collect();
    let mut dy: Vec<f64> = samples.iter().map(|s| (s.raw_norm[1] - my).abs()).collect();
    let madx = median(&mut dx)?;
    let mady = median(&mut dy)?;
    let kept_indices: Vec<usize> = samples
        .iter()
        .enumerate()
        .filter(|(_, s)| {
            (s.raw_norm[0] - mx).abs() <= mad_k * madx && (s.raw_norm[1] - my).abs() <= mad_k * mady
        })
        .map(|(i, _)| i)
        .collect();
    if kept_indices.is_empty() {
        return None;
    }
    let kept: Vec<&CalSample> = kept_indices.iter().map(|&i| &samples[i]).collect();
    let med = |f: &dyn Fn(&CalSample) -> f64| {
        let mut v: Vec<f64> = kept.iter().map(|s| f(s)).collect();
        median(&mut v).unwrap_or(f64::NAN)
    };
    let angular = if kept.iter().all(|s| s.angular.is_some()) {
        let a = |s: &CalSample| {
            s.angular.unwrap_or(AngularPair {
                measured_deg: [f64::NAN; 2],
                target_deg: [f64::NAN; 2],
            })
        };
        Some(AngularPair {
            measured_deg: [
                med(&|s| a(s).measured_deg[0]),
                med(&|s| a(s).measured_deg[1]),
            ],
            target_deg: [med(&|s| a(s).target_deg[0]), med(&|s| a(s).target_deg[1])],
        })
    } else {
        None
    };
    Some(TargetAggregate {
        kept: kept.len(),
        raw_median_norm: [med(&|s| s.raw_norm[0]), med(&|s| s.raw_norm[1])],
        eye_pos_mm_median: [
            med(&|s| s.eye_pos_mm[0]),
            med(&|s| s.eye_pos_mm[1]),
            med(&|s| s.eye_pos_mm[2]),
        ],
        angular,
        kept_indices,
    })
}

// ------------------------------------------------------------------------------------------
// CalibrationSession (CAL-2, CAL-3, CAL-8)
// ------------------------------------------------------------------------------------------

/// CAL-2 sample gate limits. `min_confidence` is `ScoringProfile.qualityGate.minConfidence`;
/// the head-speed limit is `trackingQuality.confidence.maxHeadSpeedDegPerSec`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SampleGate {
    pub min_confidence: f64,
    pub max_head_speed_deg_per_sec: f64,
}

impl SampleGate {
    pub fn from_params(gate: &QualityGate, confidence: &ConfidenceParams) -> SampleGate {
        SampleGate {
            min_confidence: gate.min_confidence,
            max_head_speed_deg_per_sec: confidence.max_head_speed_deg_per_sec,
        }
    }

    /// Whether a processed frame may contribute a calibration sample (CAL-2): tracked, a
    /// finite raycast hit and eye position, confidence at or above the minimum, head speed
    /// below the limit (a frame without a head speed yet, the first one, passes).
    pub fn accepts(&self, p: &ProcessedSample) -> bool {
        p.track_state == TrackState::Tracked
            && p.raw_point_norm.is_some_and(finite2)
            && p.eye_pos_mm.is_some_and(finite3)
            && p.confidence >= self.min_confidence
            && p.head_speed_deg_per_sec
                .is_none_or(|v| v < self.max_head_speed_deg_per_sec)
    }
}

/// What the UI should do next.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TargetStatus {
    /// Show this target and call [`CalibrationSession::begin_target`] when it appears.
    Pending {
        index: usize,
        target_norm: [f64; 2],
        attempt: u32,
    },
    /// A target window is open; keep pushing frames.
    Running {
        index: usize,
        target_norm: [f64; 2],
        attempt: u32,
        started_at_sec: f64,
    },
    /// Every target was shown; call [`CalibrationSession::finish`].
    Complete,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CalSessionError {
    #[error("a target window is already open")]
    AlreadyRunning,
    #[error("every calibration target has been shown")]
    Complete,
}

#[derive(Debug, Clone)]
struct OpenWindow {
    started_at_sec: f64,
    prev_t_sec: f64,
    prev_tracked: bool,
    active_ms: f64,
    total_ms: f64,
    frames: u32,
    samples: Vec<CalSample>,
}

/// `(distanceCm, headYawDeg, headPitchDeg)` of one kept sample.
type PoseSample = (Option<f64>, Option<f64>, Option<f64>);

#[derive(Debug, Clone)]
struct HitTarget {
    result_index: usize,
    point: CalPoint,
    pose: Vec<PoseSample>,
}

/// Result of [`CalibrationSession::finish`].
#[derive(Debug, Clone, PartialEq)]
pub struct CalibrationOutcome {
    pub fit: Result<CalibrationFit, CalError>,
    /// `trackingQuality.calibration` (QS-1). `ageSecAtStart` is left to the pipeline;
    /// `attempts` is 1 (re-calibrations are counted by the pipeline).
    pub record: CalibrationRecord,
    /// Every target window in order, hits with their calibrated median and residual.
    pub targets: Vec<TargetResult>,
    /// The points that were fitted.
    pub points: Vec<CalPoint>,
}

/// Calibration target sequencing, timing and sample acceptance (CAL-8).
///
/// Per target: the window opens at [`begin_target`](Self::begin_target) and lasts
/// `settleMs + collectMs` of **tracked** frame time: an interval between two frames counts
/// only when both are `tracked`, so the window pauses (not resets) during track loss and
/// blinks. Samples whose tracked time is still inside `settleMs` are dropped. The window
/// also closes when `maxTargetWaitMs` of frame time has passed since it opened. A target
/// with fewer than `minSamplesPerTarget` samples after the MAD rejection is shown again
/// (`maxRetriesPerTarget`, default once, immediately), then marked missed; it never falls
/// back to the target position (G-F11).
#[derive(Debug, Clone)]
pub struct CalibrationSession {
    layout: Vec<[f64; 2]>,
    params: CalibrationParams,
    gate: SampleGate,
    geo: OrientedGeometry,
    index: usize,
    attempt: u32,
    open: Option<OpenWindow>,
    results: Vec<TargetResult>,
    hits: Vec<HitTarget>,
}

impl CalibrationSession {
    pub fn new(
        layout: Vec<[f64; 2]>,
        params: CalibrationParams,
        gate: SampleGate,
        geo: OrientedGeometry,
    ) -> CalibrationSession {
        CalibrationSession {
            layout,
            params,
            gate,
            geo,
            index: 0,
            attempt: 1,
            open: None,
            results: Vec::new(),
            hits: Vec::new(),
        }
    }

    pub fn status(&self) -> TargetStatus {
        if self.index >= self.layout.len() {
            return TargetStatus::Complete;
        }
        let target_norm = self.layout[self.index];
        match &self.open {
            Some(w) => TargetStatus::Running {
                index: self.index,
                target_norm,
                attempt: self.attempt,
                started_at_sec: w.started_at_sec,
            },
            None => TargetStatus::Pending {
                index: self.index,
                target_norm,
                attempt: self.attempt,
            },
        }
    }

    /// Opens the window of the pending target at frame time `t_sec` (when it appears).
    pub fn begin_target(&mut self, t_sec: f64) -> Result<(), CalSessionError> {
        if self.index >= self.layout.len() {
            return Err(CalSessionError::Complete);
        }
        if self.open.is_some() {
            return Err(CalSessionError::AlreadyRunning);
        }
        self.open = Some(OpenWindow {
            started_at_sec: t_sec,
            prev_t_sec: t_sec,
            prev_tracked: true,
            active_ms: 0.0,
            total_ms: 0.0,
            frames: 0,
            samples: Vec::new(),
        });
        Ok(())
    }

    /// Feeds one pipeline frame. `raw` (the same frame's raw sample) supplies the face pose for
    /// `angularAffine`; without it the samples carry no angular data. Returns the target's
    /// result when this frame closed the window. Frames with no open window, or older than
    /// the previous frame, are ignored.
    pub fn push(
        &mut self,
        p: &ProcessedSample,
        raw: Option<&RawTrackingSample>,
    ) -> Option<TargetResult> {
        let settle = self.params.settle_ms;
        let window = settle + self.params.collect_ms;
        let max_wait = self.params.max_target_wait_ms;
        let target = *self.layout.get(self.index)?;
        let w = self.open.as_mut()?;
        if !le(w.prev_t_sec, p.t_sec) {
            return None;
        }
        let dt_ms = (p.t_sec - w.prev_t_sec) * 1000.0;
        let tracked = p.track_state == TrackState::Tracked;
        w.total_ms += dt_ms;
        if tracked && w.prev_tracked {
            w.active_ms += dt_ms;
        }
        w.frames += 1;
        w.prev_t_sec = p.t_sec;
        w.prev_tracked = tracked;

        if w.active_ms > settle + EPS_MS && self.gate.accepts(p) {
            if let (Some(raw_norm), Some(eye)) = (p.raw_point_norm, p.eye_pos_mm) {
                w.samples.push(CalSample {
                    t_sec: p.t_sec,
                    raw_norm,
                    eye_pos_mm: eye,
                    distance_cm: p.distance_cm,
                    head_yaw_deg: p.head_yaw_rad.map(f64::to_degrees),
                    head_pitch_deg: p.head_pitch_rad.map(f64::to_degrees),
                    angular: raw.and_then(|r| angular_pair(r, &self.geo, target)),
                });
            }
        }

        let elapsed_ms = (p.t_sec - w.started_at_sec) * 1000.0;
        if w.active_ms >= window - EPS_MS || elapsed_ms >= max_wait - EPS_MS {
            return self.close(p.t_sec);
        }
        None
    }

    /// Closes the open window early (operator skip, app interruption) and judges it on the
    /// samples collected so far. `None` without an open window.
    pub fn end_target(&mut self, t_sec: f64) -> Option<TargetResult> {
        self.open.as_ref()?;
        self.close(t_sec)
    }

    fn close(&mut self, t_sec: f64) -> Option<TargetResult> {
        let w = self.open.take()?;
        let target = *self.layout.get(self.index)?;
        let agg = robust_aggregate(&w.samples, self.params.mad_reject_k);
        let kept = agg.as_ref().map_or(0, |a| a.kept);
        let outcome = if kept >= self.params.min_samples_per_target as usize {
            TargetOutcome::Hit
        } else if self.attempt <= self.params.max_retries_per_target {
            TargetOutcome::NeedsRetry
        } else {
            TargetOutcome::Missed
        };
        let data_loss = if w.total_ms > 0.0 {
            (1.0 - w.active_ms / w.total_ms).clamp(0.0, 1.0)
        } else {
            1.0
        };
        let hit = outcome == TargetOutcome::Hit;
        let result = TargetResult {
            kind: TargetKind::Calibration,
            x_norm: target[0],
            y_norm: target[1],
            started_at_sec: w.started_at_sec,
            ended_at_sec: t_sec,
            attempt: self.attempt,
            outcome,
            valid_samples: kept as u32,
            total_samples: w.frames,
            data_loss,
            raw_median_norm: agg.as_ref().filter(|_| hit).map(|a| a.raw_median_norm),
            calibrated_median_norm: None,
            eye_pos_mm_median: agg.as_ref().filter(|_| hit).map(|a| a.eye_pos_mm_median),
            accuracy_deg: None,
            offset_deg: None,
            precision_rms_deg: None,
            precision_rms_raw_deg: None,
            sd_deg: None,
        };
        if let (true, Some(a)) = (hit, agg) {
            self.hits.push(HitTarget {
                result_index: self.results.len(),
                point: CalPoint {
                    target_norm: target,
                    raw_norm: a.raw_median_norm,
                    eye_pos_mm: a.eye_pos_mm_median,
                    angular: a.angular,
                },
                pose: a
                    .kept_indices
                    .iter()
                    .map(|&i| {
                        let s = &w.samples[i];
                        (s.distance_cm, s.head_yaw_deg, s.head_pitch_deg)
                    })
                    .collect(),
            });
        }
        if outcome == TargetOutcome::NeedsRetry {
            self.attempt += 1;
        } else {
            self.index += 1;
            self.attempt = 1;
        }
        self.results.push(result.clone());
        Some(result)
    }

    /// Every closed target window so far.
    pub fn results(&self) -> &[TargetResult] {
        &self.results
    }

    /// The calibration points collected so far (one per hit target).
    pub fn points(&self) -> Vec<CalPoint> {
        self.hits.iter().map(|h| h.point.clone()).collect()
    }

    /// Fits and scores the calibration from the hit targets (CAL-5, CAL-6). Targets not hit
    /// (missed, or never shown) count as missing.
    pub fn finish(&self) -> CalibrationOutcome {
        let points = self.points();
        let fit = if points.is_empty() {
            Err(CalError::TooFewPoints {
                needed: min_points(self.params.default_model),
                got: 0,
            })
        } else {
            calibrate(&points, &self.params, &self.geo)
        };

        let mut targets = self.results.clone();
        if let Ok(f) = &fit {
            for h in &self.hits {
                if let Some(r) = targets.get_mut(h.result_index) {
                    r.calibrated_median_norm = f.selected.model.apply_point(h.point.raw_norm);
                    r.accuracy_deg = f.selected.model.point_error_deg(&h.point, &self.geo);
                }
            }
        }

        let pose = {
            let all: Vec<_> = self.hits.iter().flat_map(|h| h.pose.iter()).collect();
            let col = |f: &dyn Fn(&PoseSample) -> Option<f64>| {
                let v: Option<Vec<f64>> = all.iter().map(|p| f(p)).collect();
                v.and_then(|mut v| median(&mut v))
            };
            match (col(&|p| p.0), col(&|p| p.1), col(&|p| p.2)) {
                (Some(distance_cm), Some(yaw_deg), Some(pitch_deg)) => Some(CalibrationPose {
                    distance_cm,
                    yaw_deg,
                    pitch_deg,
                }),
                _ => None,
            }
        };

        let usable = points.len() as u32;
        let layout = self.layout.len() as u32;
        let record = CalibrationRecord {
            model: fit.as_ref().ok().map(|f| f.selected.model.kind()),
            layout_points: layout,
            usable_points: usable,
            missing_points: layout.saturating_sub(usable),
            residual_deg: fit.as_ref().ok().map(|f| f.selected.residual_deg),
            loo_cv_deg: fit.as_ref().ok().and_then(|f| f.selected.loo_cv_deg),
            condition_number: fit.as_ref().ok().map(|f| f.selected.condition_number),
            quality: fit
                .as_ref()
                .map_or(CalibrationQuality::Failed, |f| f.quality),
            pose,
            age_sec_at_start: None,
            attempts: 1,
        };
        CalibrationOutcome {
            fit,
            record,
            targets,
            points,
        }
    }
}

// ------------------------------------------------------------------------------------------
// Small numerics
// ------------------------------------------------------------------------------------------

/// `a > b`, false when either is NaN (so a NaN fails every guard written as `!gt(..)`).
fn gt(a: f64, b: f64) -> bool {
    a > b
}

/// `a <= b`, false when either is NaN.
fn le(a: f64, b: f64) -> bool {
    a <= b
}

fn finite2(p: [f64; 2]) -> bool {
    p[0].is_finite() && p[1].is_finite()
}

fn finite3(p: [f64; 3]) -> bool {
    p.iter().all(|v| v.is_finite())
}

fn sub3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn norm3(a: [f64; 3]) -> f64 {
    (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt()
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// Median (mean of the two middle values for an even count). Sorts in place with
/// `total_cmp`, so infinities sort last. `None` for an empty slice.
fn median(v: &mut [f64]) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(f64::total_cmp);
    let n = v.len();
    Some(if n % 2 == 1 {
        v[n / 2]
    } else {
        let (a, b) = (v[n / 2 - 1], v[n / 2]);
        if a == b {
            a
        } else {
            (a + b) / 2.0
        }
    })
}

/// Hartley normalisation: centroid to the origin, mean distance √2.
#[derive(Debug, Clone, Copy)]
struct Norm2 {
    cx: f64,
    cy: f64,
    s: f64,
}

impl Norm2 {
    fn of(points: &[[f64; 2]]) -> Result<Norm2, CalError> {
        let n = points.len() as f64;
        let cx = points.iter().map(|p| p[0]).sum::<f64>() / n;
        let cy = points.iter().map(|p| p[1]).sum::<f64>() / n;
        let mean_d = points
            .iter()
            .map(|p| ((p[0] - cx).powi(2) + (p[1] - cy).powi(2)).sqrt())
            .sum::<f64>()
            / n;
        if !mean_d.is_finite() {
            return Err(CalError::NonFinite);
        }
        if !gt(mean_d, 1e-12) {
            return Err(CalError::Degenerate);
        }
        Ok(Norm2 {
            cx,
            cy,
            s: std::f64::consts::SQRT_2 / mean_d,
        })
    }

    fn apply(&self, p: [f64; 2]) -> [f64; 2] {
        [(p[0] - self.cx) * self.s, (p[1] - self.cy) * self.s]
    }
}

/// Dense row-major matrix.
#[derive(Debug, Clone)]
struct Mat {
    rows: usize,
    cols: usize,
    d: Vec<f64>,
}

impl Mat {
    fn from_rows(rows: &[Vec<f64>]) -> Mat {
        let cols = rows.first().map_or(0, Vec::len);
        let mut d = Vec::with_capacity(rows.len() * cols);
        for r in rows {
            d.extend_from_slice(r);
        }
        Mat {
            rows: rows.len(),
            cols,
            d,
        }
    }

    fn identity(n: usize) -> Mat {
        let mut d = vec![0.0; n * n];
        for i in 0..n {
            d[i * n + i] = 1.0;
        }
        Mat {
            rows: n,
            cols: n,
            d,
        }
    }

    fn get(&self, r: usize, c: usize) -> f64 {
        self.d[r * self.cols + c]
    }

    /// Rotates columns `p` and `q` by (c, s): `p' = c p - s q`, `q' = s p + c q`.
    fn rotate_cols(&mut self, p: usize, q: usize, c: f64, s: f64) {
        for r in 0..self.rows {
            let i = r * self.cols;
            let (a, b) = (self.d[i + p], self.d[i + q]);
            self.d[i + p] = c * a - s * b;
            self.d[i + q] = s * a + c * b;
        }
    }

    fn col_dot(&self, p: usize, q: usize) -> f64 {
        (0..self.rows)
            .map(|r| self.get(r, p) * self.get(r, q))
            .sum()
    }
}

/// One-sided Jacobi (Hestenes) SVD: `A V = W` with orthogonal columns, `σ_j = |W_j|`.
/// Works for any shape; rank-deficient directions end as zero columns of `W`.
struct Svd {
    sigma: Vec<f64>,
    v: Mat,
    w: Mat,
}

impl Svd {
    fn of(a: &Mat) -> Svd {
        let n = a.cols;
        let mut w = a.clone();
        let mut v = Mat::identity(n);
        for _sweep in 0..80 {
            let mut rotated = false;
            for p in 0..n {
                for q in (p + 1)..n {
                    let alpha = w.col_dot(p, p);
                    let beta = w.col_dot(q, q);
                    let gamma = w.col_dot(p, q);
                    if gamma == 0.0 || gamma.abs() <= 1e-15 * (alpha * beta).sqrt() {
                        continue;
                    }
                    rotated = true;
                    let zeta = (beta - alpha) / (2.0 * gamma);
                    let t = zeta.signum() / (zeta.abs() + (1.0 + zeta * zeta).sqrt());
                    let c = 1.0 / (1.0 + t * t).sqrt();
                    let s = c * t;
                    w.rotate_cols(p, q, c, s);
                    v.rotate_cols(p, q, c, s);
                }
            }
            if !rotated {
                break;
            }
        }
        let sigma = (0..n).map(|j| w.col_dot(j, j).sqrt()).collect();
        Svd { sigma, v, w }
    }
}

/// Least squares `A x = b` for each right-hand side through the SVD. Fails with
/// [`CalError::Degenerate`] when `A` is rank deficient (smallest singular value below
/// [`RANK_TOL`] of the largest). Returns the solutions and `σ_max / σ_min`.
fn lstsq(a: &Mat, rhs: &[&[f64]]) -> Result<(Vec<Vec<f64>>, f64), CalError> {
    if a.rows < a.cols {
        return Err(CalError::TooFewPoints {
            needed: a.cols,
            got: a.rows,
        });
    }
    let svd = Svd::of(a);
    let smax = svd.sigma.iter().copied().fold(0.0, f64::max);
    let smin = svd.sigma.iter().copied().fold(f64::INFINITY, f64::min);
    if !smax.is_finite() || !smin.is_finite() {
        return Err(CalError::NonFinite);
    }
    if !gt(smax, 0.0) || !gt(smin, RANK_TOL * smax) {
        return Err(CalError::Degenerate);
    }
    let n = a.cols;
    let solutions = rhs
        .iter()
        .map(|b| {
            let mut x = vec![0.0; n];
            for j in 0..n {
                // (W_j . b) / σ_j²  times V_j
                let wb: f64 = (0..a.rows).map(|r| svd.w.get(r, j) * b[r]).sum();
                let k = wb / (svd.sigma[j] * svd.sigma[j]);
                for (i, xi) in x.iter_mut().enumerate() {
                    *xi += k * svd.v.get(i, j);
                }
            }
            x
        })
        .collect();
    Ok((solutions, smax / smin))
}
