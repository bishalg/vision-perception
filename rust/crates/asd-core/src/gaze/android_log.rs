//! owner: WP-R.8
//!
//! Turns a WP-A.14 Android spike recording (MediaPipe back-end) into a replayable raw log
//! (REP-1): the app's `<base>.jsonl` raw log has `t_sec`, `face_tracked`, blink blend
//! shapes, orientation, clip time and the target schedule but no gaze geometry; the
//! `<base>.landmarks.jsonl` sidecar has MediaPipe's landmarks and facial transformation
//! matrix per frame; `<base>.meta.json` has the camera intrinsics and display facts. This
//! module merges them with [`super::mediapipe::convert_frame`] and fixes the header's
//! screen geometry. Text in, log out: no I/O (`asd-cli gaze convert-android` reads the
//! files).
//!
//! # Rules
//! - **Sidecar.** Optional first line `{"format": "android-gaze-spike-landmarks/1",
//!   "backend": ...}`; an `arcore` back-end is refused (no eye data). Every other line is
//!   one frame: `t_sec`, `face`, `image` (`[w, h]` of the upright image), `landmarks` (a
//!   subset of the 478-point mesh, `[x, y, z]` each, `null` for a non-finite number),
//!   `face_matrix` (16 numbers). The mesh index of each landmark position comes from the
//!   meta's `capture.landmarkIndices` / `landmarkIndicesWithoutIris`, else from
//!   [`DEFAULT_LANDMARK_INDICES`] (the spike app's `EyeLandmarks.WITH_IRIS`), picked by
//!   the line's landmark count.
//! - **Matching.** A raw sample takes the sidecar frame whose `t_sec` is within
//!   [`MATCH_TOLERANCE_SEC`] (both are the same camera timestamp, so they match exactly in
//!   practice). An unmatched sample is left without geometry (the pipeline sees `lost`).
//! - **Samples.** A converted frame sets `face_transform`, both eye transforms and, when the
//!   gaze is usable, `look_at`; `face_tracked` becomes true. A frame that cannot be
//!   converted keeps no geometry. Blink values already in the raw sample (MediaPipe's
//!   `eyeBlinkLeft/Right` blend shapes) are kept; when absent, the landmark closedness of
//!   the user's left/right eye is written instead.
//! - **Intrinsics**, first that exists: the caller's (upright image pixels), Camera2
//!   `LENS_INTRINSIC_CALIBRATION` from the meta, the nominal focal length and sensor size
//!   from the meta, else none (MediaPipe's virtual camera; warned, distances then rest on
//!   MediaPipe's 63-degree guess).
//! - **Screen geometry** (the header's `geometry`, which replay uses instead of a table
//!   lookup): the caller's screen size and camera offset (caliper measurements, `measured`),
//!   else the meta's dpi-derived size (`spec`, OEM-reported) and a camera offset assumed at
//!   the top centre of the active area (`unmeasured`), else the header's own geometry
//!   (warned). `geometrySource` is `table` only when both the size and the offset came from
//!   the caller; otherwise `idiomDefault`, which caps the replay verdict at `fair` (QS-4).
//! - **Header** `source` becomes `mediapipeFace`; portrait recordings only (the spike app
//!   records portrait only).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::mediapipe::{
    convert_frame, CameraIntrinsics, EyeLandmarks, EyeModel, FrameIssue, Landmark, MediaPipeFrame,
};
use super::raw_log::RawLog;
use crate::types::tracking::{
    CameraOffsetMm, CameraOffsetSource, DeviceGeometry, DeviceIdiom, GeometrySource, Orientation,
    ScreenSource, TrackingSource,
};

/// `format` of the sidecar header line written by the spike app.
pub const SIDECAR_FORMAT: &str = "android-gaze-spike-landmarks/1";

/// Mesh indices of the sidecar's landmark positions (spike app `EyeLandmarks.WITH_IRIS`:
/// both eye contours, seven head anchors, then the ten iris points). Without iris the
/// first 39 apply.
pub const DEFAULT_LANDMARK_INDICES: [u32; 49] = [
    33, 7, 163, 144, 145, 153, 154, 155, 133, 173, 157, 158, 159, 160, 161, 246, 263, 249, 390,
    373, 374, 380, 381, 382, 362, 398, 384, 385, 386, 387, 388, 466, 1, 4, 6, 10, 152, 234, 454,
    468, 469, 470, 471, 472, 473, 474, 475, 476, 477,
];

