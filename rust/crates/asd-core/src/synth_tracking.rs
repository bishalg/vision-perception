//! owner: WP-G.5
//!
//! TV-1: seeded generator of raw tracking logs (REP-1) from a known gaze script.
//!
//! A [`Spec`] describes one session: a 5-point calibration, a validation, `clips` stimulus
//! clips (each with a free-viewing fixation script) and, optionally, a drift-check target
//! after every clip. The "user" looks at the scripted screen point; the generator builds what
//! ARKit would deliver for that gaze: a face anchor `distance` in front of the screen centre
//! (yaw / pitch, sway), eye transforms, and a `look_at` point whose direction is the true
//! gaze direction in the face frame, distorted by
//!
//! - a per-user angular **bias** and **gain** (`measured = gain * true + bias`, yaw and pitch),
//! - linear **drift** of that bias after the validation (deg per second),
//! - seeded Gaussian **noise** and rare **outliers** (glasses-like reflections),
//! - **blinks** (blend shapes 0.9), **track-loss** runs (no face), single-frame **spikes**,
//! - **dropped** frames (a validation target for which the log has no frames at all),
//! - a head **pose change** between calibration and validation (distance, yaw, pitch).
//!
//! The coordinate conventions are `gaze::raycast::CAMERA_AXES` (metres, device-fixed portrait
//! native frame, +x right, +y towards the camera edge, viewer at z < 0, screen plane z = 0).
//! The header carries the ground truth (`groundTruth`), so a log can be checked against what
//! the generator put in.
//!
//! # Determinism
//! Output depends only on `(spec, label, seed)`. The RNG is ChaCha20 seeded with
//! `sha256("asd_tracking_synth:{GENERATOR_VERSION}:{label}:{seed}")`; uniform and Gaussian
//! draws are implemented here. The frame clock is `T0 + n / fps` (no accumulation), and every
//! emitted number is rounded to 6 or 7 decimals, so the canonical bytes are stable.
//! Nothing here reads the wall clock.
//!
//! # Simplifications
//! - Saccades take 50 ms (smoothstep), so scripted jumps stay below the artifact speed.
//! - Head roll is 0 in every orientation (the user is not rotated with the device), which
//!   leaves the confidence pose factors at their portrait values.
//! - The window of a gap run is counted in frames: `floor(durSec * fps)` frames are lost, so
//!   "60 ms" at 60 Hz is 3 lost frames (a 66.7 ms bracket, inside the 75 ms fill limit) and
//!   "200 ms" is 12 frames.

use std::fmt;
use std::str::FromStr;

use rand_chacha::rand_core::{RngCore, SeedableRng};
use rand_chacha::ChaCha20Rng;
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::gaze::calibration::oriented_to_native_mm;
use crate::gaze::geometry::{lookup, OrientedGeometry};
use crate::gaze::raw_log::{to_jsonl, RawLog};
use crate::types::tracking::{
    Orientation, RawLogHeader, RawTrackingSample, ScheduledTarget, TargetKind,
    TrackingQualityParams, TrackingSource, RAW_LOG_FORMAT,
};

pub const GENERATOR_VERSION: &str = "1.0";
/// Clock origin of every synthetic log (same as the committed sample fixture).
pub const T0_SEC: f64 = 100.0;

/// Face-frame midpoint between the eyes.
const EYE_MID_FACE: [f64; 3] = [0.0, 0.025, 0.01];
const EYE_HALF_SPACING_M: f64 = 0.031;
/// Length of the `look_at` vector in the face frame, metres.
const LOOK_AT_LEN_M: f64 = 0.5;
/// Gaze arrives at a target this long before its window opens.
const LEAD_SEC: f64 = 0.15;
const SACCADE_SEC: f64 = 0.05;
/// Pause between consecutive windows, seconds.
const GAP_SEC: f64 = 0.3;
/// Positioning pause between calibration and validation; the pose change happens in it.
const POSE_RAMP_SEC: f64 = 1.0;
const SPIKE_DEG: f64 = 40.0;

