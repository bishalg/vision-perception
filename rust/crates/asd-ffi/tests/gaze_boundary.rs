//! owner: WP-G.7
//!
//! ADR-0007 D2 / PIPE-1 across the boundary: `GazePipelineHandle` driven exactly as Swift
//! drives it (flat records per frame, JSON config in and summary out) gives the same results
//! as calling `asd_core::gaze::pipeline::GazePipeline` directly.
//!
//! - Records round-trip every field (raw and processed).
//! - Malformed config / model / AOI JSON and wrong-length matrices are `AsdError`, never a
//!   panic; non-finite numbers and API misuse do not panic either.
//! - A scripted session (calibration, validation, recorded clips with blinks, track loss and
//!   AOIs, drift checks, a re-calibration) and a 10-minute synthetic stream through the
//!   handle equal the same streams through `GazePipeline`, frame by frame and in `summary()`.
//! - `per_frame_cost_through_the_handle` (ignored; `--ignored --nocapture`) reports the
//!   host per-call cost (PIPE-4 is measured on device in WP-G.9).
//!
//! Synthetic head as in `asd-core/tests/gaze_pipeline.rs`: camera frame in metres, portrait
//! native, the viewer at z < 0, a 100 x 200 mm panel, a known angular eye-model bias.

use std::sync::Arc;

use asd_core::gaze::calibration::TargetStatus;
use asd_core::gaze::geometry::idiom_default;
use asd_core::gaze::pipeline::{GazePipeline, PipelineConfig, TelemetryWriter};
use asd_core::quality::QualityGate;
use asd_core::types::tracking::{
    DeviceGeometry, DeviceIdiom, GeometrySource, Orientation, ProcessedSample, RawTrackingSample,
    RayStatus, RecalReason, TargetKind, TrackState, TrackingQualityParams,
};
use asd_ffi::gaze::*;
use asd_ffi::AsdError;
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
        glasses: Some(false),
    }
}

fn config_json() -> String {
    serde_json::to_string(&config()).unwrap()
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

fn face(distance_m: f64, yaw_deg: f64) -> M4 {
    let c = screen_cam([0.5, 0.5]);
    yaw_rot(yaw_deg, [c[0], c[1], -distance_m])
}

#[derive(Clone, Copy, PartialEq)]
enum Eye {
    Open,
    Closed,
    NoFace,
}

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
        ambient_color_temp: Some(6500.0),
        light_direction: Some([0.0, -1.0, 0.0]),
        orientation: Orientation::Portrait,
        clip_media_time_sec: None,
        stimulus_id: None,
    };
    if eye == Eye::NoFace {
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
    s.eye_blink_left = Some(if eye == Eye::Closed { 0.9 } else { 0.05 });
    s.eye_blink_right = Some(0.05);
    s
}

/// Deterministic xorshift for jitter (PIPE-5: no randomness in the pipeline; the stream is
/// fixed by the seed).
struct Rng(u64);