/// Largest `t_sec` difference at which a raw sample and a sidecar frame are the same frame.
pub const MATCH_TOLERANCE_SEC: f64 = 0.0005;

/// Mesh indices of the eye landmarks the model uses: outer, inner, upper lid, lower lid.
const RIGHT_EYE_MESH: [u32; 4] = [33, 133, 159, 145];
const LEFT_EYE_MESH: [u32; 4] = [263, 362, 386, 374];
/// Iris groups, centre first.
const IRIS_MESH: [[u32; 5]; 2] = [[468, 469, 470, 471, 472], [473, 474, 475, 476, 477]];

/// Caller inputs of [`convert_android_log`].
#[derive(Debug, Clone, Default)]
pub struct ConvertOptions {
    /// Active-area `[width, height]` in the portrait native frame, mm (calipers).
    pub screen_mm: Option<[f64; 2]>,
    /// Camera aperture centre from the active area's top-left, portrait native frame, mm,
    /// y down (negative = above the glass).
    pub camera_offset_mm: Option<[f64; 2]>,
    /// Intrinsics in pixels of the upright analysis image the landmarks refer to.
    pub intrinsics: Option<CameraIntrinsics>,
    pub model: EyeModel,
}

/// What the conversion did (printed by the CLI; the sheet's sanity checks).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConvertSummary {
    pub source: TrackingSource,
    /// Raw log samples.
    pub frames: usize,
    /// Sidecar frame lines.
    pub sidecar_frames: usize,
    pub matched_frames: usize,
    pub with_face_transform: usize,
    pub with_look_at: usize,
    /// Frames without `look_at`, by reason (`unmatched` = no sidecar frame).
    pub issues: BTreeMap<String, usize>,
    /// `caller`, `lensIntrinsicCalibration`, `focalLength` or `mediapipeVirtualCamera`.
    pub intrinsics_source: String,
    /// Intrinsics for `intrinsicsImage`, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intrinsics: Option<CameraIntrinsics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intrinsics_image: Option<[f64; 2]>,
    pub geometry_source: GeometrySource,
    pub screen_source: ScreenSource,
    pub screen_mm: [f64; 2],
    pub camera_offset_mm: [f64; 2],
    pub camera_offset_source: CameraOffsetSource,
    /// Median eye-midpoint distance from the screen plane over converted frames: compare
    /// with the ruler distance the operator entered (`metaDistanceCm`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub median_distance_cm: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta_distance_cm: Option<f64>,
    /// Median eyeball-centre distance; adults are about 60-65 mm.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub median_interocular_mm: Option<f64>,
    pub warnings: Vec<String>,
}

/// Why a recording could not be converted.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum AndroidConvertError {
    #[error("landmarks line {line}: {message}")]
    Sidecar { line: usize, message: String },
    #[error("unsupported landmarks format `{0}` (expected `{SIDECAR_FORMAT}`)")]
    SidecarFormat(String),
    #[error("ARCore recordings carry no eye data; only MediaPipe recordings can be converted")]
    Arcore,
    #[error("orientation {0:?}: only portrait recordings can be converted")]
    NotPortrait(Orientation),
    #[error("the raw log already carries face transforms (not a spike recording)")]
    AlreadyConverted,
    #[error("geometry: {0}")]
    Geometry(String),
    #[error("meta: {0}")]
    Meta(String),
}

#[derive(Debug, Deserialize)]
struct SidecarLine {
    t_sec: f64,
    #[serde(default)]
    face: bool,
    #[serde(default)]
    image: Option<[f64; 2]>,
    #[serde(default)]
    landmarks: Option<Vec<[Option<f64>; 3]>>,
    #[serde(default)]
    face_matrix: Option<Vec<Option<f64>>>,
}

struct Indices {
    with_iris: Vec<u32>,
    without_iris: Vec<u32>,
}

