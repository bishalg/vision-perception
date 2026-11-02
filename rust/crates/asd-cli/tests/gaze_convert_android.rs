//! owner: WP-R.8
//!
//! `asd-cli gaze convert-android` end to end on a committed **synthetic** spike recording
//! (`tests/fixtures/android_mediapipe/synthetic.*`; no person's data): convert, then
//! `gaze replay --check`, then a full replay that scores the validation in degrees.
//!
//! The fixture is made by [`generate`] from `asd_core::gaze::mediapipe::synth` (the converter's
//! exact forward model) with a known screen, camera and head sway, in the spike app's file
//! layout. Regenerate it with
//! `ASD_REGEN_ANDROID_FIXTURE=1 cargo test -p asd-cli --test gaze_convert_android`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use asd_core::gaze::android_log::{DEFAULT_LANDMARK_INDICES, SIDECAR_FORMAT};
use asd_core::gaze::geometry::idiom_default;
use asd_core::gaze::mediapipe::synth::{frame, SynthFace};
use asd_core::gaze::mediapipe::{CameraIntrinsics, EyeModel, Landmark, MediaPipeFrame};
use asd_core::gaze::raw_log::{to_jsonl, RawLog};
use asd_core::types::tracking::{
    DeviceIdiom, GeometrySource, Orientation, RawLogHeader, RawTrackingSample, ScheduledTarget,
    TargetKind, TrackingQualityParams, TrackingSource, RAW_LOG_FORMAT,
};
use serde_json::{json, Value};

/// The simulated phone: screen and camera the converter is told about with flags.
const SCREEN_MM: [f64; 2] = [68.0, 150.0];
const CAMERA_MM: [f64; 2] = [34.0, -3.0];
const IMAGE: [f64; 2] = [480.0, 640.0];
const FPS: f64 = 20.0;
const T0: f64 = 100.0;

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/android_mediapipe")
}