/// Head pose of the user.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pose {
    pub distance_m: f64,
    pub yaw_deg: f64,
    pub pitch_deg: f64,
}

/// What happens at a clip-relative time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventKind {
    /// `face_tracked = false` for `floor(durSec * fps)` frames.
    Loss,
    /// Blink blend shapes at 0.9 for `round(durSec * fps)` frames (at least one).
    Blink,
    /// One frame whose `look_at` is rotated by 40 degrees of yaw.
    Spike,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClipEvent {
    pub clip: usize,
    pub start_sec: f64,
    pub dur_sec: f64,
    pub kind: EventKind,
}

/// One synthetic session.
#[derive(Debug, Clone, PartialEq)]
pub struct Spec {
    pub model_identifier: String,
    pub orientation: Orientation,
    pub fps: f64,
    pub condition: String,
    pub ambient_lux: f64,
    /// Per-user eye-model bias `[yaw, pitch]`, degrees.
    pub bias_deg: [f64; 2],
    /// Per-user eye-model gain `[yaw, pitch]`.
    pub gain: [f64; 2],
    /// Added to the bias per second after the validation ends, degrees.
    pub drift_deg_per_sec: [f64; 2],
    /// Gaussian angular noise per sample, degrees (per axis).
    pub noise_sd_deg: f64,
    /// Probability that a sample is an outlier, and its size (random direction), degrees.
    pub outlier_prob: f64,
    pub outlier_deg: f64,
    /// Pose during calibration; the validation and everything after uses `later_pose`.
    pub cal_pose: Pose,
    pub later_pose: Pose,
    /// Yaw sway on top of the pose, degrees and hertz (0 = still).
    pub sway_deg: f64,
    pub sway_hz: f64,
    /// A blink every this many seconds inside each clip (100-400 ms each).
    pub blink_period_sec: Option<f64>,
    pub events: Vec<ClipEvent>,
    pub clips: usize,
    pub clip_sec: f64,
    pub drift_checks: bool,
    /// Validate on the 13-point bench grid (VAL-5) instead of the 4-point app layout.
    pub grid13: bool,
    /// Index of a validation target whose frames are absent from the log.
    pub drop_validation_target: Option<usize>,
}

impl Spec {
    /// A steady, well-behaved user on an iPhone 16 in portrait at 60 Hz.
    pub fn base() -> Self {
        let pose = Pose {
            distance_m: 0.45,
            yaw_deg: 0.0,
            pitch_deg: 0.0,
        };
        Spec {
            model_identifier: "iPhone17,3".into(),
            orientation: Orientation::Portrait,
            fps: 60.0,
            condition: "baseline".into(),
            ambient_lux: 400.0,
            bias_deg: [0.0, 0.0],
            gain: [1.0, 1.0],
            drift_deg_per_sec: [0.0, 0.0],
            noise_sd_deg: 0.05,
            outlier_prob: 0.0,
            outlier_deg: 0.0,
            cal_pose: pose,
            later_pose: pose,
            sway_deg: 0.0,
            sway_hz: 0.4,
            blink_period_sec: None,
            events: Vec::new(),
            clips: 2,
            clip_sec: 6.0,
            drift_checks: true,
            grid13: false,
            drop_validation_target: None,
        }
    }
}

/// The TV-2 scenarios (plus `glasses_noise` and `head_sway`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Scenario {
    Ideal,
    BiasOnly,
    BiasGain,
    DistanceShiftAfterCal,
    DistanceShiftLarge,
    HeadYaw15,
    Drift3DegLinear,
    BlinksAndLoss,
    Gap60msVs200ms,
    ArtifactSpike,
    OrientationLandscapeIpad,
    Fps30,
    MissingTarget,
    GlassesNoise,
    HeadSway,
}

pub const SCENARIOS: [Scenario; 15] = [
    Scenario::Ideal,
    Scenario::BiasOnly,
    Scenario::BiasGain,
    Scenario::DistanceShiftAfterCal,
    Scenario::DistanceShiftLarge,
    Scenario::HeadYaw15,
    Scenario::Drift3DegLinear,
    Scenario::BlinksAndLoss,
    Scenario::Gap60msVs200ms,
    Scenario::ArtifactSpike,
    Scenario::OrientationLandscapeIpad,
    Scenario::Fps30,
    Scenario::MissingTarget,
    Scenario::GlassesNoise,
    Scenario::HeadSway,
];

