//! owner: WP-R.8
//!
//! MediaPipe Face Landmarker (Android front camera) to the ARKit-shaped per-frame inputs of
//! the gaze pipeline: `face_transform` (face pose in the camera frame) and `look_at` (gaze
//! point in the face frame), plus both eye transforms. Pure maths, no I/O; the log-level
//! merge of the spike app's sidecar lives in [`super::android_log`].
//!
//! # Frames and units
//! - **MediaPipe metric frame `M`** (input `face_matrix`): the OpenGL convention of MediaPipe's
//!   face-geometry module: a virtual perspective camera at the origin looking down `-z`,
//!   `+x` to the right of the image, `+y` up the image, centimetres. The facial
//!   transformation matrix maps the canonical face model (`+x` towards the image right for a
//!   frontal face, `+y` up, `+z` out of the face towards the camera) into `M`. The virtual
//!   camera has a vertical field of view of [`MEDIAPIPE_VERTICAL_FOV_DEG`] over the image
//!   height and its principal point at the image centre; MediaPipe derives depth from the
//!   size of an average adult face under that camera.
//! - **Image**: the upright (rotated, **unmirrored**) analysis image the spike app feeds to
//!   MediaPipe, pixels, origin top-left, `y` down. Landmarks are normalised to it (`x`, `y`
//!   in [0,1]); landmark `z` is MediaPipe's relative depth, origin at the head centre,
//!   negative towards the camera, in units of the image **width**.
//! - **Pipeline camera frame `P`** (output): the frame of [`super::raycast::CAMERA_AXES`]:
//!   metres, device-fixed, portrait native, `+x` towards the device's right edge, `+y`
//!   towards the camera edge, the viewer at `z < 0`, the screen plane at `z = 0`.
//!   An unmirrored front-camera image shows the user's right on the image's left, so with
//!   the default axes `P = S · M` with `S = diag(-1, 1, 1)` (and cm to m). `S` is built from
//!   `CAMERA_AXES`, so a sign flip there after the bench corners check carries over.
//! - **Face frame `F`** (output, the frame of `look_at` and the eye transforms):
//!   `F = S · canonical`, so for a frontal face looking at the screen the face transform is
//!   the identity rotation and `look_at` has `z > 0`, exactly like the synthetic ARKit logs.
//!   The face rotation in `P` is `R_P = S · R_M · S` (a proper rotation). Eye transforms are
//!   named after the user's eyes (left = MediaPipe mesh 263/362, the user's left eye) and
//!   carry only a translation; the pipeline uses only their midpoint.
//!
//! # Geometry model (iris-centre ray with an eyeball-centre estimate)
//! 1. **Head pose.** `R_M` and `t_M` from the facial transformation matrix (column-major as
//!    MediaPipe returns it; a row-major copy is recognised and transposed; the rotation is
//!    re-orthonormalised). The depth is rescaled to the real camera: the face origin
//!    projects to the same pixel under MediaPipe's virtual camera and the real one, and its
//!    apparent size fixes `Z_true = Z_mp · f_true / f_mp` with `f = √(fx·fy)`; `X`, `Y`
//!    then follow from the real principal point. Without intrinsics the virtual camera is
//!    used as is (and distances are only as good as MediaPipe's 63-degree guess).
//! 2. **Eye corners in 3D.** Each corner pixel is back-projected with the real intrinsics at
//!    depth `Z_head + z · W · Z_head / fx` (MediaPipe's relative depth turned metric at the
//!    head's depth).
//! 3. **Eyeball centre.** `E = (outer + inner) / 2 − d · forward`, `d =`
//!    [`EyeModel::centre_behind_canthi_m`], `forward` the face's `+z` axis.
//! 4. **Iris centre.** The mean of the five iris landmarks of that eye (centre + ring),
//!    back-projected as a ray from the camera and intersected with the sphere of radius
//!    [`EyeModel::iris_radius_m`] around `E` (the near intersection, the visible side). A ray
//!    that misses by up to `max_miss_fraction · r` is taken at its closest approach; further
//!    off, the eye is rejected (`irisOffEyeball`).
//! 5. **Per-eye gaze** `g = normalize(I − E)`, rejected beyond
//!    [`EyeModel::max_eye_in_head_deg`] from `forward`.
//! 6. **Binocular point.** Each valid eye's ray is intersected with the screen plane
//!    (`z = 0` in `P`); `look_at` is the mean of the hits, expressed in `F`. The pipeline's
//!    ray from the eye midpoint through `look_at` then lands exactly on that mean, which
//!    handles vergence and a single open eye correctly (a direction average from the
//!    midpoint would be off by half the interocular distance with one eye). When no eye
//!    reaches the plane, `look_at` falls back to `midpoint + 0.5 m · mean direction` and the
//!    pipeline classifies the ray (`backward`/`grazing`).
//!
//! # Accuracy limits (what calibration can and cannot fix)
//! - The model's anatomical constants (radius, centre depth) and MediaPipe's face-size prior
//!   act as a per-user gain and offset; the CAL-4 affine calibration absorbs them.
//! - The hard limit is iris localisation: at 50 cm with `f ≈ 500 px` (640 x 480 analysis
//!   image) one pixel of iris-centre error is about 1 mm, which is about 5.5 degrees of eye
//!   rotation on a 10.4 mm radius. Averaging over a target window helps only with jitter,
//!   not with a bias that changes with head pose. A larger analysis image scales this down
//!   proportionally. See the unit test `iris_pixel_sensitivity_matches_docs`.
//! - Corner depth comes from MediaPipe's relative `z`, which is a weak-perspective estimate.
//! - Blend-shape blink scores pass through unchanged; per-eye exclusion here uses the
//!   eyelid aperture from landmarks (mesh 159/145 and 386/374), which is unambiguous about
//!   which eye is which.
//! - Assumed, to be confirmed on a device (protocol section 5): column-major matrix,
//!   centimetres, 63-degree vertical FOV, `origin_point_location = TOP_LEFT_CORNER` (image
//!   up = `+y`), unmirrored image.

use serde::{Deserialize, Serialize};

use super::raycast::CAMERA_AXES;

/// Vertical field of view of MediaPipe's virtual camera (face-geometry environment of the
/// Face Landmarker task), degrees over the image height.
pub const MEDIAPIPE_VERTICAL_FOV_DEG: f64 = 63.0;

/// Length of the fallback `look_at` vector (no eye reaches the screen plane), metres.
pub const FALLBACK_LOOK_AT_M: f64 = 0.5;

/// Furthest accepted per-eye screen-plane hit along the gaze ray, metres.
pub const MAX_SCREEN_HIT_M: f64 = 3.0;

/// Eyelid aperture / eye width of a typical open eye; closedness is `1 − ratio / this`.
pub const OPEN_APERTURE_RATIO: f64 = 0.28;

type V3 = [f64; 3];
type M3 = [[f64; 3]; 3];

/// Pinhole intrinsics in pixels of the **upright** analysis image (origin top-left, y down).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CameraIntrinsics {
    pub fx: f64,
    pub fy: f64,
    pub cx: f64,
    pub cy: f64,
}