fn indices_from_meta(meta: Option<&Value>) -> Result<Indices, AndroidConvertError> {
    let read = |key: &str| -> Result<Option<Vec<u32>>, AndroidConvertError> {
        match meta.and_then(|m| m.pointer(&format!("/capture/{key}"))) {
            None => Ok(None),
            Some(v) => serde_json::from_value(v.clone())
                .map(Some)
                .map_err(|e| AndroidConvertError::Meta(format!("capture.{key}: {e}"))),
        }
    };
    let with_iris = read("landmarkIndices")?.unwrap_or_else(|| DEFAULT_LANDMARK_INDICES.to_vec());
    let without_iris = read("landmarkIndicesWithoutIris")?
        .unwrap_or_else(|| DEFAULT_LANDMARK_INDICES[..39].to_vec());
    Ok(Indices {
        with_iris,
        without_iris,
    })
}

fn num_array<const N: usize>(v: Option<&Value>) -> Option<[f64; N]> {
    let a = v?.as_array()?;
    if a.len() < N {
        return None;
    }
    let mut out = [0.0; N];
    for (o, x) in out.iter_mut().zip(a) {
        *o = x.as_f64()?;
    }
    Some(out)
}

/// Intrinsics from the meta, with their source and the upright image they belong to.
fn intrinsics_from_meta(meta: &Value) -> Option<(CameraIntrinsics, &'static str, [f64; 2])> {
    let cap = meta.get("capture")?;
    let analysis = num_array::<2>(cap.get("analysisImage"))?;
    let rotation = cap
        .get("rotationDegrees")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let upright = if rotation.rem_euclid(180) == 90 {
        [analysis[1], analysis[0]]
    } else {
        analysis
    };
    let lens = num_array::<4>(cap.get("lensIntrinsics_fx_fy_cx_cy_s"));
    let active = num_array::<4>(cap.get("sensorActiveArray"));
    if let (Some(k), Some(a)) = (lens, active) {
        if let Some(i) = CameraIntrinsics::from_android_sensor(k, a, analysis, rotation) {
            return Some((i, "lensIntrinsicCalibration", upright));
        }
    }
    let focal = cap
        .get("focalLengthsMm")
        .and_then(Value::as_array)
        .and_then(|a| a.first())
        .and_then(Value::as_f64)?;
    let sensor = num_array::<2>(cap.get("sensorPhysicalSizeMm"))?;
    let pixels = num_array::<2>(cap.get("sensorPixelArray"))?;
    CameraIntrinsics::from_android_focal_length(focal, sensor, pixels, analysis, rotation)
        .map(|i| (i, "focalLength", upright))
}

fn landmark(v: &[Option<f64>; 3]) -> Option<Landmark> {
    Some(Landmark {
        x: v[0]?,
        y: v[1]?,
        z: v[2]?,
    })
}

/// The model input of one sidecar frame.
fn build_frame(
    line: &SidecarLine,
    indices: &Indices,
    fallback_image: Option<[f64; 2]>,
) -> Result<MediaPipeFrame, FrameIssue> {
    let lms = match (&line.landmarks, line.face) {
        (Some(l), true) => l,
        _ => return Err(FrameIssue::NoFace),
    };
    let idx = if lms.len() == indices.with_iris.len() {
        &indices.with_iris
    } else if lms.len() == indices.without_iris.len() {
        &indices.without_iris
    } else {
        return Err(FrameIssue::IncompleteLandmarks);
    };
    let image = line
        .image
        .or(fallback_image)
        .ok_or(FrameIssue::IncompleteLandmarks)?;
    let get = |mesh: u32| -> Option<Landmark> {
        let pos = idx.iter().position(|&i| i == mesh)?;
        landmark(&lms[pos])
    };
    let eye = |m: [u32; 4]| -> Result<EyeLandmarks, FrameIssue> {
        let g = |i: usize| get(m[i]).ok_or(FrameIssue::IncompleteLandmarks);
        Ok(EyeLandmarks {
            outer: g(0)?,
            inner: g(1)?,
            upper: g(2)?,
            lower: g(3)?,
        })
    };
    let group = |m: [u32; 5]| -> Option<[Landmark; 5]> {
        Some([get(m[0])?, get(m[1])?, get(m[2])?, get(m[3])?, get(m[4])?])
    };
    let irises = match (group(IRIS_MESH[0]), group(IRIS_MESH[1])) {
        (Some(a), Some(b)) => Some([a, b]),
        _ => None,
    };
    let fm = line.face_matrix.as_ref().ok_or(FrameIssue::BadFaceMatrix)?;
    if fm.len() != 16 {
        return Err(FrameIssue::BadFaceMatrix);
    }
    let mut face_matrix = [0.0; 16];
    for (o, v) in face_matrix.iter_mut().zip(fm) {
        *o = v.ok_or(FrameIssue::BadFaceMatrix)?;
    }
    Ok(MediaPipeFrame {
        image_size: image,
        right_eye: eye(RIGHT_EYE_MESH)?,
        left_eye: eye(LEFT_EYE_MESH)?,
        irises,
        face_matrix,
    })
}