impl Scenario {
    pub fn name(self) -> &'static str {
        match self {
            Scenario::Ideal => "ideal",
            Scenario::BiasOnly => "bias_only",
            Scenario::BiasGain => "bias_gain",
            Scenario::DistanceShiftAfterCal => "distance_shift_after_cal",
            Scenario::DistanceShiftLarge => "distance_shift_large",
            Scenario::HeadYaw15 => "head_yaw_15",
            Scenario::Drift3DegLinear => "drift_3deg_linear",
            Scenario::BlinksAndLoss => "blinks_and_loss",
            Scenario::Gap60msVs200ms => "gap_60ms_vs_200ms",
            Scenario::ArtifactSpike => "artifact_spike",
            Scenario::OrientationLandscapeIpad => "orientation_landscape_ipad",
            Scenario::Fps30 => "fps_30",
            Scenario::MissingTarget => "missing_target",
            Scenario::GlassesNoise => "glasses_noise",
            Scenario::HeadSway => "head_sway",
        }
    }

    /// The generator settings of the scenario.
    pub fn spec(self) -> Spec {
        let mut s = Spec::base();
        s.condition = self.name().into();
        let ev = |clip, start_sec, dur_sec, kind| ClipEvent {
            clip,
            start_sec,
            dur_sec,
            kind,
        };
        match self {
            Scenario::Ideal => {}
            Scenario::BiasOnly => s.bias_deg = [2.0, -1.5],
            Scenario::BiasGain => {
                s.bias_deg = [1.5, -1.0];
                s.gain = [1.15, 0.9];
            }
            Scenario::DistanceShiftAfterCal => {
                s.bias_deg = [1.5, -1.0];
                s.later_pose.distance_m = 0.52;
            }
            Scenario::DistanceShiftLarge => {
                s.bias_deg = [1.5, -1.0];
                s.later_pose.distance_m = 0.60;
            }
            Scenario::HeadYaw15 => {
                s.bias_deg = [1.0, -0.5];
                s.later_pose.yaw_deg = 15.0;
            }
            Scenario::Drift3DegLinear => {
                s.bias_deg = [1.0, 0.0];
                s.drift_deg_per_sec = [0.09, 0.0];
                s.clips = 4;
            }
            Scenario::BlinksAndLoss => {
                s.blink_period_sec = Some(2.0);
                s.events = vec![
                    ev(0, 1.0, 0.5, EventKind::Loss),
                    ev(1, 3.5, 0.8, EventKind::Loss),
                ];
            }
            Scenario::Gap60msVs200ms => {
                s.clips = 1;
                s.drift_checks = false;
                s.events = vec![
                    ev(0, 1.0, 0.05, EventKind::Loss),
                    ev(0, 3.0, 0.2, EventKind::Loss),
                ];
            }
            Scenario::ArtifactSpike => {
                s.events = vec![
                    ev(0, 1.0, 0.0, EventKind::Spike),
                    ev(0, 2.5, 0.0, EventKind::Spike),
                    ev(0, 4.0, 0.0, EventKind::Spike),
                ];
            }
            Scenario::OrientationLandscapeIpad => {
                s.model_identifier = "iPad16,3".into();
                s.orientation = Orientation::LandscapeLeft;
                s.bias_deg = [1.0, -1.0];
            }
            Scenario::Fps30 => {
                s.fps = 30.0;
                s.bias_deg = [1.0, -1.0];
            }
            Scenario::MissingTarget => s.drop_validation_target = Some(2),
            Scenario::GlassesNoise => {
                s.bias_deg = [0.8, 0.5];
                s.noise_sd_deg = 0.5;
                s.outlier_prob = 0.02;
                s.outlier_deg = 2.5;
            }
            Scenario::HeadSway => {
                s.bias_deg = [1.0, -0.5];
                s.sway_deg = 6.0;
            }
        }
        s
    }
}