impl CameraIntrinsics {
    /// MediaPipe's own virtual camera for an upright image of `image = [w, h]` pixels.
    pub fn mediapipe_virtual(image: [f64; 2]) -> Self {
        let f = (image[1] / 2.0) / (MEDIAPIPE_VERTICAL_FOV_DEG.to_radians() / 2.0).tan();
        CameraIntrinsics {
            fx: f,
            fy: f,
            cx: image[0] / 2.0,
            cy: image[1] / 2.0,
        }
    }

    /// Finite and with positive focal lengths.
    pub fn is_valid(&self) -> bool {
        [self.fx, self.fy, self.cx, self.cy]
            .iter()
            .all(|v| v.is_finite())
            && self.fx > 0.0
            && self.fy > 0.0
    }

    /// Geometric-mean focal length (the isotropic scale of apparent size).
    pub fn f(&self) -> f64 {
        (self.fx * self.fy).sqrt()
    }

    /// The same camera for the image scaled from `from` to `to` pixels (uniform per axis).
    pub fn scaled(&self, from: [f64; 2], to: [f64; 2]) -> Self {
        let (sx, sy) = (to[0] / from[0], to[1] / from[1]);
        CameraIntrinsics {
            fx: self.fx * sx,
            fy: self.fy * sy,
            cx: self.cx * sx,
            cy: self.cy * sy,
        }
    }

    /// Camera2 `LENS_INTRINSIC_CALIBRATION` (`[fx, fy, cx, cy]` in sensor active-array
    /// pixels; the skew is ignored) mapped to the upright analysis image.
    ///
    /// - `active_array = [left, top, right, bottom]` (`SENSOR_INFO_ACTIVE_ARRAY_SIZE`); the
    ///   calibration is taken relative to its top-left (Camera2 defines it on the
    ///   pre-correction array, which is the same rectangle or very close on phones).
    /// - `analysis = [w, h]` is the sensor-oriented analysis image (CameraX output before the
    ///   app rotates it), assumed to be a centred, uniformly scaled crop of the active array
    ///   that fills the image.
    /// - `rotation_deg` is the clockwise rotation the app applied to make it upright
    ///   (`ImageInfo.rotationDegrees`: 0, 90, 180 or 270).
    pub fn from_android_sensor(
        k: [f64; 4],
        active_array: [f64; 4],
        analysis: [f64; 2],
        rotation_deg: i64,
    ) -> Option<Self> {
        let aw = active_array[2] - active_array[0];
        let ah = active_array[3] - active_array[1];
        if !(aw > 0.0 && ah > 0.0 && analysis[0] > 0.0 && analysis[1] > 0.0) {
            return None;
        }
        let s = (analysis[0] / aw).max(analysis[1] / ah);
        let sensor = CameraIntrinsics {
            fx: k[0] * s,
            fy: k[1] * s,
            cx: (k[2] - active_array[0] - aw / 2.0) * s + analysis[0] / 2.0,
            cy: (k[3] - active_array[1] - ah / 2.0) * s + analysis[1] / 2.0,
        };
        let out = sensor.rotated_cw(analysis, rotation_deg)?;
        out.is_valid().then_some(out)
    }

    /// Fallback from the nominal focal length: `focal_mm` over a sensor of `sensor_mm`
    /// (`SENSOR_INFO_PHYSICAL_SIZE`) with `pixel_array` pixels, principal point at the
    /// centre; then as [`Self::from_android_sensor`].
    pub fn from_android_focal_length(
        focal_mm: f64,
        sensor_mm: [f64; 2],
        pixel_array: [f64; 2],
        analysis: [f64; 2],
        rotation_deg: i64,
    ) -> Option<Self> {
        if !(focal_mm > 0.0 && sensor_mm[0] > 0.0 && sensor_mm[1] > 0.0) {
            return None;
        }
        let fx = focal_mm / sensor_mm[0] * pixel_array[0];
        let fy = focal_mm / sensor_mm[1] * pixel_array[1];
        Self::from_android_sensor(
            [fx, fy, pixel_array[0] / 2.0, pixel_array[1] / 2.0],
            [0.0, 0.0, pixel_array[0], pixel_array[1]],
            analysis,
            rotation_deg,
        )
    }

    /// The intrinsics after rotating a `size = [w, h]` image clockwise by `deg`.
    fn rotated_cw(&self, size: [f64; 2], deg: i64) -> Option<Self> {
        let [w, h] = size;
        Some(match deg.rem_euclid(360) {
            0 => *self,
            // (x, y) -> (h - y, x)
            90 => CameraIntrinsics {
                fx: self.fy,
                fy: self.fx,
                cx: h - self.cy,
                cy: self.cx,
            },
            180 => CameraIntrinsics {
                fx: self.fx,
                fy: self.fy,
                cx: w - self.cx,
                cy: h - self.cy,
            },
            // (x, y) -> (y, w - x)
            270 => CameraIntrinsics {
                fx: self.fy,
                fy: self.fx,
                cx: self.cy,
                cy: w - self.cx,
            },
            _ => return None,
        })
    }
}

/// Anatomical constants and rejection limits of the eye model (module docs).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct EyeModel {
    /// Eyeball centre to iris centre (12 mm eyeball, 6 mm limbus radius: √(12² − 6²)).
    pub iris_radius_m: f64,
    /// Eyeball centre behind the midpoint of the two eye corners, along the face's forward
    /// axis.
    pub centre_behind_canthi_m: f64,
    /// An eye with eyelid aperture / eye width below this is closed (no gaze from it).
    pub closed_aperture_ratio: f64,
    /// Head yaw beyond which the frame carries no gaze (MediaPipe's iris degrades).
    pub max_head_yaw_deg: f64,
    pub max_head_pitch_deg: f64,
    /// Eye-in-head rotation beyond which an eye is rejected.
    pub max_eye_in_head_deg: f64,
    /// A camera ray that misses the eyeball by up to this fraction of the radius is taken
    /// at its closest approach.
    pub max_miss_fraction: f64,
}

impl Default for EyeModel {
    fn default() -> Self {
        EyeModel {
            iris_radius_m: 0.0104,
            centre_behind_canthi_m: 0.008,
            closed_aperture_ratio: 0.12,
            max_head_yaw_deg: 45.0,
            max_head_pitch_deg: 35.0,
            max_eye_in_head_deg: 50.0,
            max_miss_fraction: 0.15,
        }
    }
}

/// One landmark as MediaPipe returns it (see module docs for the units).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Landmark {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Landmark {
    fn finite(&self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }
}

/// The four eye-contour landmarks the model uses (MediaPipe canonical mesh indices:
/// user's right eye 33 / 133 / 159 / 145, user's left eye 263 / 362 / 386 / 374).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EyeLandmarks {
    pub outer: Landmark,
    pub inner: Landmark,
    pub upper: Landmark,
    pub lower: Landmark,
}