fn parse_sidecar(text: &str) -> Result<Vec<SidecarLine>, AndroidConvertError> {
    let mut frames = Vec::new();
    for (i, l) in text.lines().enumerate() {
        let line = i + 1;
        if l.trim().is_empty() {
            continue;
        }
        let v: Value = serde_json::from_str(l).map_err(|e| AndroidConvertError::Sidecar {
            line,
            message: e.to_string(),
        })?;
        if let Some(f) = v.get("format") {
            let f = f.as_str().unwrap_or_default();
            if f != SIDECAR_FORMAT {
                return Err(AndroidConvertError::SidecarFormat(f.to_string()));
            }
            if v.get("backend").and_then(Value::as_str) == Some("arcore") {
                return Err(AndroidConvertError::Arcore);
            }
            continue;
        }
        let s: SidecarLine =
            serde_json::from_value(v).map_err(|e| AndroidConvertError::Sidecar {
                line,
                message: e.to_string(),
            })?;
        frames.push(s);
    }
    frames.sort_by(|a, b| a.t_sec.total_cmp(&b.t_sec));
    Ok(frames)
}

fn median(mut v: Vec<f64>) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(f64::total_cmp);
    let n = v.len();
    Some(if n % 2 == 1 {
        v[n / 2]
    } else {
        (v[n / 2 - 1] + v[n / 2]) / 2.0
    })
}

fn positive2(v: [f64; 2]) -> bool {
    v.iter().all(|x| x.is_finite() && *x > 0.0)
}