impl Rng {
    fn next_f64(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// Both sides of the comparison: the handle (as Swift calls it) and the core pipeline.
struct Twin {
    handle: Arc<GazePipelineHandle>,
    core: GazePipeline,
    frames: u64,
    t0: f64,
}

impl Twin {
    fn new() -> Self {
        Twin {
            handle: GazePipelineHandle::new(config_json()).unwrap(),
            core: GazePipeline::new(config()),
            frames: 0,
            t0: 100.0,
        }
    }

    fn now(&self) -> f64 {
        self.t0 + self.frames as f64 / HZ
    }

    /// Pushes one frame through both and asserts identical outputs.
    fn push(&mut self, raw: RawTrackingSample) -> ProcessedSample {
        self.frames += 1;
        let via_ffi = self.handle.push(raw.clone().into()).unwrap();
        let direct = self.core.push(&raw);
        assert_eq!(
            ProcessedSample::from(via_ffi.clone()),
            direct,
            "frame at t={}",
            raw.t_sec
        );
        assert_eq!(via_ffi, ProcessedSampleRecord::from(direct.clone()));
        direct
    }

    fn look(&mut self, target: [f64; 2], bias: [f64; 2], eye: Eye) -> ProcessedSample {
        let raw = frame(self.now(), &face(0.40, 0.0), target, bias, eye);
        self.push(raw)
    }

    fn set_recording(&mut self, on: bool) {
        self.handle.set_recording(on);
        self.core.set_recording(on);
    }

    fn assert_same_state(&self) {
        assert_eq!(
            self.handle.calibration_status(),
            self.core.calibration_status().map(Into::into)
        );
        assert_eq!(self.handle.target_complete(), self.core.target_complete());
        assert_eq!(self.handle.advice(), self.core.advice().into());
        assert_eq!(
            self.handle.next_check_target(),
            self.core.next_check_target().into()
        );
        assert_eq!(
            self.handle.valid_ratio().to_bits(),
            self.core.valid_ratio().to_bits()
        );
        assert_eq!(
            self.handle.data_loss().to_bits(),
            self.core.data_loss().to_bits()
        );
        assert_eq!(
            self.handle.calibration_json().unwrap(),
            self.core
                .calibration()
                .map(|m| serde_json::to_string(m).unwrap())
        );
        assert_eq!(
            self.handle.validation_json().unwrap(),
            self.core
                .validation()
                .map(|v| serde_json::to_string(&v).unwrap())
        );
        assert_eq!(
            self.handle.drift_checks_json().unwrap(),
            serde_json::to_string(self.core.drift_checks()).unwrap()
        );
    }

    /// `summary()` JSON through the handle is byte-identical to the core's.
    fn assert_same_summary(&self) -> Value {
        let via_ffi = self.handle.summary_json().unwrap();
        let direct = serde_json::to_string(&self.core.summary()).unwrap();
        assert_eq!(via_ffi, direct, "summary JSON");
        serde_json::from_str(&via_ffi).unwrap()
    }

    /// One target window on both sides, until both report it complete.
    fn run_target(
        &mut self,
        kind: GazeTargetKind,
        target: [f64; 2],
        bias: [f64; 2],
    ) -> GazeTargetResultRecord {
        self.handle.begin_target(kind, target.into()).unwrap();
        self.core.begin_target(kind.into(), target).unwrap();
        for _ in 0..600 {
            self.look(target, bias, Eye::Open);
            assert_eq!(self.handle.target_complete(), self.core.target_complete());
            if self.handle.target_complete() {
                break;
            }
        }
        assert!(self.handle.target_complete(), "window never completed");
        let via_ffi = self.handle.end_target().expect("end_target");
        let direct = self.core.end_target().expect("end_target");
        assert_eq!(via_ffi, direct.into());
        via_ffi
    }

    fn calibrate(&mut self, bias: [f64; 2], reason: Option<GazeRecalReason>) {
        self.handle.start_calibration(None, reason).unwrap();
        self.core
            .start_calibration(None, reason.map(Into::into))
            .unwrap();
        while let Some(GazeCalibrationStatus::Pending { target, .. }) =
            self.handle.calibration_status()
        {
            assert!(matches!(
                self.core.calibration_status(),
                Some(TargetStatus::Pending { .. })
            ));
            let r = self.run_target(GazeTargetKind::Calibration, target.into(), bias);
            assert_eq!(r.kind, GazeTargetKind::Calibration);
        }
        assert_eq!(
            self.handle.calibration_status(),
            Some(GazeCalibrationStatus::Complete)
        );
        let via_ffi = self.handle.finish_calibration().unwrap();
        let direct = self.core.finish_calibration().unwrap();
        assert_eq!(via_ffi.quality, direct.record.quality.into());
        assert_eq!(
            via_ffi.record_json,
            serde_json::to_string(&direct.record).unwrap()
        );
        let fit = direct.fit.as_ref().expect("synthetic calibration fits");
        assert_eq!(
            via_ffi.model_json,
            Some(serde_json::to_string(&fit.selected.model).unwrap())
        );
        assert_eq!(via_ffi.fit_error, None);
        assert_eq!(
            via_ffi.targets,
            direct
                .targets
                .into_iter()
                .map(GazeTargetResultRecord::from)
                .collect::<Vec<_>>()
        );
        assert!(via_ffi.applied, "a good calibration is applied");
        assert_eq!(via_ffi.model_json, self.handle.calibration_json().unwrap());
        self.assert_same_state();
    }

    fn validate(&mut self, bias: [f64; 2]) {
        for t in TrackingQualityParams::default().validation.layout {
            let r = self.run_target(GazeTargetKind::Validation, t, bias);
            assert_eq!(r.kind, GazeTargetKind::Validation);
        }
        self.assert_same_state();
    }
}

fn aois_json() -> String {
    let areas = json!([
        { "name": "face", "startSec": 0.0, "endSec": 30.0,
          "xMin": 0.3, "yMin": 0.2, "xMax": 0.7, "yMax": 0.5, "roles": ["social"] },
        { "name": "toy", "startSec": 0.0, "endSec": 30.0,
          "xMin": 0.1, "yMin": 0.6, "xMax": 0.5, "yMax": 0.9, "roles": ["nonSocial"] }
    ]);
    gaze_clip_aois_json(
        areas.to_string(),
        GazeAoiSpace::Screen,
        9.0 / 16.0,
        0.5,
        GazeVideoGravity::AspectFill,
    )
    .unwrap()
}

/// A recorded clip: scanning between AOIs with saccades, blinks and track loss.
fn play_clip(tw: &mut Twin, rng: &mut Rng, stimulus: &str, seconds: f64, bias: [f64; 2]) {
    let aois = aois_json();
    tw.handle.set_clip_aois_json(aois.clone()).unwrap();
    tw.core.set_clip_aois(serde_json::from_str(&aois).unwrap());
    tw.set_recording(true);
    let n = (seconds * HZ) as u64;
    let mut target = [0.5, 0.35];
    let start = tw.now();
    for i in 0..n {
        if i % 45 == 0 {
            target = [0.1 + 0.8 * rng.next_f64(), 0.1 + 0.8 * rng.next_f64()];
        }
        let jitter = [(rng.next_f64() - 0.5) * 0.01, (rng.next_f64() - 0.5) * 0.01];
        let p = [target[0] + jitter[0], target[1] + jitter[1]];
        let phase = i % 400;
        let eye = match phase {
            100..=107 => Eye::Closed,
            250..=271 => Eye::NoFace,
            _ => Eye::Open,
        };
        let yaw = if (300..330).contains(&phase) {
            25.0
        } else {
            0.0
        };
        let mut raw = frame(
            tw.now(),
            &face(0.40 + 0.02 * rng.next_f64(), yaw),
            p,
            bias,
            eye,
        );
        raw.stimulus_id = Some(stimulus.to_owned());
        raw.clip_media_time_sec = Some(tw.now() - start);
        tw.push(raw);
    }
    tw.set_recording(false);
    tw.handle.set_clip_aois_json("[]".into()).unwrap();
    tw.core.set_clip_aois(Vec::new());
    tw.assert_same_state();
}

fn drift_check(tw: &mut Twin, bias: [f64; 2]) {
    let t: [f64; 2] = tw.handle.next_check_target().into();
    tw.run_target(GazeTargetKind::DriftCheck, t, bias);
    tw.assert_same_state();
}

// ------------------------------------------------------------------------------------------
// Records
// ------------------------------------------------------------------------------------------

fn full_raw() -> RawTrackingSample {
    let m = |k: f64| -> M4 { std::array::from_fn(|i| k + i as f64 * 0.5) };
    RawTrackingSample {
        t_sec: 12.5,
        face_tracked: true,
        face_transform: Some(m(1.0)),
        look_at: Some([0.1, 0.2, 0.3]),
        left_eye_transform: Some(m(20.0)),
        right_eye_transform: Some(m(40.0)),
        eye_blink_left: Some(0.11),
        eye_blink_right: Some(0.22),
        ambient_intensity: Some(950.0),
        ambient_color_temp: Some(5600.0),
        light_direction: Some([0.4, 0.5, 0.6]),
        orientation: Orientation::LandscapeRight,
        clip_media_time_sec: Some(3.25),
        stimulus_id: Some("clip-7".into()),
    }
}

#[test]
fn raw_record_round_trips_every_field() {
    let raw = full_raw();
    let rec = RawSampleRecord::from(raw.clone());
    assert_eq!(
        rec.face_transform.as_deref(),
        Some(&raw.face_transform.unwrap()[..])
    );
    assert_eq!(
        rec.look_at,
        Some(GazeVec3 {
            x: 0.1,
            y: 0.2,
            z: 0.3
        })
    );
    assert_eq!(rec.orientation, GazeOrientation::LandscapeRight);
    assert_eq!(RawTrackingSample::try_from(rec).unwrap(), raw);

    // Every optional field absent.
    let bare = RawTrackingSample {
        t_sec: 0.0,
        face_tracked: false,
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
    };
    assert_eq!(
        RawTrackingSample::try_from(RawSampleRecord::from(bare.clone())).unwrap(),
        bare
    );

    // JSON of the core type equals JSON after a record round trip (all fields, all names).
    let via = RawTrackingSample::try_from(RawSampleRecord::from(raw.clone())).unwrap();
    assert_eq!(
        serde_json::to_value(&via).unwrap(),
        serde_json::to_value(&raw).unwrap()
    );
    assert_eq!(
        serde_json::to_value(&raw)
            .unwrap()
            .as_object()
            .unwrap()
            .len(),
        14
    );
}

#[test]
fn every_orientation_maps_both_ways() {
    for (o, g) in [
        (Orientation::Portrait, GazeOrientation::Portrait),
        (
            Orientation::PortraitUpsideDown,
            GazeOrientation::PortraitUpsideDown,
        ),
        (Orientation::LandscapeLeft, GazeOrientation::LandscapeLeft),
        (Orientation::LandscapeRight, GazeOrientation::LandscapeRight),
    ] {
        assert_eq!(GazeOrientation::from(o), g);
        assert_eq!(Orientation::from(g), o);
    }
}

#[test]
fn processed_record_round_trips_every_field() {
    let p = ProcessedSample {
        t_sec: 1.0,
        track_state: TrackState::Offscreen,
        ray_status: Some(RayStatus::Grazing),
        raw_point_norm: Some([1.1, -0.2]),
        calibrated_point_norm: Some([1.2, -0.3]),
        filtered_point_norm: Some([0.4, 0.5]),
        eye_pos_mm: Some([10.0, 20.0, 400.0]),
        distance_cm: Some(40.0),
        head_yaw_rad: Some(0.1),
        head_pitch_rad: Some(-0.2),
        head_roll_rad: Some(0.3),
        head_speed_deg_per_sec: Some(12.0),
        eye_speed_deg_per_sec: Some(80.0),
        left_eye_openness: Some(0.9),
        right_eye_openness: Some(0.8),
        ambient_intensity: Some(700.0),
        confidence: 0.75,
        valid: true,
        gap_filled: true,
        active_aoi_names: vec!["face".into(), "toy".into()],
        stimulus_id: Some("clip-1".into()),
        clip_media_time_sec: Some(2.5),
    };
    let rec = ProcessedSampleRecord::from(p.clone());
    assert_eq!(rec.track_state, GazeTrackState::Offscreen);
    assert_eq!(rec.ray_status, Some(GazeRayStatus::Grazing));
    assert_eq!(rec.raw_point_norm, Some(GazeVec2 { x: 1.1, y: -0.2 }));
    assert_eq!(ProcessedSample::from(rec), p);
    let json = serde_json::to_value(&p).unwrap();
    assert_eq!(json.as_object().unwrap().len(), 22, "every field covered");

    for (s, r) in [
        (TrackState::Tracked, GazeTrackState::Tracked),
        (TrackState::Blink, GazeTrackState::Blink),
        (TrackState::Lost, GazeTrackState::Lost),
        (TrackState::Offscreen, GazeTrackState::Offscreen),
        (TrackState::Artifact, GazeTrackState::Artifact),
    ] {
        assert_eq!(GazeTrackState::from(s), r);
        assert_eq!(TrackState::from(r), s);
    }
    for (s, r) in [
        (RayStatus::Ok, GazeRayStatus::Ok),
        (RayStatus::Backward, GazeRayStatus::Backward),
        (RayStatus::Grazing, GazeRayStatus::Grazing),
        (RayStatus::OutOfRange, GazeRayStatus::OutOfRange),
    ] {
        assert_eq!(GazeRayStatus::from(s), r);
        assert_eq!(RayStatus::from(r), s);
    }
}

// ------------------------------------------------------------------------------------------
// Robustness: errors, never panics
// ------------------------------------------------------------------------------------------

fn is_invalid_json<T: std::fmt::Debug>(r: Result<T, AsdError>) -> bool {
    matches!(r, Err(AsdError::InvalidJson { .. }))
}

fn is_invalid_argument<T: std::fmt::Debug>(r: Result<T, AsdError>) -> bool {
    matches!(r, Err(AsdError::InvalidArgument { .. }))
}

#[test]
fn malformed_config_json_is_an_error_not_a_panic() {
    for bad in [
        "",
        "not json",
        "{",
        "[]",
        "null",
        "{}",
        r#"{"modelIdentifier": 5}"#,
        r#"{"modelIdentifier":"x","geometrySource":"table","orientation":"sideways"}"#,
        "\u{0}",
    ] {
        assert!(
            is_invalid_json(GazePipelineHandle::new(bad.into())),
            "config {bad:?}"
        );
    }
    // A config missing `orientation` is wrong-shaped too.
    let mut v = serde_json::to_value(config()).unwrap();
    v.as_object_mut().unwrap().remove("orientation");
    assert!(is_invalid_json(GazePipelineHandle::new(v.to_string())));
    // The default config for any model string decodes (unknown model: idiom default).
    for model in ["iPhone15,2", "iPad13,4", "", "unknown\u{1F600}"] {
        let j = gaze_default_config_json(model.into(), GazeOrientation::Portrait).unwrap();
        let h = GazePipelineHandle::new(j.clone()).unwrap();
        let back: PipelineConfig = serde_json::from_str(&h.config_json().unwrap()).unwrap();
        assert_eq!(back, serde_json::from_str::<PipelineConfig>(&j).unwrap());
    }
}

#[test]
fn other_malformed_json_is_an_error() {
    let h = GazePipelineHandle::new(config_json()).unwrap();
    assert!(is_invalid_json(h.set_calibration_json(Some("{".into()))));
    assert!(is_invalid_json(
        h.set_calibration_json(Some(r#"{"kind":"nope"}"#.into()))
    ));
    assert!(is_invalid_json(h.set_clip_aois_json("{}".into())));
    assert!(is_invalid_json(h.set_clip_aois_json("".into())));
    assert!(is_invalid_json(GazeTelemetryWriterHandle::new(
        0.0,
        Some("[1]".into())
    )));
    assert!(is_invalid_json(gaze_clip_aois_json(
        "{}".into(),
        GazeAoiSpace::Video,
        0.5625,
        0.5,
        GazeVideoGravity::AspectFill
    )));
    // A valid model and `None` are accepted.
    h.set_calibration_json(Some(
        r#"{"kind":"screenAffine","x":[1,0,0.1],"y":[0,1,0]}"#.into(),
    ))
    .unwrap();
    assert!(h.calibration_json().unwrap().is_some());
    h.set_calibration_json(None).unwrap();
    assert_eq!(h.calibration_json().unwrap(), None);
}

#[test]
fn wrong_length_matrices_are_invalid_argument() {
    let h = GazePipelineHandle::new(config_json()).unwrap();
    let good = RawSampleRecord::from(full_raw());
    for len in [0usize, 1, 15, 17, 32] {
        let mut r = good.clone();
        r.face_transform = Some(vec![0.0; len]);
        assert!(is_invalid_argument(h.push(r)), "face {len}");
        let mut r = good.clone();
        r.left_eye_transform = Some(vec![0.0; len]);
        assert!(is_invalid_argument(h.push(r)), "left eye {len}");
        let mut r = good.clone();
        r.right_eye_transform = Some(vec![0.0; len]);
        assert!(is_invalid_argument(h.push(r)), "right eye {len}");
    }
    // A rejected frame changes nothing: the next frame is the first one.
    let core = GazePipeline::new(config());
    let mut core = core;
    let raw = frame(0.0, &face(0.4, 0.0), [0.5, 0.5], [0.0, 0.0], Eye::Open);
    assert_eq!(
        ProcessedSample::from(h.push(raw.clone().into()).unwrap()),
        core.push(&raw)
    );
}

#[test]
fn non_finite_and_absurd_input_does_not_panic() {
    let h = GazePipelineHandle::new(config_json()).unwrap();
    let w = GazeTelemetryWriterHandle::new(0.0, None).unwrap();
    let specials = [
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        0.0,
        -0.0,
        1e308,
        -1e308,
    ];
    let mut t = 0.0;
    for &x in &specials {
        for case in 0..6 {
            let mut r =
                RawSampleRecord::from(frame(t, &face(0.4, 0.0), [0.5, 0.5], [0.0, 0.0], Eye::Open));
            match case {
                0 => r.t_sec = x,
                1 => r.face_transform = Some(vec![x; 16]),
                2 => r.look_at = Some(GazeVec3 { x, y: x, z: x }),
                3 => {
                    r.eye_blink_left = Some(x);
                    r.eye_blink_right = Some(x);
                }
                4 => r.left_eye_transform = Some(vec![x; 16]),
                _ => {
                    r.ambient_intensity = Some(x);
                    r.clip_media_time_sec = Some(x);
                }
            }
            let out = h.push(r).unwrap();
            let _ = w.write(out);
            t += 1.0 / HZ;
        }
    }
    // Time going backwards, duplicate timestamps, recording toggles, orientation changes.
    h.set_recording(true);
    for (i, t) in [5.0, 5.0, 4.0, 1e9, -1.0].into_iter().enumerate() {
        if i == 2 {
            h.set_orientation(GazeOrientation::LandscapeLeft);
        }
        let _ = h
            .push(frame(t, &face(0.4, 0.0), [0.5, 0.5], [0.0, 0.0], Eye::Open).into())
            .unwrap();
    }
    let _ = h.summary_json().unwrap();
    let _ = h.advice();
    let _ = h.gap_fill(Vec::new());
    let _ = h.valid_ratio();
    let _ = h.data_loss();
    // Absurd layouts and targets.
    let _ = h.start_calibration(Some(Vec::new()), None);
    let _ = h.calibration_status();
    let _ = h.finish_calibration();
    let _ = h.start_calibration(
        Some(vec![GazeVec2 {
            x: f64::NAN,
            y: f64::INFINITY,
        }]),
        Some(GazeRecalReason::Drift),
    );
    let _ = h.begin_target(
        GazeTargetKind::Calibration,
        GazeVec2 {
            x: f64::NAN,
            y: f64::NAN,
        },
    );
    let _ = h.end_target();
    let _ = h.finish_calibration();
    let _ = h.begin_target(
        GazeTargetKind::Validation,
        GazeVec2 {
            x: f64::NAN,
            y: 1e300,
        },
    );
    let _ = h
        .push(frame(2e9, &face(0.4, 0.0), [0.5, 0.5], [0.0, 0.0], Eye::Open).into())
        .unwrap();
    let _ = h.target_complete();
    let _ = h.end_target();
    let _ = h.summary_json().unwrap();
    let _ = gaze_clip_aois_json(
        aois_json_areas(),
        GazeAoiSpace::Video,
        f64::NAN,
        0.0,
        GazeVideoGravity::AspectFit,
    )
    .unwrap();
}

fn aois_json_areas() -> String {
    json!([{ "name": "a", "startSec": 0.0, "endSec": 1.0,
             "xMin": 0.0, "yMin": 0.0, "xMax": 1.0, "yMax": 1.0, "roles": [] }])
    .to_string()
}

#[test]
fn target_api_misuse_is_invalid_argument() {
    let h = GazePipelineHandle::new(config_json()).unwrap();
    // No calibration running.
    assert!(is_invalid_argument(h.begin_target(
        GazeTargetKind::Calibration,
        GazeVec2 { x: 0.5, y: 0.5 }
    )));
    assert!(is_invalid_argument(h.finish_calibration()));
    assert_eq!(h.end_target(), None);
    assert!(!h.target_complete());
    // Calibration target that is not the pending one.
    h.start_calibration(None, None).unwrap();
    let Some(GazeCalibrationStatus::Pending {
        target,
        index,
        attempt,
    }) = h.calibration_status()
    else {
        panic!("pending target expected");
    };
    assert_eq!((index, attempt), (0, 1));
    let wrong = GazeVec2 {
        x: target.x + 0.25,
        y: target.y,
    };
    match h.begin_target(GazeTargetKind::Calibration, wrong) {
        Err(AsdError::InvalidArgument { message }) => {
            assert!(message.contains("expects target"), "{message}")
        }
        other => panic!("{other:?}"),
    }
    // A window already open.
    h.begin_target(GazeTargetKind::Calibration, target).unwrap();
    assert!(is_invalid_argument(
        h.begin_target(GazeTargetKind::Validation, target)
    ));
    assert!(is_invalid_argument(h.start_calibration(None, None)));
    assert!(is_invalid_argument(h.finish_calibration()));
    assert!(matches!(
        h.calibration_status(),
        Some(GazeCalibrationStatus::Running { .. }) | Some(GazeCalibrationStatus::Pending { .. })
    ));
}

#[test]
fn handle_is_send_and_sync_and_usable_from_threads() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<GazePipelineHandle>();
    assert_send_sync::<GazeTelemetryWriterHandle>();
    let h = GazePipelineHandle::new(config_json()).unwrap();
    let threads: Vec<_> = (0..4)
        .map(|k| {
            let h = Arc::clone(&h);
            std::thread::spawn(move || {
                for i in 0..50 {
                    let t = (k * 50 + i) as f64 / HZ;
                    h.push(frame(t, &face(0.4, 0.0), [0.5, 0.5], [0.0, 0.0], Eye::Open).into())
                        .unwrap();
                }
            })
        })
        .collect();
    for t in threads {
        t.join().unwrap();
    }
    let _ = h.summary_json().unwrap();
}

// ------------------------------------------------------------------------------------------
// Parity: the handle equals the core pipeline
// ------------------------------------------------------------------------------------------

#[test]
fn scripted_session_through_the_handle_equals_the_core() {
    let bias = [3.0, -2.0];
    let mut tw = Twin::new();
    let mut rng = Rng(0x5EED_1234_ABCD_0001);
    // Positioning: not recorded, some loss.
    for i in 0..90 {
        let eye = if (30..40).contains(&i) {
            Eye::NoFace
        } else {
            Eye::Open
        };
        tw.look([0.5, 0.5], bias, eye);
    }
    tw.assert_same_state();
    tw.calibrate(bias, None);
    tw.validate(bias);
    play_clip(&mut tw, &mut rng, "clip-social-1", 20.0, bias);
    drift_check(&mut tw, bias);
    play_clip(&mut tw, &mut rng, "clip-nonsocial-2", 20.0, bias);
    drift_check(&mut tw, bias);
    // Operator re-calibration with a different bias (drift), then validation and a clip.
    let bias2 = [5.0, 1.0];
    tw.calibrate(bias2, Some(GazeRecalReason::OperatorRequest));
    tw.validate(bias2);
    play_clip(&mut tw, &mut rng, "clip-social-3", 15.0, bias2);

    let summary = tw.assert_same_summary();
    assert_eq!(summary["calibration"]["attempts"], json!(2));
    assert_eq!(summary["recalibrations"].as_array().unwrap().len(), 1);
    assert!(summary["validation"]["accuracyDeg"].is_number());
    assert_eq!(summary["driftChecks"].as_array().unwrap().len(), 2);
    let loss = &summary["loss"];
    assert!(loss["blinkSec"].as_f64().unwrap() > 0.0, "{loss}");
    assert!(loss["trackLossSec"].as_f64().unwrap() > 0.0, "{loss}");
    assert!(tw.handle.valid_ratio() > 0.5 && tw.handle.valid_ratio() < 1.0);
}

#[test]
fn recorded_frames_gap_fill_and_telemetry_match_the_core() {
    let bias = [2.0, 1.0];
    let mut tw = Twin::new();
    let mut rng = Rng(42);
    tw.calibrate(bias, None);
    tw.set_recording(true);
    let mut processed = Vec::new();
    let mut records = Vec::new();
    let mut target = [0.5, 0.5];
    for i in 0..600u64 {
        // Short losses (fillable: <= 75 ms, steady gaze) and a long one (not fillable).
        let eye = match i % 120 {
            50..=52 => Eye::NoFace,
            70..=85 => Eye::NoFace,
            100..=102 => Eye::Closed,
            _ => Eye::Open,
        };
        if i % 120 == 0 {
            target = [0.3 + 0.4 * rng.next_f64(), 0.3 + 0.4 * rng.next_f64()];
        }
        let p = target;
        let raw = frame(
            tw.now(),
            &face(0.4, 8.0 * (i as f64 / 50.0).sin()),
            p,
            bias,
            eye,
        );
        tw.frames += 1;
        let rec = tw.handle.push(raw.clone().into()).unwrap();
        processed.push(tw.core.push(&raw));
        records.push(rec);
    }
    assert_eq!(
        records
            .iter()
            .cloned()
            .map(ProcessedSample::from)
            .collect::<Vec<_>>(),
        processed
    );
    // Gap fill through the handle equals the core.
    let filled_ffi = tw.handle.gap_fill(records.clone());
    let filled_core = tw.core.gap_fill(&processed);
    assert_eq!(
        filled_ffi
            .iter()
            .cloned()
            .map(ProcessedSample::from)
            .collect::<Vec<_>>(),
        filled_core
    );
    assert!(filled_ffi.iter().any(|s| s.gap_filled), "short gaps filled");
    // Telemetry writer through the handle equals the core writer + gate.
    let gate = QualityGate::default();
    let w =
        GazeTelemetryWriterHandle::new(100.0, Some(serde_json::to_string(&gate).unwrap())).unwrap();
    let mut core_w = TelemetryWriter::new(100.0, gate);
    for (rec, p) in filled_ffi.into_iter().zip(&filled_core) {
        let got = w.write(rec);
        let s = core_w.sample(p);
        let e = core_w.evaluate(&s);
        let want = json!({
            "timestamp_ms": s.timestamp_ms, "video_time_sec": s.video_time_sec,
            "x_norm": s.x_norm, "y_norm": s.y_norm, "confidence": s.confidence,
            "left_eye_openness": s.left_eye_openness, "right_eye_openness": s.right_eye_openness,
            "head_pitch_rad": s.head_pitch_rad, "head_yaw_rad": s.head_yaw_rad,
            "head_roll_rad": s.head_roll_rad, "distance_to_screen_cm": s.distance_to_screen_cm,
            "valid": s.is_valid, "stimulus_id": s.stimulus_id,
            "clip_media_time_sec": s.clip_media_time_sec,
            "verdict": serde_json::to_value(e.verdict).unwrap(),
            "failure_reasons": e.failure_reasons, "is_blink": e.is_blink,
        });
        let have = json!({
            "timestamp_ms": got.timestamp_ms, "video_time_sec": got.video_time_sec,
            "x_norm": got.x_norm, "y_norm": got.y_norm, "confidence": got.confidence,
            "left_eye_openness": got.left_eye_openness,
            "right_eye_openness": got.right_eye_openness,
            "head_pitch_rad": got.head_pitch_rad, "head_yaw_rad": got.head_yaw_rad,
            "head_roll_rad": got.head_roll_rad,
            "distance_to_screen_cm": got.distance_to_screen_cm,
            "valid": got.is_valid, "stimulus_id": got.stimulus_id,
            "clip_media_time_sec": got.clip_media_time_sec,
            "verdict": got.quality_verdict,
            "failure_reasons": got.failure_reasons, "is_blink": got.is_blink,
        });
        assert_eq!(have, want);
    }
    tw.set_recording(false);
    tw.assert_same_summary();
}

/// 10 minutes at 60 Hz (36 000 frames) of mixed tracking through both, frame by frame, with
/// orientation changes; summary JSON byte-identical.
#[test]
fn ten_minute_stream_through_the_handle_equals_the_core() {
    let bias = [2.5, -1.5];
    let mut tw = Twin::new();
    let mut rng = Rng(0xDEC0_DE00_0000_0010);
    tw.calibrate(bias, None);
    tw.validate(bias);
    tw.set_recording(true);
    let frames = 10 * 60 * 60;
    let mut target = [0.5, 0.5];
    for i in 0..frames {
        if i % 30 == 0 {
            target = [rng.next_f64() * 1.2 - 0.1, rng.next_f64() * 1.2 - 0.1];
        }
        let r = rng.next_f64();
        let eye = if r < 0.02 {
            Eye::NoFace
        } else if r < 0.04 {
            Eye::Closed
        } else {
            Eye::Open
        };
        let yaw = (i as f64 / 600.0).sin() * 20.0;
        let mut raw = frame(
            tw.now(),
            &face(0.35 + 0.1 * rng.next_f64(), yaw),
            target,
            bias,
            eye,
        );
        // Orientation changes mid-stream (GEO-3: the raw sample's orientation wins).
        if (20_000..21_000).contains(&i) {
            raw.orientation = Orientation::PortraitUpsideDown;
        }
        raw.stimulus_id = Some(format!("clip-{}", i / 3600));
        tw.push(raw);
    }
    tw.set_recording(false);
    tw.assert_same_state();
    let s = tw.assert_same_summary();
    assert_eq!(s["calibration"]["attempts"], json!(1));
}

#[test]
fn default_config_and_clip_aois_match_the_core() {
    let j = gaze_default_config_json("iPhone15,2".into(), GazeOrientation::LandscapeLeft).unwrap();
    let want = PipelineConfig::for_model("iPhone15,2", Orientation::LandscapeLeft);
    assert_eq!(serde_json::from_str::<PipelineConfig>(&j).unwrap(), want);

    let areas = aois_json_areas();
    let parsed: Vec<asd_core::types::catalog::Area> = serde_json::from_str(&areas).unwrap();
    for (space, gravity) in [
        (GazeAoiSpace::Screen, GazeVideoGravity::AspectFill),
        (GazeAoiSpace::Video, GazeVideoGravity::AspectFill),
        (GazeAoiSpace::Video, GazeVideoGravity::AspectFit),
    ] {
        let got = gaze_clip_aois_json(areas.clone(), space, 9.0 / 16.0, 0.46, gravity).unwrap();
        let want = asd_core::gaze::aoi_space::clip_aois(
            &parsed,
            space.into(),
            9.0 / 16.0,
            0.46,
            gravity.into(),
        );
        assert_eq!(got, serde_json::to_string(&want).unwrap());
    }
}

#[test]
fn failed_calibration_is_reported_not_applied() {
    // Every calibration frame without a face: every target is missed.
    let h = GazePipelineHandle::new(config_json()).unwrap();
    let mut core = GazePipeline::new(config());
    h.start_calibration(None, None).unwrap();
    core.start_calibration(None, None).unwrap();
    let mut t = 0.0;
    let mut guard = 0;
    while let Some(GazeCalibrationStatus::Pending { target, .. }) = h.calibration_status() {
        h.begin_target(GazeTargetKind::Calibration, target).unwrap();
        core.begin_target(TargetKind::Calibration, target.into())
            .unwrap();
        while !h.target_complete() {
            let raw = frame(t, &face(0.4, 0.0), [0.5, 0.5], [0.0, 0.0], Eye::NoFace);
            assert_eq!(
                ProcessedSample::from(h.push(raw.clone().into()).unwrap()),
                core.push(&raw)
            );
            t += 1.0 / HZ;
            guard += 1;
            assert!(guard < 100_000, "calibration windows never close");
        }
        assert_eq!(h.end_target(), core.end_target().map(Into::into));
    }
    let out = h.finish_calibration().unwrap();
    let direct = core.finish_calibration().unwrap();
    assert_eq!(out.quality, GazeCalibrationQuality::Failed);
    assert_eq!(out.quality, direct.record.quality.into());
    assert!(!out.applied);
    assert!(out.model_json.is_none());
    assert!(out.fit_error.is_some());
    assert_eq!(h.calibration_json().unwrap(), None);
    let advice = h.advice();
    assert_eq!(advice, core.advice().into());
    assert_eq!(advice.action, GazeRecalAction::Recalibrate);
    assert!(advice
        .reasons
        .contains(&GazeRecalReason::CalibrationQuality));
    assert_eq!(
        h.summary_json().unwrap(),
        serde_json::to_string(&core.summary()).unwrap()
    );
    let _ = RecalReason::OperatorRequest;
}

// ------------------------------------------------------------------------------------------
// Cost (PIPE-4 is measured on device through the FFI in WP-G.9; this is the host figure)
// ------------------------------------------------------------------------------------------

#[test]
#[ignore = "timing; run with --ignored --nocapture (release for meaningful numbers)"]
fn per_frame_cost_through_the_handle() {
    let h = GazePipelineHandle::new(config_json()).unwrap();
    h.set_calibration_json(Some(
        r#"{"kind":"screenAffine","x":[1,0,0.01],"y":[0,1,0]}"#.into(),
    ))
    .unwrap();
    h.set_clip_aois_json(aois_json()).unwrap();
    h.set_recording(true);
    let mut rng = Rng(7);
    let n = 36_000;
    let raws: Vec<RawSampleRecord> = (0..n)
        .map(|i| {
            let p = [rng.next_f64(), rng.next_f64()];
            let mut r = frame(i as f64 / HZ, &face(0.4, 0.0), p, [1.0, 1.0], Eye::Open);
            r.clip_media_time_sec = Some(i as f64 / HZ);
            r.into()
        })
        .collect();
    let mut times = Vec::with_capacity(n);
    for r in raws {
        let start = std::time::Instant::now();
        let out = h.push(r).unwrap();
        times.push(start.elapsed().as_secs_f64() * 1e3);
        std::hint::black_box(out);
    }
    times.sort_by(f64::total_cmp);
    let q = |p: f64| times[((times.len() - 1) as f64 * p) as usize];
    println!(
        "GazePipelineHandle::push over {n} frames: median {:.4} ms, p99 {:.4} ms, max {:.4} ms",
        q(0.5),
        q(0.99),
        q(1.0)
    );
    let start = std::time::Instant::now();
    let _ = h.summary_json().unwrap();
    println!(
        "summary_json after {n} recorded frames: {:.3} ms",
        start.elapsed().as_secs_f64() * 1e3
    );
}