/// One MediaPipe Face Landmarker result for one face.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MediaPipeFrame {
    /// Upright analysis image `[w, h]`, pixels.
    pub image_size: [f64; 2],
    /// The user's right eye (mesh 33, 133, 159, 145).
    pub right_eye: EyeLandmarks,
    /// The user's left eye (mesh 263, 362, 386, 374).
    pub left_eye: EyeLandmarks,
    /// The two iris groups (mesh 468..472 and 473..477, centre first), or `None` when the
    /// model ran without iris refinement. Each group is assigned to the eye whose corners it
    /// lies between, so the order does not matter.
    pub irises: Option<[[Landmark; 5]; 2]>,
    /// The facial transformation matrix, 16 values as MediaPipe returns them.
    pub face_matrix: [f64; 16],
}

/// Why a frame has no face transform (`Err`) or no `look_at` (`ArkitInputs::issue`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FrameIssue {
    /// MediaPipe found no face.
    NoFace,
    /// A required landmark is missing or not finite, or the image size is unusable.
    IncompleteLandmarks,
    /// The facial transformation matrix is missing, not finite, not a rotation, or puts the
    /// face behind the camera.
    BadFaceMatrix,
    BadIntrinsics,
    /// Landmarks without the 10 iris points.
    MissingIris,
    /// Both eyes closed by the eyelid-aperture test.
    EyesClosed,
    /// Head yaw or pitch beyond the model's limits.
    ExtremeHeadPose,
    /// Every open eye's iris ray missed its eyeball or turned too far.
    IrisOffEyeball,
}

impl FrameIssue {
    /// The camelCase wire name.
    pub fn as_str(self) -> &'static str {
        match self {
            FrameIssue::NoFace => "noFace",
            FrameIssue::IncompleteLandmarks => "incompleteLandmarks",
            FrameIssue::BadFaceMatrix => "badFaceMatrix",
            FrameIssue::BadIntrinsics => "badIntrinsics",
            FrameIssue::MissingIris => "missingIris",
            FrameIssue::EyesClosed => "eyesClosed",
            FrameIssue::ExtremeHeadPose => "extremeHeadPose",
            FrameIssue::IrisOffEyeball => "irisOffEyeball",
        }
    }
}

/// The ARKit-equivalent per-frame values (module docs for the frames).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ArkitInputs {
    /// Face pose in the pipeline camera frame, column-major, metres.
    pub face_transform: [f64; 16],
    /// Gaze point in the face frame, metres; `None` with an `issue`.
    pub look_at: Option<[f64; 3]>,
    /// Translation-only transforms of the user's eyeball centres in the face frame.
    pub left_eye_transform: [f64; 16],
    pub right_eye_transform: [f64; 16],
    /// Per-eye unit gaze direction in the face frame (`None` when that eye was rejected).
    pub left_gaze: Option<[f64; 3]>,
    pub right_gaze: Option<[f64; 3]>,
    /// Landmark closedness per eye, 0 open .. 1 closed (`1 − aperture / OPEN_APERTURE_RATIO`).
    pub left_closedness: f64,
    pub right_closedness: f64,
    /// Head yaw and pitch with the pipeline's formulas, degrees.
    pub head_yaw_deg: f64,
    pub head_pitch_deg: f64,
    /// Eye-midpoint distance in front of the screen plane, metres.
    pub distance_m: f64,
    /// Distance between the two eyeball centres, metres (sanity check: adults ~ 0.06).
    pub interocular_m: f64,
    pub issue: Option<FrameIssue>,
}

// ------------------------------------------------------------------------------------------
// Small linear algebra
// ------------------------------------------------------------------------------------------

fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn scale(a: V3, s: f64) -> V3 {
    [a[0] * s, a[1] * s, a[2] * s]
}
fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn norm(a: V3) -> f64 {
    dot(a, a).sqrt()
}
fn unit(a: V3) -> Option<V3> {
    let n = norm(a);
    (n > 1e-12 && n.is_finite()).then(|| scale(a, 1.0 / n))
}
fn mul_mv(m: &M3, v: V3) -> V3 {
    [dot(m[0], v), dot(m[1], v), dot(m[2], v)]
}
fn transpose(m: &M3) -> M3 {
    let mut t = [[0.0; 3]; 3];
    for (r, row) in m.iter().enumerate() {
        for (c, v) in row.iter().enumerate() {
            t[c][r] = *v;
        }
    }
    t
}
fn angle_between_deg(a: V3, b: V3) -> f64 {
    norm(cross(a, b)).atan2(dot(a, b)).to_degrees()
}

/// Signs of `S` (`P = S · M`, module docs), from the pipeline's axis convention.
fn s_signs() -> V3 {
    [
        -CAMERA_AXES.x_to_native_right,
        CAMERA_AXES.y_to_native_up,
        -CAMERA_AXES.z_viewer_side,
    ]
}
fn apply_s(v: V3) -> V3 {
    let s = s_signs();
    [s[0] * v[0], s[1] * v[1], s[2] * v[2]]
}
/// `S · R · S` (S diagonal and its own inverse).
fn conj_s(r: &M3) -> M3 {
    let s = s_signs();
    let mut out = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            out[i][j] = s[i] * s[j] * r[i][j];
        }
    }
    out
}

/// Column-major 4x4 from a rotation (rows) and a translation.
fn to_m4(r: &M3, t: V3) -> [f64; 16] {
    let mut m = [0.0; 16];
    for (row, values) in r.iter().enumerate() {
        for (c, v) in values.iter().enumerate() {
            m[c * 4 + row] = *v;
        }
    }
    m[12] = t[0];
    m[13] = t[1];
    m[14] = t[2];
    m[15] = 1.0;
    m
}

fn translation_m4(t: V3) -> [f64; 16] {
    to_m4(&[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]], t)
}

/// Reads MediaPipe's facial transformation matrix: rotation rows and translation in cm.
/// Column-major (translation at 12, 13, 14) is MediaPipe's layout; a matrix whose
/// translation sits at 3, 7, 11 with a zero row at 12..14 is read as row-major. The
/// rotation is re-orthonormalised (Gram-Schmidt on its first two columns); a scale outside
/// [0.5, 2] or a reflection is rejected.
fn read_face_matrix(m: &[f64; 16]) -> Option<(M3, V3)> {
    if !m.iter().all(|v| v.is_finite()) {
        return None;
    }
    let small = |v: f64| v.abs() < 1e-6;
    let col_major = small(m[3]) && small(m[7]) && small(m[11]);
    let row_major = !col_major && small(m[12]) && small(m[13]) && small(m[14]);
    let at = |r: usize, c: usize| {
        if row_major {
            m[r * 4 + c]
        } else {
            m[c * 4 + r]
        }
    };
    if !(col_major || row_major) || (at(3, 3) - 1.0).abs() > 1e-3 {
        return None;
    }
    let col = |c: usize| [at(0, c), at(1, c), at(2, c)];
    let (c0, c1) = (col(0), col(1));
    let (n0, n1) = (norm(c0), norm(c1));
    if !(0.5..=2.0).contains(&n0) || !(0.5..=2.0).contains(&n1) {
        return None;
    }
    let x = unit(c0)?;
    let y = unit(sub(c1, scale(x, dot(c1, x))))?;
    let z = cross(x, y);
    if dot(z, col(2)) <= 0.0 {
        return None;
    }
    let r = [[x[0], y[0], z[0]], [x[1], y[1], z[1]], [x[2], y[2], z[2]]];
    Some((r, [at(0, 3), at(1, 3), at(2, 3)]))
}