fn cli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_asd-cli"))
        .args(args)
        .output()
        .expect("run asd-cli")
}

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("asd-cli-android-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

fn p(path: &Path) -> &str {
    path.to_str().unwrap()
}

// ------------------------------------------------------------------------------------------
// Fixture generator
// ------------------------------------------------------------------------------------------

/// Upright-image intrinsics of the simulated front camera.
fn camera() -> CameraIntrinsics {
    CameraIntrinsics {
        fx: 505.0,
        fy: 498.0,
        cx: 236.0,
        cy: 327.0,
    }
}

/// Screen-normalised point -> pipeline camera frame (portrait, camera coplanar).
fn screen_p(norm: [f64; 2]) -> [f64; 3] {
    [
        (norm[0] * SCREEN_MM[0] - CAMERA_MM[0]) / 1000.0,
        -(norm[1] * SCREEN_MM[1] - CAMERA_MM[1]) / 1000.0,
        0.0,
    ]
}

fn r5(x: f64) -> f64 {
    (x * 1e5).round() / 1e5
}

fn r6(x: f64) -> f64 {
    (x * 1e6).round() / 1e6
}

/// One sidecar line in the spike app's layout (`MediaPipeBackend.sidecarLine`). Landmarks
/// the converter does not read (the other eye-contour points and the head anchors) are
/// written as `[0, 0, 0]`: this fixture does not model them.
fn sidecar_line(t: f64, f: Option<&MediaPipeFrame>, blink: f64) -> String {
    let Some(f) = f else {
        return json!({"t_sec": t, "frame_ns": (t * 1e9).round() as i64, "face": false})
            .to_string();
    };
    let (re, le) = (&f.right_eye, &f.left_eye);
    let pick = |mesh: u32| -> Option<Landmark> {
        match mesh {
            33 => Some(re.outer),
            133 => Some(re.inner),
            159 => Some(re.upper),
            145 => Some(re.lower),
            263 => Some(le.outer),
            362 => Some(le.inner),
            386 => Some(le.upper),
            374 => Some(le.lower),
            468..=472 => f.irises.map(|ir| ir[0][(mesh - 468) as usize]),
            473..=477 => f.irises.map(|ir| ir[1][(mesh - 473) as usize]),
            _ => None,
        }
    };
    let n = if f.irises.is_some() { 49 } else { 39 };
    let landmarks: Vec<Value> = DEFAULT_LANDMARK_INDICES[..n]
        .iter()
        .map(|m| match pick(*m) {
            Some(l) => json!([r5(l.x), r5(l.y), r5(l.z)]),
            None => json!([0, 0, 0]),
        })
        .collect();
    json!({
        "t_sec": t,
        "frame_ns": (t * 1e9).round() as i64,
        "face": true,
        "image": IMAGE,
        "landmarks": landmarks,
        "blendshapes": {"eyeBlinkLeft": blink, "eyeBlinkRight": blink},
        "face_matrix": f.face_matrix.iter().map(|v| r6(*v)).collect::<Vec<_>>(),
    })
    .to_string()
}

/// The three fixture files: raw log, landmarks sidecar, meta.
fn generate() -> (String, String, String) {
    let params = TrackingQualityParams::default();
    let model = EyeModel::default();
    let cal = &params.calibration;
    let val = &params.validation;

    // Target schedule: 5-point calibration, then the 4-point validation, then a 1 s clip.
    let mut targets = Vec::new();
    let mut t = T0 + 0.3;
    let mut push = |kind, at: [f64; 2], window_sec: f64, t: &mut f64| {
        targets.push(ScheduledTarget {
            kind,
            x_norm: at[0],
            y_norm: at[1],
            start_sec: r6(*t),
            end_sec: r6(*t + window_sec),
            label: None,
        });
        *t += window_sec + 0.1;
    };
    for at in &cal.layout5 {
        push(
            TargetKind::Calibration,
            *at,
            (cal.settle_ms + cal.collect_ms) / 1000.0,
            &mut t,
        );
    }
    for at in &val.layout {
        push(
            TargetKind::Validation,
            *at,
            (val.settle_ms + val.collect_ms) / 1000.0,
            &mut t,
        );
    }
    let clip_start = t;
    let clip_end = clip_start + 1.0;

    let mut samples = Vec::new();
    let mut lines = vec![json!({"format": SIDECAR_FORMAT, "backend": "mediapipe-cpu",
        "note": "SYNTHETIC fixture (asd-cli tests, WP-R.8); no person's data"})
    .to_string()];
    let frames = ((clip_end - T0) * FPS).ceil() as usize;
    for n in 0..frames {
        let t = r6(T0 + n as f64 / FPS);
        // Gaze: the target that is due (from 0.15 s before its window), else the last one;
        // in the clip, a slow sweep across the middle of the screen.
        let in_clip = t >= clip_start;
        let gaze = if in_clip {
            let u = (t - clip_start) / (clip_end - clip_start);
            [0.3 + 0.4 * u, 0.5]
        } else {
            targets
                .iter()
                .rev()
                .find(|g| t >= g.start_sec - 0.15)
                .map_or([0.5, 0.5], |g| [g.x_norm, g.y_norm])
        };
        let mut face = SynthFace::adult([0.004, -0.108, -0.48]);
        face.yaw_deg = 3.0 * (std::f64::consts::TAU * 0.2 * (t - T0)).sin();
        face.pitch_deg = -2.0 + 1.5 * (std::f64::consts::TAU * 0.13 * (t - T0)).sin();
        face.position_m[0] += 0.004 * (std::f64::consts::TAU * 0.1 * (t - T0)).sin();
        // Clip events: a two-frame blink, a frame without iris, a frame without a face.
        let k = ((t - clip_start) * FPS).round() as i64;
        let (mut iris, mut has_face) = (true, true);
        if in_clip {
            match k {
                4 | 5 => face.lid_half_open_m = [0.0004, 0.0004],
                10 => iris = false,
                14 => has_face = false,
                _ => {}
            }
        }
        let f = frame(&face, screen_p(gaze), &camera(), IMAGE, &model, iris);
        let blink = if face.lid_half_open_m[0] < 0.001 {
            0.9
        } else {
            0.05
        };
        let clip_time = in_clip.then(|| r6(t - clip_start));
        samples.push(RawTrackingSample {
            t_sec: t,
            face_tracked: has_face,
            face_transform: None,
            look_at: None,
            left_eye_transform: None,
            right_eye_transform: None,
            eye_blink_left: has_face.then_some(blink),
            eye_blink_right: has_face.then_some(blink),
            ambient_intensity: None,
            ambient_color_temp: None,
            light_direction: None,
            orientation: Orientation::Portrait,
            clip_media_time_sec: clip_time,
            stimulus_id: clip_time.map(|_| "spike-free-view".to_string()),
        });
        lines.push(sidecar_line(t, has_face.then_some(&f), blink));
    }

    // As the spike app writes it: `arkitFace` and the iPhone idiom default geometry.
    let log = RawLog {
        header: RawLogHeader {
            format: RAW_LOG_FORMAT.to_string(),
            source: TrackingSource::ArkitFace,
            device_model: "SyntheticAndroidPhone".into(),
            orientation: Orientation::Portrait,
            geometry_source: GeometrySource::IdiomDefault,
            geometry: idiom_default(DeviceIdiom::Phone),
            targets,
            app_version: Some("android-gaze-spike synthetic fixture (WP-R.8)".into()),
            condition: Some("baseline".into()),
            ground_truth: Some(json!({
                "generator": "asd-cli tests gaze_convert_android (WP-R.8)",
                "screenMm": SCREEN_MM, "cameraOffsetMm": CAMERA_MM,
                "intrinsics": camera(), "fps": FPS
            })),
        },
        samples,
    };

    // Camera2 values that map to `camera()` on the upright image: 4000 x 3000 active array,
    // 640 x 480 analysis (scale 0.16), rotated 270 (see CameraIntrinsics::from_android_sensor).
    let s = 0.16;
    let k = camera();
    let lens = [
        k.fy / s,
        k.fx / s,
        (640.0 - k.cy - 320.0) / s + 2000.0,
        (k.cx - 240.0) / s + 1500.0,
    ];
    let meta = json!({
        "format": "android-gaze-spike-meta/1",
        "rawLog": "synthetic.jsonl",
        "note": "SYNTHETIC fixture (asd-cli tests, WP-R.8); no person's data",
        "condition": "baseline",
        "distanceCm": 45,
        "device": {"manufacturer": "synthetic", "model": "SyntheticAndroidPhone"},
        "display": {"realPixels": [1080, 2400], "xdpi": 400.0, "ydpi": 400.0,
                    "widthMmFromDpi": 68.58, "heightMmFromDpi": 152.4},
        "capture": {
            "backend": "mediapipe",
            "imageMirrored": false,
            "landmarkIndices": DEFAULT_LANDMARK_INDICES.to_vec(),
            "landmarkIndicesWithoutIris": &DEFAULT_LANDMARK_INDICES[..39],
            "analysisImage": [640, 480],
            "uprightImage": IMAGE,
            "rotationDegrees": 270,
            "lensIntrinsics_fx_fy_cx_cy_s": [lens[0], lens[1], lens[2], lens[3], 0.0],
            "sensorActiveArray": [0, 0, 4000, 3000]
        }
    });
    (
        to_jsonl(&log).unwrap(),
        lines.join("\n") + "\n",
        serde_json::to_string_pretty(&meta).unwrap() + "\n",
    )
}

#[test]
fn regenerate_fixture_when_asked() {
    if std::env::var("ASD_REGEN_ANDROID_FIXTURE").as_deref() != Ok("1") {
        return;
    }
    let d = fixture_dir();
    fs::create_dir_all(&d).unwrap();
    let (log, sidecar, meta) = generate();
    fs::write(d.join("synthetic.jsonl"), log).unwrap();
    fs::write(d.join("synthetic.landmarks.jsonl"), sidecar).unwrap();
    fs::write(d.join("synthetic.meta.json"), meta).unwrap();
}

// ------------------------------------------------------------------------------------------
// End to end
// ------------------------------------------------------------------------------------------

fn convert(out: &Path, extra: &[&str]) -> (Output, Value) {
    let d = fixture_dir();
    let (log, lm, meta) = (
        d.join("synthetic.jsonl"),
        d.join("synthetic.landmarks.jsonl"),
        d.join("synthetic.meta.json"),
    );
    let mut args = vec![
        "gaze",
        "convert-android",
        p(&log),
        p(&lm),
        "--meta",
        p(&meta),
        "-o",
        p(out),
    ];
    args.extend_from_slice(extra);
    let o = cli(&args);
    let summary = serde_json::from_slice(&o.stdout).unwrap_or(Value::Null);
    (o, summary)
}

fn replay_json(log: &Path) -> Value {
    let o = cli(&["gaze", "replay", p(log)]);
    assert_eq!(
        o.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    serde_json::from_slice(&o.stdout).unwrap()
}

#[test]
fn unconverted_spike_log_scores_nothing() {
    // The binding gap the converter closes: the raw spike log has no gaze geometry.
    let r = replay_json(&fixture_dir().join("synthetic.jsonl"));
    let hits = r["targets"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|t| t["outcome"] == "hit")
        .count();
    assert_eq!(hits, 0);
}

#[test]
fn converts_and_replays_in_degrees() {
    let d = scratch("measured");
    let out = d.join("converted.jsonl");
    let (o, summary) = convert(
        &out,
        &["--screen-mm", "68,150", "--camera-offset-mm", "34,-3"],
    );
    assert_eq!(
        o.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    assert_eq!(summary["source"], "mediapipeFace");
    assert_eq!(summary["intrinsicsSource"], "lensIntrinsicCalibration");
    assert_eq!(summary["geometrySource"], "table");
    assert_eq!(summary["matchedFrames"], summary["frames"]);
    assert_eq!(summary["issues"]["eyesClosed"], 2);
    assert_eq!(summary["issues"]["missingIris"], 1);
    assert_eq!(summary["issues"]["noFace"], 1);
    let dist = summary["medianDistanceCm"].as_f64().unwrap();
    assert!((dist - 45.0).abs() < 0.5, "distance {dist}");
    let iod = summary["medianInterocularMm"].as_f64().unwrap();
    assert!((iod - 62.0).abs() < 0.5, "interocular {iod}");

    let check = cli(&["gaze", "replay", "--check", p(&out)]);
    assert_eq!(
        check.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&check.stderr)
    );

    let r = replay_json(&out);
    let validation: Vec<&Value> = r["targets"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|t| t["kind"] == "validation")
        .collect();
    assert_eq!(validation.len(), 4);
    for t in &validation {
        assert_eq!(t["outcome"], "hit", "{t}");
        let acc = t["accuracyDeg"].as_f64().unwrap();
        assert!(acc < 0.5, "validation target accuracy {acc} deg");
    }
    let tq = &r["summary"];
    let acc = tq["validation"]["accuracyDeg"].as_f64().unwrap();
    assert!(acc < 0.5, "session accuracy {acc} deg");
    assert_eq!(tq["device"]["geometrySource"], "table");
    assert_eq!(tq["device"]["screenWidthMm"], 68.0);
    assert_eq!(tq["calibration"]["usablePoints"], 5);
}

#[test]
fn dpi_geometry_still_scores_but_caps_the_verdict() {
    let d = scratch("dpi");
    let out = d.join("converted.jsonl");
    let (o, summary) = convert(&out, &[]);
    assert_eq!(o.status.code(), Some(0));
    assert_eq!(summary["geometrySource"], "idiomDefault");
    assert_eq!(summary["screenSource"], "spec");
    assert!(String::from_utf8_lossy(&o.stderr).contains("warning: camera offset assumed"));
    let r = replay_json(&out);
    let acc = r["summary"]["validation"]["accuracyDeg"].as_f64().unwrap();
    // Wrong screen millimetres and camera offset distort the mapping a little; the
    // calibration absorbs most of it.
    assert!(acc < 2.0, "accuracy {acc} deg");
    assert_ne!(r["summary"]["verdict"], "good");
}

#[test]
fn usage_and_input_errors() {
    let d = scratch("errors");
    let out = d.join("x.jsonl");
    let fx = fixture_dir();
    let log = fx.join("synthetic.jsonl");
    let o = cli(&["gaze", "convert-android", p(&log), "-o", p(&out)]);
    assert_eq!(o.status.code(), Some(2));
    let o = cli(&[
        "gaze",
        "convert-android",
        p(&log),
        p(&fx.join("synthetic.landmarks.jsonl")),
        "-o",
        p(&out),
        "--screen-mm",
        "68",
    ]);
    assert_eq!(o.status.code(), Some(2));
    // An ARCore sidecar is refused with exit 1.
    let arcore = d.join("arcore.landmarks.jsonl");
    fs::write(
        &arcore,
        json!({"format": SIDECAR_FORMAT, "backend": "arcore"}).to_string(),
    )
    .unwrap();
    let o = cli(&[
        "gaze",
        "convert-android",
        p(&log),
        p(&arcore),
        "-o",
        p(&out),
    ]);
    assert_eq!(o.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&o.stderr).contains("ARCore"));
}