/// Converts one spike recording (module docs). `meta` is the parsed `<base>.meta.json`.
pub fn convert_android_log(
    log: &RawLog,
    sidecar: &str,
    meta: Option<&Value>,
    opts: &ConvertOptions,
) -> Result<(RawLog, ConvertSummary), AndroidConvertError> {
    if log.header.orientation != Orientation::Portrait {
        return Err(AndroidConvertError::NotPortrait(log.header.orientation));
    }
    if log.samples.iter().any(|s| s.face_transform.is_some()) {
        return Err(AndroidConvertError::AlreadyConverted);
    }
    let frames = parse_sidecar(sidecar)?;
    let indices = indices_from_meta(meta)?;
    let mut warnings = Vec::new();

    // Intrinsics.
    let meta_upright = meta.and_then(|m| num_array::<2>(m.pointer("/capture/uprightImage")));
    let (intrinsics, intrinsics_source, intrinsics_image) = match opts.intrinsics {
        Some(k) => {
            if !k.is_valid() {
                return Err(AndroidConvertError::Geometry(
                    "intrinsics need finite values and positive focal lengths".into(),
                ));
            }
            (Some(k), "caller", None)
        }
        None => match meta.and_then(intrinsics_from_meta) {
            Some((k, src, img)) => (Some(k), src, Some(img)),
            None => {
                warnings.push(
                    "no camera intrinsics: MediaPipe's virtual 63-degree camera is used, so \
                     distances (and the eye-model scale) rest on MediaPipe's guess"
                        .into(),
                );
                (None, "mediapipeVirtualCamera", None)
            }
        },
    };
    let intrinsics_for = |image: [f64; 2]| -> Option<CameraIntrinsics> {
        let k = intrinsics?;
        match intrinsics_image {
            Some(from) if from != image => Some(k.scaled(from, image)),
            _ => Some(k),
        }
    };

    // Screen geometry.
    let display = meta.and_then(|m| m.get("display"));
    let dpi_mm = display.and_then(|d| {
        let w = d.get("widthMmFromDpi")?.as_f64()?;
        let h = d.get("heightMmFromDpi")?.as_f64()?;
        positive2([w, h]).then_some([w.min(h), w.max(h)])
    });
    let (screen_mm, screen_source) = match (opts.screen_mm, dpi_mm) {
        (Some(s), _) => (Some(s), ScreenSource::Measured),
        (None, Some(s)) => {
            warnings.push(
                "screen size from the OEM-reported dpi; measure the active area with calipers \
                 and pass --screen-mm"
                    .into(),
            );
            (Some(s), ScreenSource::Spec)
        }
        (None, None) => (None, log.header.geometry.source),
    };
    if let Some(s) = screen_mm {
        if !positive2(s) {
            return Err(AndroidConvertError::Geometry(
                "screen size must be two positive numbers (mm)".into(),
            ));
        }
    }
    if let Some(c) = opts.camera_offset_mm {
        if !c.iter().all(|v| v.is_finite()) {
            return Err(AndroidConvertError::Geometry(
                "camera offset must be two finite numbers (mm)".into(),
            ));
        }
    }
    let geometry = match screen_mm {
        Some([w, h]) => {
            let (offset, offset_source) = match opts.camera_offset_mm {
                Some(c) => (c, CameraOffsetSource::Measured),
                None => {
                    warnings.push(
                        "camera offset assumed at the top centre of the active area; measure it \
                         and pass --camera-offset-mm"
                            .into(),
                    );
                    ([w / 2.0, 0.0], CameraOffsetSource::Unmeasured)
                }
            };
            let native_pixels = display
                .and_then(|d| num_array::<2>(d.get("realPixels")))
                .filter(|p| positive2(*p))
                .map(|p| [p[0].min(p[1]) as u32, p[0].max(p[1]) as u32]);
            DeviceGeometry {
                model_identifiers: vec![log.header.device_model.clone()],
                display_name: format!("{} (Android)", log.header.device_model),
                idiom: DeviceIdiom::Phone,
                native_pixels,
                ppi: None,
                screen_width_mm: w,
                screen_height_mm: h,
                camera_offset_mm: CameraOffsetMm {
                    x: offset[0],
                    y: offset[1],
                    source: offset_source,
                },
                source: screen_source,
                reference: match screen_source {
                    ScreenSource::Measured => {
                        "asd-cli gaze convert-android: caliper measurements (--screen-mm)".into()
                    }
                    _ => "asd-cli gaze convert-android: OEM-reported dpi (meta.display)".into(),
                },
                note: None,
            }
        }
        None => {
            warnings.push(
                "no screen size given and none in the meta: the header's own geometry (an \
                 iPhone idiom default for spike logs) is kept"
                    .into(),
            );
            let mut g = log.header.geometry.clone();
            if let Some(c) = opts.camera_offset_mm {
                g.camera_offset_mm = CameraOffsetMm {
                    x: c[0],
                    y: c[1],
                    source: CameraOffsetSource::Measured,
                };
            }
            g
        }
    };
    let geometry_source = if opts.screen_mm.is_some() && opts.camera_offset_mm.is_some() {
        GeometrySource::Table
    } else {
        GeometrySource::IdiomDefault
    };

    // Samples.
    let mut out = log.clone();
    out.header.source = TrackingSource::MediapipeFace;
    out.header.geometry = geometry.clone();
    out.header.geometry_source = geometry_source;

    let mut issues: BTreeMap<String, usize> = BTreeMap::new();
    let mut bump = |k: &str| *issues.entry(k.to_string()).or_insert(0) += 1;
    let (mut matched, mut with_face, mut with_look) = (0usize, 0usize, 0usize);
    let (mut distances, mut interocular) = (Vec::new(), Vec::new());
    for s in &mut out.samples {
        let i = frames.partition_point(|f| f.t_sec < s.t_sec);
        let nearest = [i.checked_sub(1), Some(i)]
            .into_iter()
            .flatten()
            .filter_map(|j| frames.get(j))
            .min_by(|a, b| {
                (a.t_sec - s.t_sec)
                    .abs()
                    .total_cmp(&(b.t_sec - s.t_sec).abs())
            })
            .filter(|f| (f.t_sec - s.t_sec).abs() <= MATCH_TOLERANCE_SEC);
        let Some(f) = nearest else {
            bump("unmatched");
            continue;
        };
        matched += 1;
        let converted = build_frame(f, &indices, meta_upright).and_then(|mf| {
            let k = intrinsics_for(mf.image_size);
            convert_frame(&mf, k.as_ref(), &opts.model)
        });
        let c = match converted {
            Ok(c) => c,
            Err(issue) => {
                bump(issue.as_str());
                continue;
            }
        };
        s.face_tracked = true;
        s.face_transform = Some(c.face_transform);
        s.left_eye_transform = Some(c.left_eye_transform);
        s.right_eye_transform = Some(c.right_eye_transform);
        s.look_at = c.look_at;
        s.eye_blink_left = s.eye_blink_left.or(Some(c.left_closedness));
        s.eye_blink_right = s.eye_blink_right.or(Some(c.right_closedness));
        with_face += 1;
        distances.push(c.distance_m * 100.0);
        interocular.push(c.interocular_m * 1000.0);
        match (c.look_at, c.issue) {
            (Some(_), _) => with_look += 1,
            (None, Some(issue)) => bump(issue.as_str()),
            (None, None) => bump("noLookAt"),
        }
    }
    if matched == 0 && !out.samples.is_empty() {
        warnings.push("no raw sample matched a landmarks frame (t_sec)".into());
    }

    let summary = ConvertSummary {
        source: TrackingSource::MediapipeFace,
        frames: out.samples.len(),
        sidecar_frames: frames.len(),
        matched_frames: matched,
        with_face_transform: with_face,
        with_look_at: with_look,
        issues,
        intrinsics_source: intrinsics_source.to_string(),
        intrinsics,
        intrinsics_image,
        geometry_source,
        screen_source: geometry.source,
        screen_mm: [geometry.screen_width_mm, geometry.screen_height_mm],
        camera_offset_mm: [geometry.camera_offset_mm.x, geometry.camera_offset_mm.y],
        camera_offset_source: geometry.camera_offset_mm.source,
        median_distance_cm: median(distances),
        meta_distance_cm: meta
            .and_then(|m| m.get("distanceCm"))
            .and_then(Value::as_f64),
        median_interocular_mm: median(interocular),
        warnings,
    };
    Ok((out, summary))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gaze::geometry::idiom_default;
    use crate::gaze::mediapipe::synth::{frame, SynthFace};
    use crate::types::tracking::{RawLogHeader, RawTrackingSample, RAW_LOG_FORMAT};
    use serde_json::json;

    const IMAGE: [f64; 2] = [480.0, 640.0];

    fn header() -> RawLogHeader {
        RawLogHeader {
            format: RAW_LOG_FORMAT.into(),
            source: TrackingSource::ArkitFace,
            device_model: "Pixel 7a".into(),
            orientation: Orientation::Portrait,
            geometry_source: GeometrySource::IdiomDefault,
            geometry: idiom_default(DeviceIdiom::Phone),
            targets: Vec::new(),
            app_version: Some("spike".into()),
            condition: None,
            ground_truth: None,
            clips: Vec::new(),
            bundle_version: None,
            profile_version: None,
            session_id: None,
            participant_token: None,
        }
    }

    fn sample(t: f64, face: bool) -> RawTrackingSample {
        RawTrackingSample {
            t_sec: t,
            face_tracked: face,
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
            affect: None,
        }
    }

    /// One sidecar line in the app's layout from a model frame.
    fn sidecar_line(t: f64, f: &MediaPipeFrame, iris: bool) -> String {
        let mut by_mesh: BTreeMap<u32, Landmark> = BTreeMap::new();
        for (m, l) in RIGHT_EYE_MESH.iter().zip([
            f.right_eye.outer,
            f.right_eye.inner,
            f.right_eye.upper,
            f.right_eye.lower,
        ]) {
            by_mesh.insert(*m, l);
        }
        for (m, l) in LEFT_EYE_MESH.iter().zip([
            f.left_eye.outer,
            f.left_eye.inner,
            f.left_eye.upper,
            f.left_eye.lower,
        ]) {
            by_mesh.insert(*m, l);
        }
        if let Some(ir) = f.irises {
            for (g, mesh) in ir.iter().zip(IRIS_MESH) {
                for (l, m) in g.iter().zip(mesh) {
                    by_mesh.insert(m, *l);
                }
            }
        }
        let n = if iris { 49 } else { 39 };
        let lms: Vec<Value> = DEFAULT_LANDMARK_INDICES[..n]
            .iter()
            .map(|m| {
                let l = by_mesh.get(m).copied().unwrap_or(Landmark {
                    x: 0.5,
                    y: 0.5,
                    z: 0.0,
                });
                json!([l.x, l.y, l.z])
            })
            .collect();
        json!({"t_sec": t, "frame_ns": (t * 1e9) as i64, "face": true, "image": IMAGE,
               "landmarks": lms, "face_matrix": f.face_matrix.to_vec()})
        .to_string()
    }

    fn camera() -> CameraIntrinsics {
        CameraIntrinsics {
            fx: 500.0,
            fy: 500.0,
            cx: 240.0,
            cy: 320.0,
        }
    }

    fn recording() -> (RawLog, String) {
        let model = EyeModel::default();
        let face = SynthFace::adult([0.0, -0.07, -0.5]);
        let target = [0.0, -0.07, 0.0];
        let f = frame(&face, target, &camera(), IMAGE, &model, true);
        let f_no_iris = frame(&face, target, &camera(), IMAGE, &model, false);
        let mut sidecar = vec![
            json!({"format": SIDECAR_FORMAT, "backend": "mediapipe-cpu"}).to_string(),
            sidecar_line(1.0, &f, true),
            sidecar_line(1.05, &f_no_iris, false),
            json!({"t_sec": 1.1, "frame_ns": 1_100_000_000i64, "face": false}).to_string(),
        ];
        sidecar.push(sidecar_line(1.2, &f, true));
        let log = RawLog {
            header: header(),
            samples: vec![
                sample(1.0, true),
                sample(1.05, true),
                sample(1.1, false),
                sample(1.15, true),
                sample(1.2, true),
            ],
        };
        (log, sidecar.join("\n"))
    }

    #[test]
    fn merges_frames_and_counts_issues() {
        let (log, sidecar) = recording();
        let opts = ConvertOptions {
            screen_mm: Some([68.0, 150.0]),
            camera_offset_mm: Some([34.0, -3.0]),
            intrinsics: Some(camera()),
            ..Default::default()
        };
        let (out, sum) = convert_android_log(&log, &sidecar, None, &opts).unwrap();
        assert_eq!(out.header.source, TrackingSource::MediapipeFace);
        assert_eq!(out.header.geometry_source, GeometrySource::Table);
        assert_eq!(out.header.geometry.screen_width_mm, 68.0);
        assert_eq!(out.header.geometry.source, ScreenSource::Measured);
        assert_eq!(sum.frames, 5);
        assert_eq!(sum.sidecar_frames, 4);
        assert_eq!(sum.matched_frames, 4);
        assert_eq!(sum.with_face_transform, 3);
        assert_eq!(sum.with_look_at, 2);
        assert_eq!(sum.issues.get("missingIris"), Some(&1));
        assert_eq!(sum.issues.get("noFace"), Some(&1));
        assert_eq!(sum.issues.get("unmatched"), Some(&1));
        assert_eq!(sum.intrinsics_source, "caller");
        assert!((sum.median_distance_cm.unwrap() - 47.0).abs() < 1e-6);
        assert!((sum.median_interocular_mm.unwrap() - 62.0).abs() < 1e-6);
        assert!(sum.warnings.is_empty(), "{:?}", sum.warnings);
        let s0 = &out.samples[0];
        assert!(s0.face_tracked && s0.look_at.is_some() && s0.face_transform.is_some());
        // Closedness fills absent blink values.
        assert!(s0.eye_blink_left.unwrap() < 0.1);
        let s1 = &out.samples[1];
        assert!(s1.face_transform.is_some() && s1.look_at.is_none());
        assert!(out.samples[2].face_transform.is_none());
        assert!(out.samples[3].face_transform.is_none());
        // The converted log is a valid raw log again.
        let text = crate::gaze::raw_log::to_jsonl(&out).unwrap();
        assert_eq!(crate::gaze::raw_log::parse(&text).unwrap(), out);
    }

    #[test]
    fn meta_supplies_intrinsics_and_dpi_geometry() {
        let (log, sidecar) = recording();
        // Analysis 640 x 480 rotated 270; lens calibration = camera() before rotation.
        let meta = json!({
            "distanceCm": 50,
            "display": {"realPixels": [1080, 2400], "widthMmFromDpi": 69.5,
                        "heightMmFromDpi": 154.4},
            "capture": {"analysisImage": [640, 480], "uprightImage": [480, 640],
                        "rotationDegrees": 270,
                        "lensIntrinsics_fx_fy_cx_cy_s": [500.0, 500.0, 320.0, 240.0, 0.0],
                        "sensorActiveArray": [0, 0, 640, 480]}
        });
        let (out, sum) =
            convert_android_log(&log, &sidecar, Some(&meta), &ConvertOptions::default()).unwrap();
        assert_eq!(sum.intrinsics_source, "lensIntrinsicCalibration");
        let k = sum.intrinsics.unwrap();
        assert!((k.cx - 240.0).abs() < 1e-9 && (k.cy - 320.0).abs() < 1e-9);
        assert_eq!(sum.geometry_source, GeometrySource::IdiomDefault);
        assert_eq!(sum.screen_source, ScreenSource::Spec);
        assert_eq!(sum.camera_offset_source, CameraOffsetSource::Unmeasured);
        assert_eq!(out.header.geometry.native_pixels, Some([1080, 2400]));
        assert_eq!(sum.meta_distance_cm, Some(50.0));
        assert!((sum.median_distance_cm.unwrap() - 47.0).abs() < 1e-6);
        assert_eq!(sum.warnings.len(), 2);
    }

    #[test]
    fn without_intrinsics_or_screen_it_warns_and_keeps_the_header_geometry() {
        let (log, sidecar) = recording();
        let (out, sum) =
            convert_android_log(&log, &sidecar, None, &ConvertOptions::default()).unwrap();
        assert_eq!(sum.intrinsics_source, "mediapipeVirtualCamera");
        assert_eq!(out.header.geometry, log.header.geometry);
        assert_eq!(sum.warnings.len(), 2);
    }

    #[test]
    fn refuses_arcore_wrong_format_landscape_and_converted_logs() {
        let (log, _) = recording();
        let o = ConvertOptions::default();
        let arcore = json!({"format": SIDECAR_FORMAT, "backend": "arcore"}).to_string();
        assert_eq!(
            convert_android_log(&log, &arcore, None, &o),
            Err(AndroidConvertError::Arcore)
        );
        let other = json!({"format": "something/2"}).to_string();
        assert!(matches!(
            convert_android_log(&log, &other, None, &o),
            Err(AndroidConvertError::SidecarFormat(_))
        ));
        assert!(matches!(
            convert_android_log(&log, "{not json", None, &o),
            Err(AndroidConvertError::Sidecar { line: 1, .. })
        ));
        let mut land = log.clone();
        land.header.orientation = Orientation::LandscapeLeft;
        assert!(matches!(
            convert_android_log(&land, "", None, &o),
            Err(AndroidConvertError::NotPortrait(_))
        ));
        let (log, sidecar) = recording();
        let (done, _) = convert_android_log(&log, &sidecar, None, &o).unwrap();
        assert_eq!(
            convert_android_log(&done, &sidecar, None, &o),
            Err(AndroidConvertError::AlreadyConverted)
        );
        let bad = ConvertOptions {
            screen_mm: Some([0.0, 150.0]),
            ..Default::default()
        };
        assert!(matches!(
            convert_android_log(&log, &sidecar, None, &bad),
            Err(AndroidConvertError::Geometry(_))
        ));
    }
}