/// Pixel of a normalised landmark.
fn pixel(l: &Landmark, image: [f64; 2]) -> [f64; 2] {
    [l.x * image[0], l.y * image[1]]
}

/// Unit ray (in `M`) through pixel `p`.
fn pixel_ray(k: &CameraIntrinsics, p: [f64; 2]) -> V3 {
    let d = [(p[0] - k.cx) / k.fx, -(p[1] - k.cy) / k.fy, -1.0];
    scale(d, 1.0 / norm(d))
}

/// Point in `M` (metres) at positive depth `z_depth` along pixel `p`.
fn back_project(k: &CameraIntrinsics, p: [f64; 2], z_depth: f64) -> V3 {
    [
        (p[0] - k.cx) / k.fx * z_depth,
        -(p[1] - k.cy) / k.fy * z_depth,
        -z_depth,
    ]
}

struct EyeGeom {
    centre_m: V3,
    corner_mid_px: [f64; 2],
    closedness: f64,
    closed: bool,
}

fn eye_geometry(
    eye: &EyeLandmarks,
    image: [f64; 2],
    k: &CameraIntrinsics,
    z_head: f64,
    forward_m: V3,
    model: &EyeModel,
) -> Option<EyeGeom> {
    let pts = [&eye.outer, &eye.inner, &eye.upper, &eye.lower];
    if !pts.iter().all(|l| l.finite()) {
        return None;
    }
    let depth = |l: &Landmark| z_head + l.z * image[0] * z_head / k.fx;
    let (po, pi) = (pixel(&eye.outer, image), pixel(&eye.inner, image));
    let (pu, pl) = (pixel(&eye.upper, image), pixel(&eye.lower, image));
    let width = ((po[0] - pi[0]).powi(2) + (po[1] - pi[1]).powi(2)).sqrt();
    let aperture = ((pu[0] - pl[0]).powi(2) + (pu[1] - pl[1]).powi(2)).sqrt();
    if width <= 0.0 {
        return None;
    }
    let ratio = aperture / width;
    let (zo, zi) = (depth(&eye.outer), depth(&eye.inner));
    if !(zo > 0.0 && zi > 0.0) {
        return None;
    }
    let mid = scale(add(back_project(k, po, zo), back_project(k, pi, zi)), 0.5);
    Some(EyeGeom {
        centre_m: sub(mid, scale(forward_m, model.centre_behind_canthi_m)),
        corner_mid_px: [(po[0] + pi[0]) / 2.0, (po[1] + pi[1]) / 2.0],
        closedness: (1.0 - ratio / OPEN_APERTURE_RATIO).clamp(0.0, 1.0),
        closed: ratio < model.closed_aperture_ratio,
    })
}

/// Gaze direction (in `M`) of one eye from its iris group, or `None` when rejected.
fn iris_gaze(
    iris: &[Landmark; 5],
    eye: &EyeGeom,
    image: [f64; 2],
    k: &CameraIntrinsics,
    forward_m: V3,
    model: &EyeModel,
) -> Option<V3> {
    if !iris.iter().all(|l| l.finite()) {
        return None;
    }
    let mut px = [0.0, 0.0];
    for l in iris {
        let p = pixel(l, image);
        px[0] += p[0] / 5.0;
        px[1] += p[1] / 5.0;
    }
    let d = pixel_ray(k, px);
    let e = eye.centre_m;
    let r = model.iris_radius_m;
    let b = dot(d, e);
    let disc = b * b - dot(e, e) + r * r;
    let lambda = if disc >= 0.0 {
        b - disc.sqrt()
    } else {
        let miss = (dot(e, e) - b * b).max(0.0).sqrt();
        if miss > r * (1.0 + model.max_miss_fraction) {
            return None;
        }
        b
    };
    if !lambda.is_finite() || lambda <= 0.0 {
        return None;
    }
    let g = unit(sub(scale(d, lambda), e))?;
    (angle_between_deg(g, forward_m) <= model.max_eye_in_head_deg).then_some(g)
}