impl fmt::Display for Scenario {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl FromStr for Scenario {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String> {
        SCENARIOS
            .iter()
            .copied()
            .find(|x| x.name() == s)
            .ok_or_else(|| {
                let all: Vec<&str> = SCENARIOS.iter().map(|x| x.name()).collect();
                format!("unknown scenario `{s}` (one of: {})", all.join(", "))
            })
    }
}

// ------------------------------------------------------------------------------------------
// RNG
// ------------------------------------------------------------------------------------------

struct Rng(ChaCha20Rng);

impl Rng {
    fn new(label: &str, seed: u64) -> Self {
        let digest = Sha256::digest(format!(
            "asd_tracking_synth:{GENERATOR_VERSION}:{label}:{seed}"
        ));
        let mut s = [0u8; 32];
        s.copy_from_slice(&digest);
        Rng(ChaCha20Rng::from_seed(s))
    }
    /// Uniform in [0, 1) with 53 bits.
    fn unit(&mut self) -> f64 {
        (self.0.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
    fn uniform(&mut self, a: f64, b: f64) -> f64 {
        a + (b - a) * self.unit()
    }
    /// Box-Muller.
    fn gauss(&mut self, sigma: f64) -> f64 {
        let u1 = 1.0 - self.unit();
        let u2 = self.unit();
        sigma * (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()
    }
}

// ------------------------------------------------------------------------------------------
// Maths
// ------------------------------------------------------------------------------------------

type M4 = [f64; 16];

fn r6(x: f64) -> f64 {
    (x * 1e6).round() / 1e6
}

fn r7(x: f64) -> f64 {
    (x * 1e7).round() / 1e7
}

fn smoothstep(x: f64) -> f64 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

fn lerp(a: f64, b: f64, s: f64) -> f64 {
    a + (b - a) * s
}

/// Face anchor transform: rotation `Ry(yaw) * Rx(pitch)`, translation `t` (column-major).
fn face_transform(yaw_deg: f64, pitch_deg: f64, t: [f64; 3]) -> M4 {
    let (sy, cy) = yaw_deg.to_radians().sin_cos();
    let (sp, cp) = pitch_deg.to_radians().sin_cos();
    // R = Ry * Rx, rows r, columns c.
    let r = [
        [cy, sy * sp, sy * cp],
        [0.0, cp, -sp],
        [-sy, cy * sp, cy * cp],
    ];
    let mut m = [0.0; 16];
    for col in 0..3 {
        for (row, rr) in r.iter().enumerate() {
            m[col * 4 + row] = rr[col];
        }
    }
    m[12] = t[0];
    m[13] = t[1];
    m[14] = t[2];
    m[15] = 1.0;
    m
}

fn translation(t: [f64; 3]) -> M4 {
    face_transform(0.0, 0.0, t)
}

fn rigid_inverse_apply(m: &M4, p: [f64; 3]) -> [f64; 3] {
    let d = [p[0] - m[12], p[1] - m[13], p[2] - m[14]];
    [
        m[0] * d[0] + m[1] * d[1] + m[2] * d[2],
        m[4] * d[0] + m[5] * d[1] + m[6] * d[2],
        m[8] * d[0] + m[9] * d[1] + m[10] * d[2],
    ]
}

/// `[yaw, pitch]` degrees of a face-frame direction (yaw = atan2(x, z)).
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

/// Screen-normalised point (current orientation) to the camera frame (z = 0), metres.
fn screen_cam(geo: &OrientedGeometry, n: [f64; 2]) -> [f64; 3] {
    let native = oriented_to_native_mm(geo, [n[0] * geo.width_mm, n[1] * geo.height_mm]);
    [
        (native[0] - geo.camera_native_mm[0]) / 1000.0,
        -(native[1] - geo.camera_native_mm[1]) / 1000.0,
        0.0,
    ]
}

// ------------------------------------------------------------------------------------------
// Plan
// ------------------------------------------------------------------------------------------

struct ClipPlan {
    start: f64,
}

struct Plan {
    targets: Vec<ScheduledTarget>,
    clips: Vec<ClipPlan>,
    /// Gaze script: from `start` the user looks at the point (sorted by start).
    segs: Vec<(f64, [f64; 2])>,
    ramp_start: f64,
    ramp_end: f64,
    drift_start: f64,
    end: f64,
}

/// Schedules one target window at `*t` and moves `*t` past it.
fn window(
    targets: &mut Vec<ScheduledTarget>,
    segs: &mut Vec<(f64, [f64; 2])>,
    kind: TargetKind,
    at: [f64; 2],
    dur: f64,
    t: &mut f64,
) {
    targets.push(ScheduledTarget {
        kind,
        x_norm: at[0],
        y_norm: at[1],
        start_sec: *t,
        end_sec: *t + dur,
        label: None,
    });
    segs.push((*t - LEAD_SEC, at));
    *t += dur + GAP_SEC;
}

fn plan(spec: &Spec, rng: &mut Rng) -> Plan {
    let params = TrackingQualityParams::default();
    let mut targets = Vec::new();
    let mut segs: Vec<(f64, [f64; 2])> = vec![(f64::NEG_INFINITY, [0.5, 0.5])];
    let mut clips = Vec::new();
    let mut t = T0_SEC + 0.5;

    let cal = &params.calibration;
    let cal_win = (cal.settle_ms + cal.collect_ms) / 1000.0;
    for p in &cal.layout5 {
        window(
            &mut targets,
            &mut segs,
            TargetKind::Calibration,
            *p,
            cal_win,
            &mut t,
        );
    }
    t += POSE_RAMP_SEC;
    let ramp_end = t;
    let ramp_start = ramp_end - POSE_RAMP_SEC;

    let val = &params.validation;
    let val_win = (val.settle_ms + val.collect_ms) / 1000.0;
    let layout = if spec.grid13 {
        val.bench_layout13.clone()
    } else {
        val.layout.clone()
    };
    for p in &layout {
        window(
            &mut targets,
            &mut segs,
            TargetKind::Validation,
            *p,
            val_win,
            &mut t,
        );
    }
    let drift_start = t - GAP_SEC;

    let chk = &params.drift;
    let chk_win = (chk.settle_ms + chk.collect_ms) / 1000.0;
    for k in 0..spec.clips {
        let start = t + 0.2;
        let end = start + spec.clip_sec;
        let mut cur = start;
        while cur < end {
            let at = [rng.uniform(0.15, 0.85), rng.uniform(0.15, 0.85)];
            segs.push((cur, at));
            cur += rng.uniform(0.7, 1.4);
        }
        clips.push(ClipPlan { start });
        t = end + GAP_SEC;
        if spec.drift_checks && !chk.check_layout.is_empty() {
            let at = chk.check_layout[k % chk.check_layout.len()];
            window(
                &mut targets,
                &mut segs,
                TargetKind::DriftCheck,
                at,
                chk_win,
                &mut t,
            );
        }
    }
    Plan {
        targets,
        clips,
        segs,
        ramp_start,
        ramp_end,
        drift_start,
        end: t + GAP_SEC,
    }
}

fn gaze_at(segs: &[(f64, [f64; 2])], t: f64) -> [f64; 2] {
    let i = segs.partition_point(|s| s.0 <= t).saturating_sub(1);
    let (start, cur) = segs[i];
    let prev = segs[i.saturating_sub(1)].1;
    let s = smoothstep((t - start) / SACCADE_SEC);
    [lerp(prev[0], cur[0], s), lerp(prev[1], cur[1], s)]
}

fn pose_at(spec: &Spec, plan: &Plan, t: f64) -> Pose {
    let s = smoothstep((t - plan.ramp_start) / (plan.ramp_end - plan.ramp_start));
    let (a, b) = (spec.cal_pose, spec.later_pose);
    let sway = spec.sway_deg * (std::f64::consts::TAU * spec.sway_hz * (t - T0_SEC)).sin();
    Pose {
        distance_m: lerp(a.distance_m, b.distance_m, s),
        yaw_deg: lerp(a.yaw_deg, b.yaw_deg, s) + sway,
        pitch_deg: lerp(a.pitch_deg, b.pitch_deg, s),
    }
}

/// Per-frame flags.
#[derive(Clone, Copy, Default)]
struct Flags {
    lost: bool,
    blink: bool,
    spike: bool,
}

fn frame_flags(spec: &Spec, plan: &Plan, rng: &mut Rng, frames: usize) -> Vec<Flags> {
    let mut flags = vec![Flags::default(); frames];
    let fps = spec.fps;
    let first_frame = |t: f64| ((t - T0_SEC) * fps - 1e-9).ceil().max(0.0) as usize;
    let mut mark = |from: usize, count: usize, f: &dyn Fn(&mut Flags)| {
        for fl in flags.iter_mut().skip(from).take(count) {
            f(fl);
        }
    };
    for (k, clip) in plan.clips.iter().enumerate() {
        if let Some(period) = spec.blink_period_sec {
            let mut at = period * 0.5;
            while at < spec.clip_sec - 0.5 {
                let dur = rng.uniform(0.1, 0.4);
                let n = ((dur * fps).round() as usize).max(1);
                mark(first_frame(clip.start + at), n, &|f| f.blink = true);
                at += period;
            }
        }
        for e in spec.events.iter().filter(|e| e.clip == k) {
            let from = first_frame(clip.start + e.start_sec);
            let n = (e.dur_sec * fps + 1e-9).floor() as usize;
            match e.kind {
                EventKind::Loss => mark(from, n, &|f| f.lost = true),
                EventKind::Blink => mark(from, n.max(1), &|f| f.blink = true),
                EventKind::Spike => mark(from, 1, &|f| f.spike = true),
            }
        }
    }
    flags
}

// ------------------------------------------------------------------------------------------
// Generator
// ------------------------------------------------------------------------------------------

/// Generates the log of `scenario`.
pub fn generate(scenario: Scenario, seed: u64) -> RawLog {
    generate_with(&scenario.spec(), scenario.name(), seed)
}

/// Canonical JSONL bytes of [`generate`].
pub fn generate_jsonl(scenario: Scenario, seed: u64) -> String {
    to_jsonl(&generate(scenario, seed)).expect("raw log serialises")
}

/// Generates the log of an arbitrary `spec`; `label` and `seed` select the random stream.
pub fn generate_with(spec: &Spec, label: &str, seed: u64) -> RawLog {
    let mut rng = Rng::new(label, seed);
    let found = lookup(&spec.model_identifier);
    let geo = found.geometry.rotate(spec.orientation);
    let plan = plan(spec, &mut rng);
    let frames = ((plan.end - T0_SEC) * spec.fps).floor() as usize + 1;
    let flags = frame_flags(spec, &plan, &mut rng, frames);
    let centre = screen_cam(&geo, [0.5, 0.5]);
    let dropped = spec
        .drop_validation_target
        .and_then(|i| {
            plan.targets
                .iter()
                .filter(|t| t.kind == TargetKind::Validation)
                .nth(i)
        })
        .map(|t| (t.start_sec, t.end_sec + 0.2));

    let mut samples = Vec::with_capacity(frames);
    for (n, fl) in flags.iter().enumerate() {
        let t = r6(T0_SEC + n as f64 / spec.fps);
        // Draw first so the random stream does not depend on which frames are dropped.
        let noise = [rng.gauss(spec.noise_sd_deg), rng.gauss(spec.noise_sd_deg)];
        let outlier = rng.unit() < spec.outlier_prob;
        let outlier_dir = rng.uniform(0.0, std::f64::consts::TAU);
        if dropped.is_some_and(|(a, b)| t >= a && t < b) {
            continue;
        }
        let clip = plan
            .clips
            .iter()
            .enumerate()
            .rev()
            .find(|(_, c)| t >= c.start && t < c.start + spec.clip_sec);
        let mut s = RawTrackingSample {
            t_sec: t,
            face_tracked: false,
            face_transform: None,
            look_at: None,
            left_eye_transform: None,
            right_eye_transform: None,
            eye_blink_left: None,
            eye_blink_right: None,
            ambient_intensity: Some(spec.ambient_lux),
            ambient_color_temp: Some(5500.0),
            light_direction: None,
            orientation: spec.orientation,
            clip_media_time_sec: clip.map(|(_, c)| r6(t - c.start)),
            stimulus_id: clip.map(|(k, _)| format!("synth_clip_{}", k + 1)),
        };
        if !fl.lost {
            let pose = pose_at(spec, &plan, t);
            let anchor = face_transform(
                pose.yaw_deg,
                pose.pitch_deg,
                [centre[0], centre[1], -pose.distance_m],
            );
            let target = screen_cam(&geo, gaze_at(&plan.segs, t));
            let local = rigid_inverse_apply(&anchor, target);
            let true_angles = angles_of([
                local[0] - EYE_MID_FACE[0],
                local[1] - EYE_MID_FACE[1],
                local[2] - EYE_MID_FACE[2],
            ]);
            let drift_t = (t - plan.drift_start).max(0.0);
            let mut measured = [
                spec.gain[0] * true_angles[0]
                    + spec.bias_deg[0]
                    + spec.drift_deg_per_sec[0] * drift_t
                    + noise[0],
                spec.gain[1] * true_angles[1]
                    + spec.bias_deg[1]
                    + spec.drift_deg_per_sec[1] * drift_t
                    + noise[1],
            ];
            if outlier {
                measured[0] += spec.outlier_deg * outlier_dir.cos();
                measured[1] += spec.outlier_deg * outlier_dir.sin();
            }
            if fl.spike {
                measured[0] += SPIKE_DEG;
            }
            let md = dir_of(measured);
            s.face_tracked = true;
            s.face_transform = Some(anchor.map(r7));
            s.look_at = Some([
                r7(EYE_MID_FACE[0] + LOOK_AT_LEN_M * md[0]),
                r7(EYE_MID_FACE[1] + LOOK_AT_LEN_M * md[1]),
                r7(EYE_MID_FACE[2] + LOOK_AT_LEN_M * md[2]),
            ]);
            s.left_eye_transform = Some(translation([
                EYE_HALF_SPACING_M,
                EYE_MID_FACE[1],
                EYE_MID_FACE[2],
            ]));
            s.right_eye_transform = Some(translation([
                -EYE_HALF_SPACING_M,
                EYE_MID_FACE[1],
                EYE_MID_FACE[2],
            ]));
            let blink = if fl.blink { 0.9 } else { 0.05 };
            s.eye_blink_left = Some(blink);
            s.eye_blink_right = Some(blink);
        }
        samples.push(s);
    }

    let header = RawLogHeader {
        format: RAW_LOG_FORMAT.to_string(),
        source: TrackingSource::ArkitFace,
        device_model: found.model_identifier,
        orientation: spec.orientation,
        geometry_source: found.source,
        geometry: found.geometry,
        targets: plan.targets,
        app_version: Some(format!("asd-tracking-synth/{GENERATOR_VERSION}")),
        condition: Some(spec.condition.clone()),
        ground_truth: Some(json!({
            "generator": format!("asd-tracking-synth/{GENERATOR_VERSION}"),
            "label": label,
            "seed": seed,
            "fps": spec.fps,
            "biasDeg": spec.bias_deg,
            "gain": spec.gain,
            "driftDegPerSec": spec.drift_deg_per_sec,
            "noiseSdDeg": spec.noise_sd_deg,
            "calPose": pose_json(spec.cal_pose),
            "laterPose": pose_json(spec.later_pose),
            "swayDeg": spec.sway_deg,
            "ambientLux": spec.ambient_lux,
        })),
    };
    RawLog { header, samples }
}

fn pose_json(p: Pose) -> serde_json::Value {
    json!({"distanceM": p.distance_m, "yawDeg": p.yaw_deg, "pitchDeg": p.pitch_deg})
}