/// Converts one MediaPipe frame (module docs). `intrinsics` are those of the upright image
/// (`None`: MediaPipe's virtual camera, so metric depth is MediaPipe's guess).
///
/// `Err` when no face transform can be produced; `Ok` with `look_at = None` and an `issue`
/// when the face pose is usable but the gaze is not (missing iris, eyes closed, extreme
/// pose, iris off the eyeball). The pipeline then classifies the frame (`blink` from the
/// blend shapes, else `artifact`, TRK-1).
pub fn convert_frame(
    frame: &MediaPipeFrame,
    intrinsics: Option<&CameraIntrinsics>,
    model: &EyeModel,
) -> Result<ArkitInputs, FrameIssue> {
    let image = frame.image_size;
    if !(image[0] > 0.0 && image[1] > 0.0 && image[0].is_finite() && image[1].is_finite()) {
        return Err(FrameIssue::IncompleteLandmarks);
    }
    let virt = CameraIntrinsics::mediapipe_virtual(image);
    let k = match intrinsics {
        Some(k) if k.is_valid() => *k,
        Some(_) => return Err(FrameIssue::BadIntrinsics),
        None => virt,
    };
    let (r_m, t_cm) = read_face_matrix(&frame.face_matrix).ok_or(FrameIssue::BadFaceMatrix)?;
    let t_mp = scale(t_cm, 0.01);
    // Finite (checked by `read_face_matrix`); the face must be in front of the camera.
    if t_mp[2] >= 0.0 {
        return Err(FrameIssue::BadFaceMatrix);
    }

    // 1. Depth correction: same pixel, apparent size rescaled to the real focal length.
    let z_mp = -t_mp[2];
    let u0 = [
        virt.cx + virt.fx * t_mp[0] / z_mp,
        virt.cy - virt.fy * t_mp[1] / z_mp,
    ];
    let z_head = z_mp * k.f() / virt.f();
    let t_m = back_project(&k, u0, z_head);
    let forward_m = [r_m[0][2], r_m[1][2], r_m[2][2]];

    // Output pose in P.
    let r_p = conj_s(&r_m);
    let t_p = apply_s(t_m);
    let face_transform = to_m4(&r_p, t_p);
    let r_p_t = transpose(&r_p);
    let to_face = |p_m: V3| mul_mv(&r_p_t, sub(apply_s(p_m), t_p));
    let dir_to_face = |d_m: V3| mul_mv(&r_p_t, apply_s(d_m));
    let head_pitch_deg = (-face_transform[9]).clamp(-1.0, 1.0).asin().to_degrees();
    let head_yaw_deg = face_transform[8].atan2(face_transform[10]).to_degrees();

    // 2-3. Eyeball centres.
    let right = eye_geometry(&frame.right_eye, image, &k, z_head, forward_m, model)
        .ok_or(FrameIssue::IncompleteLandmarks)?;
    let left = eye_geometry(&frame.left_eye, image, &k, z_head, forward_m, model)
        .ok_or(FrameIssue::IncompleteLandmarks)?;
    let mid_m = scale(add(right.centre_m, left.centre_m), 0.5);
    let mid_p = apply_s(mid_m);
    let distance_m = CAMERA_AXES.z_viewer_side * mid_p[2];

    let mut out = ArkitInputs {
        face_transform,
        look_at: None,
        left_eye_transform: translation_m4(to_face(left.centre_m)),
        right_eye_transform: translation_m4(to_face(right.centre_m)),
        left_gaze: None,
        right_gaze: None,
        left_closedness: left.closedness,
        right_closedness: right.closedness,
        head_yaw_deg,
        head_pitch_deg,
        distance_m,
        interocular_m: norm(sub(left.centre_m, right.centre_m)),
        issue: None,
    };

    let Some(irises) = frame.irises else {
        out.issue = Some(FrameIssue::MissingIris);
        return Ok(out);
    };
    if head_yaw_deg.abs() > model.max_head_yaw_deg
        || head_pitch_deg.abs() > model.max_head_pitch_deg
    {
        out.issue = Some(FrameIssue::ExtremeHeadPose);
        return Ok(out);
    }
    if right.closed && left.closed {
        out.issue = Some(FrameIssue::EyesClosed);
        return Ok(out);
    }

    // 4-5. Assign iris groups to eyes by image proximity, then intersect.
    let centre_px = |g: &[Landmark; 5]| pixel(&g[0], image);
    let dist2 = |a: [f64; 2], b: [f64; 2]| (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2);
    let (a, b) = (centre_px(&irises[0]), centre_px(&irises[1]));
    let straight = dist2(a, right.corner_mid_px) + dist2(b, left.corner_mid_px);
    let swapped = dist2(a, left.corner_mid_px) + dist2(b, right.corner_mid_px);
    let (iris_r, iris_l) = if straight <= swapped {
        (&irises[0], &irises[1])
    } else {
        (&irises[1], &irises[0])
    };
    let gaze = |eye: &EyeGeom, iris: &[Landmark; 5]| {
        if eye.closed {
            None
        } else {
            iris_gaze(iris, eye, image, &k, forward_m, model)
        }
    };
    let g_r = gaze(&right, iris_r);
    let g_l = gaze(&left, iris_l);
    out.right_gaze = g_r.map(dir_to_face);
    out.left_gaze = g_l.map(dir_to_face);

    // 6. Binocular point on the screen plane (z = 0 in P).
    let towards_sign = -CAMERA_AXES.z_viewer_side;
    let screen_hit = |centre_m: V3, g_m: V3| -> Option<V3> {
        let (e, d) = (apply_s(centre_m), apply_s(g_m));
        let towards = towards_sign * d[2];
        if towards <= 1e-6 {
            return None;
        }
        let lambda = -e[2] / d[2];
        (lambda > 0.0 && lambda <= MAX_SCREEN_HIT_M).then(|| add(e, scale(d, lambda)))
    };
    let valid: Vec<(V3, V3)> = [(right.centre_m, g_r), (left.centre_m, g_l)]
        .into_iter()
        .filter_map(|(c, g)| g.map(|g| (c, g)))
        .collect();
    if valid.is_empty() {
        out.issue = Some(FrameIssue::IrisOffEyeball);
        return Ok(out);
    }
    let hits: Vec<V3> = valid
        .iter()
        .filter_map(|(c, g)| screen_hit(*c, *g))
        .collect();
    let look_at = if hits.len() == valid.len() {
        let n = hits.len() as f64;
        let mean = hits
            .iter()
            .fold([0.0; 3], |acc, h| add(acc, scale(*h, 1.0 / n)));
        mul_mv(&r_p_t, sub(mean, t_p))
    } else {
        let mean_dir = valid.iter().fold([0.0; 3], |acc, (_, g)| add(acc, *g));
        let d = unit(mean_dir).ok_or(FrameIssue::IrisOffEyeball)?;
        add(to_face(mid_m), scale(dir_to_face(d), FALLBACK_LOOK_AT_M))
    };
    if !look_at.iter().all(|v| v.is_finite()) {
        out.issue = Some(FrameIssue::IrisOffEyeball);
        return Ok(out);
    }
    out.look_at = Some(look_at);
    Ok(out)
}

// ------------------------------------------------------------------------------------------
// Forward model (synthetic frames for tests and fixtures)
// ------------------------------------------------------------------------------------------

/// The inverse of [`convert_frame`]: what MediaPipe would report for a known head pose and
/// gaze under the module's model. Used by the unit tests and by the committed
/// `asd-cli gaze convert-android` fixture; never by product code.
#[cfg(any(test, feature = "synth"))]
pub mod synth {
    use super::*;

    /// A known head pose and eye geometry, in the pipeline frames.
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct SynthFace {
        /// Rotation `Ry(yaw) · Rx(pitch) · Rz(roll)` of the face frame in `P`, degrees.
        pub yaw_deg: f64,
        pub pitch_deg: f64,
        pub roll_deg: f64,
        /// Face origin (MediaPipe's head centre) in `P`, metres.
        pub position_m: [f64; 3],
        /// Eyeball centres `[right, left]` in the face frame `F`, metres.
        pub eye_centres_face_m: [[f64; 3]; 2],
        /// Half the eye-corner distance, metres.
        pub eye_half_width_m: f64,
        /// Half the eyelid aperture, metres (about 0.004 open, 0.0005 closed).
        pub lid_half_open_m: [f64; 2],
        /// Iris ring radius, pixels.
        pub iris_ring_px: f64,
    }

    impl SynthFace {
        /// An adult face with its origin at `position_m`, frontal.
        pub fn adult(position_m: [f64; 3]) -> Self {
            SynthFace {
                yaw_deg: 0.0,
                pitch_deg: 0.0,
                roll_deg: 0.0,
                position_m,
                eye_centres_face_m: [[0.031, 0.03, 0.03], [-0.031, 0.03, 0.03]],
                eye_half_width_m: 0.0135,
                lid_half_open_m: [0.004, 0.004],
                iris_ring_px: 5.0,
            }
        }

        /// Face rotation in `P` (rows).
        pub fn rotation(&self) -> [[f64; 3]; 3] {
            let (sy, cy) = self.yaw_deg.to_radians().sin_cos();
            let (sp, cp) = self.pitch_deg.to_radians().sin_cos();
            let (sr, cr) = self.roll_deg.to_radians().sin_cos();
            let ry = [[cy, 0.0, sy], [0.0, 1.0, 0.0], [-sy, 0.0, cy]];
            let rx = [[1.0, 0.0, 0.0], [0.0, cp, -sp], [0.0, sp, cp]];
            let rz = [[cr, -sr, 0.0], [sr, cr, 0.0], [0.0, 0.0, 1.0]];
            mul_mm(&mul_mm(&ry, &rx), &rz)
        }

        /// A face-frame point in `P`.
        pub fn to_p(&self, f: [f64; 3]) -> [f64; 3] {
            add(mul_mv(&self.rotation(), f), self.position_m)
        }

        /// Eyeball centres `[right, left]` in `P`.
        pub fn eye_centres_p(&self) -> [[f64; 3]; 2] {
            [
                self.to_p(self.eye_centres_face_m[0]),
                self.to_p(self.eye_centres_face_m[1]),
            ]
        }
    }

    fn mul_mm(a: &M3, b: &M3) -> M3 {
        let mut out = [[0.0; 3]; 3];
        for i in 0..3 {
            for j in 0..3 {
                out[i][j] = (0..3).map(|k| a[i][k] * b[k][j]).sum();
            }
        }
        out
    }

    fn project(k: &CameraIntrinsics, image: [f64; 2], p_m: V3, z_head: f64) -> Landmark {
        let depth = -p_m[2];
        let u = k.cx + k.fx * p_m[0] / depth;
        let v = k.cy - k.fy * p_m[1] / depth;
        Landmark {
            x: u / image[0],
            y: v / image[1],
            z: (depth - z_head) * k.fx / (image[0] * z_head),
        }
    }

    /// The frame MediaPipe would report for `face` fixating `target_p` (a point in `P`,
    /// usually on the screen plane), seen by a camera with `intrinsics` on an upright image
    /// of `image` pixels. `iris = false` drops the iris groups.
    pub fn frame(
        face: &SynthFace,
        target_p: [f64; 3],
        intrinsics: &CameraIntrinsics,
        image: [f64; 2],
        model: &EyeModel,
        iris: bool,
    ) -> MediaPipeFrame {
        let r_p = face.rotation();
        let r_m = conj_s(&r_p);
        let t_true_m = apply_s(face.position_m);
        let z_head = -t_true_m[2];
        let k = intrinsics;

        // MediaPipe's matrix: same pixel, depth scaled to the virtual camera.
        let virt = CameraIntrinsics::mediapipe_virtual(image);
        let u0 = [
            k.cx + k.fx * t_true_m[0] / z_head,
            k.cy - k.fy * t_true_m[1] / z_head,
        ];
        let z_mp = z_head * virt.f() / k.f();
        let t_mp = back_project(&virt, u0, z_mp);
        let face_matrix = to_m4(&r_m, scale(t_mp, 100.0));

        let to_m = |f: V3| apply_s(face.to_p(f));
        let eye = |i: usize| {
            let c = face.eye_centres_face_m[i];
            // Right eye (i = 0) sits at +x of F; its outer corner is further out.
            let out_sign = if i == 0 { 1.0 } else { -1.0 };
            let front = c[2] + model.centre_behind_canthi_m;
            let w = face.eye_half_width_m;
            let lid = face.lid_half_open_m[i];
            EyeLandmarks {
                outer: project(k, image, to_m([c[0] + out_sign * w, c[1], front]), z_head),
                inner: project(k, image, to_m([c[0] - out_sign * w, c[1], front]), z_head),
                upper: project(k, image, to_m([c[0], c[1] + lid, front]), z_head),
                lower: project(k, image, to_m([c[0], c[1] - lid, front]), z_head),
            }
        };
        let iris_group = |i: usize| {
            let e = face.to_p(face.eye_centres_face_m[i]);
            let g = unit(sub(target_p, e)).unwrap_or([0.0, 0.0, 1.0]);
            let centre = project(
                k,
                image,
                apply_s(add(e, scale(g, model.iris_radius_m))),
                z_head,
            );
            let (dx, dy) = (face.iris_ring_px / image[0], face.iris_ring_px / image[1]);
            let at = |ox: f64, oy: f64| Landmark {
                x: centre.x + ox,
                y: centre.y + oy,
                z: centre.z,
            };
            [centre, at(dx, 0.0), at(0.0, -dy), at(-dx, 0.0), at(0.0, dy)]
        };
        MediaPipeFrame {
            image_size: image,
            right_eye: eye(0),
            left_eye: eye(1),
            irises: iris.then(|| [iris_group(0), iris_group(1)]),
            face_matrix,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::synth::{frame, SynthFace};
    use super::*;
    use crate::gaze::geometry::{angle_deg, idiom_default};
    use crate::gaze::raycast::raycast_parts;
    use crate::types::tracking::{DeviceIdiom, Orientation, RayStatus};

    const IMAGE: [f64; 2] = [480.0, 640.0];

    /// A real-looking front camera: f = 500 px, principal point off centre.
    fn camera() -> CameraIntrinsics {
        CameraIntrinsics {
            fx: 505.0,
            fy: 498.0,
            cx: 236.0,
            cy: 327.0,
        }
    }

    fn face_at(distance_m: f64) -> SynthFace {
        SynthFace::adult([0.0, -0.07, -distance_m])
    }

    /// Screen-normalised point -> P (portrait phone idiom default).
    fn target_p(norm: [f64; 2]) -> ([f64; 3], crate::gaze::geometry::OrientedGeometry) {
        let geo = idiom_default(DeviceIdiom::Phone).rotate(Orientation::Portrait);
        let mm = geo.norm_to_mm(norm);
        let p = [
            (mm[0] - geo.camera_native_mm[0]) / 1000.0,
            -(mm[1] - geo.camera_native_mm[1]) / 1000.0,
            0.0,
        ];
        (p, geo)
    }

    /// Angular error of the pipeline raycast of `out` against the scripted target.
    fn raycast_error_deg(out: &ArkitInputs, norm: [f64; 2]) -> f64 {
        let (_, geo) = target_p(norm);
        let look_at = out.look_at.expect("look_at");
        let hit = raycast_parts(
            &out.face_transform,
            &look_at,
            Some(&out.left_eye_transform),
            Some(&out.right_eye_transform),
            &geo,
        );
        assert_eq!(hit.status, RayStatus::Ok);
        let p = geo.norm_to_mm(hit.point_norm.unwrap());
        let q = geo.norm_to_mm(norm);
        angle_deg(
            [p[0], p[1], 0.0],
            [q[0], q[1], 0.0],
            hit.eye_pos_mm.unwrap(),
        )
    }

    #[test]
    fn frontal_face_recovers_pose_and_gaze() {
        let model = EyeModel::default();
        let face = face_at(0.45);
        let (t, _) = target_p([0.5, 0.5]);
        let f = frame(&face, t, &camera(), IMAGE, &model, true);
        let out = convert_frame(&f, Some(&camera()), &model).unwrap();
        assert_eq!(out.issue, None);
        // Identity rotation, translation back in metres in P.
        let ft = out.face_transform;
        for (i, expect) in [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0]
            .iter()
            .enumerate()
        {
            assert!((ft[i] - expect).abs() < 1e-9, "face[{i}] = {}", ft[i]);
        }
        for (i, expect) in face.position_m.iter().enumerate() {
            assert!((ft[12 + i] - expect).abs() < 1e-9);
        }
        // The look-at point is in front of the face (z > 0 in F), like ARKit.
        assert!(out.look_at.unwrap()[2] > 0.3);
        // Eye transforms at the scripted centres; left is the user's left (-x in F).
        assert!((out.right_eye_transform[12] - 0.031).abs() < 1e-9);
        assert!((out.left_eye_transform[12] + 0.031).abs() < 1e-9);
        assert!((out.interocular_m - 0.062).abs() < 1e-9);
        assert!((out.distance_m - 0.42).abs() < 1e-9);
        assert!(raycast_error_deg(&out, [0.5, 0.5]) < 0.01);
        assert!(out.left_closedness < 0.1 && out.right_closedness < 0.1);
    }

    #[test]
    fn known_head_poses_and_targets_recovered() {
        let model = EyeModel::default();
        let targets = [[0.1, 0.1], [0.9, 0.1], [0.5, 0.5], [0.1, 0.9], [0.9, 0.9]];
        let poses = [
            (0.0, 0.0, 0.0),
            (20.0, 0.0, 0.0),
            (-20.0, 10.0, 0.0),
            (10.0, -15.0, 8.0),
            (-30.0, 20.0, -10.0),
        ];
        for (yaw, pitch, roll) in poses {
            for distance in [0.35, 0.5, 0.65] {
                let mut face = face_at(distance);
                face.yaw_deg = yaw;
                face.pitch_deg = pitch;
                face.roll_deg = roll;
                face.position_m[0] = 0.02;
                for norm in targets {
                    let (t, _) = target_p(norm);
                    let f = frame(&face, t, &camera(), IMAGE, &model, true);
                    let out = convert_frame(&f, Some(&camera()), &model).unwrap();
                    assert_eq!(out.issue, None, "pose {yaw},{pitch},{roll} target {norm:?}");
                    assert!((out.head_yaw_deg - yaw).abs() < 1e-6);
                    assert!((out.head_pitch_deg - pitch).abs() < 1e-6);
                    let err = raycast_error_deg(&out, norm);
                    assert!(
                        err < 0.01,
                        "pose {yaw},{pitch},{roll} d {distance} {norm:?}: {err}"
                    );
                    // Per-eye directions point from each eyeball centre to the target.
                    let truth = |i: usize| {
                        let e = face.eye_centres_p()[i];
                        let g = unit(sub(t, e)).unwrap();
                        mul_mv(&transpose(&face.rotation()), g)
                    };
                    assert!(angle_between_deg(out.right_gaze.unwrap(), truth(0)) < 1e-6);
                    assert!(angle_between_deg(out.left_gaze.unwrap(), truth(1)) < 1e-6);
                }
            }
        }
    }

    #[test]
    fn depth_uses_real_intrinsics_and_falls_back_to_mediapipe_virtual_camera() {
        let model = EyeModel::default();
        let face = face_at(0.5);
        let (t, _) = target_p([0.3, 0.6]);
        let f = frame(&face, t, &camera(), IMAGE, &model, true);
        let with = convert_frame(&f, Some(&camera()), &model).unwrap();
        assert!((with.face_transform[14] + 0.5).abs() < 1e-9);
        // Without intrinsics the distance is MediaPipe's (63-degree) guess: wrong by f ratio.
        let without = convert_frame(&f, None, &model).unwrap();
        let virt = CameraIntrinsics::mediapipe_virtual(IMAGE);
        let expect = -0.5 * virt.f() / camera().f();
        assert!((without.face_transform[14] - expect).abs() < 1e-9);
        assert!((without.face_transform[14] - with.face_transform[14]).abs() > 0.01);
        // Bad intrinsics are an error, not silently replaced.
        let bad = CameraIntrinsics {
            fx: 0.0,
            ..camera()
        };
        assert_eq!(
            convert_frame(&f, Some(&bad), &model),
            Err(FrameIssue::BadIntrinsics)
        );
    }

    #[test]
    fn row_major_matrix_reads_like_column_major() {
        let model = EyeModel::default();
        let mut face = face_at(0.45);
        face.yaw_deg = 12.0;
        face.pitch_deg = -7.0;
        let (t, _) = target_p([0.7, 0.2]);
        let f = frame(&face, t, &camera(), IMAGE, &model, true);
        let mut g = f;
        for r in 0..4 {
            for c in 0..4 {
                g.face_matrix[r * 4 + c] = f.face_matrix[c * 4 + r];
            }
        }
        let a = convert_frame(&f, Some(&camera()), &model).unwrap();
        let b = convert_frame(&g, Some(&camera()), &model).unwrap();
        assert_eq!(a.face_transform, b.face_transform);
        assert_eq!(a.look_at, b.look_at);
    }

    #[test]
    fn iris_groups_are_assigned_by_position_not_order() {
        let model = EyeModel::default();
        let face = face_at(0.45);
        let (t, _) = target_p([0.2, 0.8]);
        let f = frame(&face, t, &camera(), IMAGE, &model, true);
        let mut g = f;
        let ir = f.irises.unwrap();
        g.irises = Some([ir[1], ir[0]]);
        let a = convert_frame(&f, Some(&camera()), &model).unwrap();
        let b = convert_frame(&g, Some(&camera()), &model).unwrap();
        assert_eq!(a.look_at, b.look_at);
    }

    #[test]
    fn missing_iris_keeps_pose_without_gaze() {
        let model = EyeModel::default();
        let face = face_at(0.45);
        let (t, _) = target_p([0.5, 0.5]);
        let f = frame(&face, t, &camera(), IMAGE, &model, false);
        let out = convert_frame(&f, Some(&camera()), &model).unwrap();
        assert_eq!(out.issue, Some(FrameIssue::MissingIris));
        assert_eq!(out.look_at, None);
        assert!((out.face_transform[14] + 0.45).abs() < 1e-9);
    }

    #[test]
    fn blink_one_eye_uses_the_other_both_closed_has_no_gaze() {
        let model = EyeModel::default();
        let mut face = face_at(0.45);
        let norm = [0.8, 0.3];
        let (t, _) = target_p(norm);
        face.lid_half_open_m = [0.0005, 0.004];
        let f = frame(&face, t, &camera(), IMAGE, &model, true);
        let out = convert_frame(&f, Some(&camera()), &model).unwrap();
        assert_eq!(out.issue, None);
        assert_eq!(out.right_gaze, None);
        assert!(out.left_gaze.is_some());
        assert!(out.right_closedness > 0.8);
        // One eye's screen hit, cast from the eye midpoint, still lands on the target.
        assert!(raycast_error_deg(&out, norm) < 0.01);

        face.lid_half_open_m = [0.0005, 0.0005];
        let f = frame(&face, t, &camera(), IMAGE, &model, true);
        let out = convert_frame(&f, Some(&camera()), &model).unwrap();
        assert_eq!(out.issue, Some(FrameIssue::EyesClosed));
        assert_eq!(out.look_at, None);
        assert!(out.left_closedness > 0.8 && out.right_closedness > 0.8);
    }

    #[test]
    fn extreme_pose_is_invalid_for_gaze() {
        let model = EyeModel::default();
        let (t, _) = target_p([0.5, 0.5]);
        for (yaw, pitch) in [(60.0, 0.0), (-50.0, 0.0), (0.0, 40.0)] {
            let mut face = face_at(0.45);
            face.yaw_deg = yaw;
            face.pitch_deg = pitch;
            let f = frame(&face, t, &camera(), IMAGE, &model, true);
            let out = convert_frame(&f, Some(&camera()), &model).unwrap();
            assert_eq!(
                out.issue,
                Some(FrameIssue::ExtremeHeadPose),
                "{yaw},{pitch}"
            );
            assert_eq!(out.look_at, None);
        }
    }

    #[test]
    fn iris_far_off_the_eyeball_is_rejected() {
        let model = EyeModel::default();
        let face = face_at(0.45);
        let (t, _) = target_p([0.5, 0.5]);
        let mut f = frame(&face, t, &camera(), IMAGE, &model, true);
        // Move both irises 40 px sideways: well outside a 10 mm eyeball at 45 cm.
        if let Some(ir) = f.irises.as_mut() {
            for group in ir.iter_mut() {
                for l in group.iter_mut() {
                    l.x += 40.0 / IMAGE[0];
                }
            }
        }
        let out = convert_frame(&f, Some(&camera()), &model).unwrap();
        assert_eq!(out.issue, Some(FrameIssue::IrisOffEyeball));
        assert_eq!(out.look_at, None);
    }

    #[test]
    fn bad_inputs_are_errors() {
        let model = EyeModel::default();
        let face = face_at(0.45);
        let (t, _) = target_p([0.5, 0.5]);
        let f = frame(&face, t, &camera(), IMAGE, &model, true);

        let mut g = f;
        g.face_matrix[5] = f64::NAN;
        assert_eq!(
            convert_frame(&g, None, &model),
            Err(FrameIssue::BadFaceMatrix)
        );
        let mut g = f;
        g.face_matrix = [0.0; 16];
        assert_eq!(
            convert_frame(&g, None, &model),
            Err(FrameIssue::BadFaceMatrix)
        );
        // Face behind the camera.
        let mut g = f;
        g.face_matrix[14] = -g.face_matrix[14];
        assert_eq!(
            convert_frame(&g, None, &model),
            Err(FrameIssue::BadFaceMatrix)
        );
        // A reflection is not a rotation.
        let mut g = f;
        for i in 8..11 {
            g.face_matrix[i] = -g.face_matrix[i];
        }
        assert_eq!(
            convert_frame(&g, None, &model),
            Err(FrameIssue::BadFaceMatrix)
        );
        let mut g = f;
        g.right_eye.outer.x = f64::NAN;
        assert_eq!(
            convert_frame(&g, None, &model),
            Err(FrameIssue::IncompleteLandmarks)
        );
        let mut g = f;
        g.image_size = [0.0, 640.0];
        assert_eq!(
            convert_frame(&g, None, &model),
            Err(FrameIssue::IncompleteLandmarks)
        );
    }

    #[test]
    fn iris_pixel_sensitivity_matches_docs() {
        // One pixel of iris error at 50 cm with f ~ 500 px is ~ 1 mm ~ 5.5 degrees of eye
        // rotation (module docs, "Accuracy limits").
        let model = EyeModel::default();
        let face = face_at(0.5);
        let (t, _) = target_p([0.5, 0.5]);
        let f = frame(&face, t, &camera(), IMAGE, &model, true);
        let a = convert_frame(&f, Some(&camera()), &model).unwrap();
        let mut g = f;
        if let Some(ir) = g.irises.as_mut() {
            for group in ir.iter_mut() {
                for l in group.iter_mut() {
                    l.x += 1.0 / IMAGE[0];
                }
            }
        }
        let b = convert_frame(&g, Some(&camera()), &model).unwrap();
        let d = angle_between_deg(a.right_gaze.unwrap(), b.right_gaze.unwrap());
        assert!((4.5..6.5).contains(&d), "{d} degrees per pixel");
    }

    #[test]
    fn android_sensor_intrinsics_map_to_the_upright_image() {
        // 4000 x 3000 active array, 640 x 480 analysis, front camera rotated 270.
        let k = CameraIntrinsics::from_android_sensor(
            [3200.0, 3200.0, 2010.0, 1490.0],
            [0.0, 0.0, 4000.0, 3000.0],
            [640.0, 480.0],
            270,
        )
        .unwrap();
        let s = 0.16;
        assert!((k.fx - 3200.0 * s).abs() < 1e-9 && (k.fy - 3200.0 * s).abs() < 1e-9);
        // Rotated 270 CW: (x, y) -> (y, w - x) with w = 640.
        assert!((k.cx - 1490.0 * s).abs() < 1e-9);
        assert!((k.cy - (640.0 - 2010.0 * s)).abs() < 1e-9);
        // 90 degrees: (x, y) -> (h - y, x).
        let k90 = CameraIntrinsics::from_android_sensor(
            [3200.0, 3100.0, 2010.0, 1490.0],
            [0.0, 0.0, 4000.0, 3000.0],
            [640.0, 480.0],
            90,
        )
        .unwrap();
        assert!((k90.fx - 3100.0 * s).abs() < 1e-9);
        assert!((k90.cx - (480.0 - 1490.0 * s)).abs() < 1e-9);
        assert!((k90.cy - 2010.0 * s).abs() < 1e-9);
        // 16:9 analysis of a 4:3 array: centre crop, scale by the larger ratio.
        let wide = CameraIntrinsics::from_android_sensor(
            [3200.0, 3200.0, 2000.0, 1500.0],
            [0.0, 0.0, 4000.0, 3000.0],
            [1280.0, 720.0],
            0,
        )
        .unwrap();
        assert!((wide.fx - 3200.0 * 0.32).abs() < 1e-9);
        assert!((wide.cy - 360.0).abs() < 1e-9);
        assert_eq!(
            CameraIntrinsics::from_android_sensor(
                [1.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 4000.0, 3000.0],
                [640.0, 480.0],
                45
            ),
            None
        );
        let nominal = CameraIntrinsics::from_android_focal_length(
            2.0,
            [4.0, 3.0],
            [4000.0, 3000.0],
            [640.0, 480.0],
            0,
        )
        .unwrap();
        assert!((nominal.fx - 320.0).abs() < 1e-9 && (nominal.cx - 320.0).abs() < 1e-9);
    }

    #[test]
    fn virtual_camera_matches_63_degrees() {
        let k = CameraIntrinsics::mediapipe_virtual(IMAGE);
        let fov = 2.0 * (IMAGE[1] / 2.0 / k.fy).atan();
        assert!((fov.to_degrees() - MEDIAPIPE_VERTICAL_FOV_DEG).abs() < 1e-9);
    }
}
