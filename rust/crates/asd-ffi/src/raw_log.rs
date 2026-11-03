//! owner: WP-G.8 (audit fix)
//!
//! REP-1 raw tracking log encoders for the Debug-only bench capture build (BEN-2). The app
//! collects one [`RawSampleRecord`] per ARKit frame and hands them here; Rust writes the
//! `asd-raw-tracking/1` header and sample lines with the asd-core types
//! ([`RawLogHeader`], [`RawTrackingSample`], `asd_core::gaze::raw_log::to_jsonl`), so the
//! format is defined once.
//!
//! What the encoder decides (the Swift side decides none of it):
//! - An untracked frame (`face_tracked == false`) carries no face fields (`face_transform`,
//!   `look_at`, both eye transforms, both blinks): zeros would read as a perfect frontal pose.
//! - A non-finite optional value is omitted, never defaulted (CON-14); a matrix or vector with
//!   any non-finite component is omitted whole. A non-finite `t_sec` is refused.
//! - Matrices, vectors and blend shapes are ARKit single-precision values (`simd_float4x4`,
//!   `simd_float3`, `NSNumber(Float)`): each is written at its shortest `f32` decimal form, so
//!   0.004 stays 0.004 in the log instead of 0.004000000189989805. `t_sec` and the light
//!   estimate (`TimeInterval`, `CGFloat`) are doubles and are written as given.
//! - The document keeps `t_sec` strictly increasing (REP-1): a repeated, older or non-finite
//!   frame time is dropped and counted.
//! - The header geometry is the one the pipeline would use for the model (table entry or
//!   idiom default, GEO-2), and `condition` is a BEN-4 name ([`GazeBenchCondition`]), never
//!   free text.

use asd_core::gaze::pipeline::PipelineConfig;
use asd_core::gaze::raw_log::{self, RawLog};
use asd_core::types::tracking::{
    BenchCondition, RawLogHeader, RawTrackingSample, ScheduledTarget, TrackingSource,
    RAW_LOG_FORMAT,
};

use crate::gaze::{GazeOrientation, GazeTargetKind, RawSampleRecord};
use crate::{encode, AsdError};

/// A BEN-4 bench condition (the header's `condition`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum GazeBenchCondition {
    Baseline,
    Distance,
    Head,
    Motion,
    Glasses,
    LowLight,
    SideLight,
    Occlusion,
    Blinks,
}

impl From<GazeBenchCondition> for BenchCondition {
    fn from(c: GazeBenchCondition) -> Self {
        match c {
            GazeBenchCondition::Baseline => Self::Baseline,
            GazeBenchCondition::Distance => Self::Distance,
            GazeBenchCondition::Head => Self::Head,
            GazeBenchCondition::Motion => Self::Motion,
            GazeBenchCondition::Glasses => Self::Glasses,
            GazeBenchCondition::LowLight => Self::LowLight,
            GazeBenchCondition::SideLight => Self::SideLight,
            GazeBenchCondition::Occlusion => Self::Occlusion,
            GazeBenchCondition::Blinks => Self::Blinks,
        }
    }
}

impl From<BenchCondition> for GazeBenchCondition {
    fn from(c: BenchCondition) -> Self {
        match c {
            BenchCondition::Baseline => Self::Baseline,
            BenchCondition::Distance => Self::Distance,
            BenchCondition::Head => Self::Head,
            BenchCondition::Motion => Self::Motion,
            BenchCondition::Glasses => Self::Glasses,
            BenchCondition::LowLight => Self::LowLight,
            BenchCondition::SideLight => Self::SideLight,
            BenchCondition::Occlusion => Self::Occlusion,
            BenchCondition::Blinks => Self::Blinks,
        }
    }
}

/// One target of the bench schedule (`ScheduledTarget`), times on the ARKit frame clock.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct GazeScheduledTarget {
    pub kind: GazeTargetKind,
    pub x_norm: f64,
    pub y_norm: f64,
    pub start_sec: f64,
    pub end_sec: f64,
    pub label: Option<String>,
}

impl From<GazeScheduledTarget> for ScheduledTarget {
    fn from(t: GazeScheduledTarget) -> Self {
        Self {
            kind: t.kind.into(),
            x_norm: t.x_norm,
            y_norm: t.y_norm,
            start_sec: t.start_sec,
            end_sec: t.end_sec,
            label: t.label,
        }
    }
}

/// What the header needs from the app; the geometry is resolved here.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct GazeRawLogHeaderInput {
    /// Hardware model identifier (`utsname.machine`).
    pub model_identifier: String,
    pub orientation: GazeOrientation,
    pub targets: Vec<GazeScheduledTarget>,
    pub app_version: Option<String>,
    pub condition: Option<GazeBenchCondition>,
}

/// A whole raw log and what was left out of it.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct GazeRawLogDocument {
    /// Header line, one sample per line, trailing newline (`raw_log::to_jsonl`).
    pub jsonl: String,
    pub sample_count: u64,
    /// Frames dropped for a repeated, older or non-finite `t_sec`.
    pub dropped_sample_count: u64,
}

fn header(input: GazeRawLogHeaderInput) -> RawLogHeader {
    let config = PipelineConfig::for_model(&input.model_identifier, input.orientation.into());
    RawLogHeader {
        format: RAW_LOG_FORMAT.to_owned(),
        source: TrackingSource::ArkitFace,
        device_model: config.model_identifier,
        orientation: input.orientation.into(),
        geometry_source: config.geometry_source,
        geometry: config.geometry,
        targets: input.targets.into_iter().map(Into::into).collect(),
        app_version: input.app_version,
        condition: input
            .condition
            .map(|c| BenchCondition::from(c).as_str().to_owned()),
        ground_truth: None,
        clips: Vec::new(),
        bundle_version: None,
        profile_version: None,
        session_id: None,
        participant_token: None,
    }
}

/// A single-precision value at its shortest decimal form (see the module docs).
fn widen_f32(v: f64) -> f64 {
    // `v as f32` is exact for a value that came from an f32; `Display` of f32 is the shortest
    // string that reads back as the same f32.
    let f = v as f32;
    f.to_string().parse::<f64>().unwrap_or(v)
}

fn finite(v: Option<f64>) -> Option<f64> {
    v.filter(|x| x.is_finite())
}

fn finite_f32(v: Option<f64>) -> Option<f64> {
    finite(v).map(widen_f32)
}

fn finite_array<const N: usize>(v: Option<[f64; N]>) -> Option<[f64; N]> {
    v.filter(|a| a.iter().all(|x| x.is_finite()))
        .map(|a| a.map(widen_f32))
}

/// The canonical log sample of one frame (module docs), or `InvalidArgument` for a matrix
/// that is not 16 numbers or a non-finite `t_sec`.
fn sample(record: RawSampleRecord) -> Result<RawTrackingSample, AsdError> {
    if !record.t_sec.is_finite() {
        return Err(AsdError::InvalidArgument {
            message: "t_sec must be finite".to_owned(),
        });
    }
    canonical(RawTrackingSample::try_from(record)?)
}

/// The module-doc rules applied to a decoded sample (shared by the bench and research
/// writers). `t_sec` was checked by the caller.
fn canonical(s: RawTrackingSample) -> Result<RawTrackingSample, AsdError> {
    let tracked = s.face_tracked;
    // RFL-1: affect values are model outputs (doubles): kept as given when finite.
    let affect = s.affect.clone().map(|a| asd_core::types::GazeFrameAffect {
        social_smile_intensity: finite(a.social_smile_intensity),
        expressivity_index: finite(a.expressivity_index),
        surprise_score: finite(a.surprise_score),
    });
    Ok(RawTrackingSample {
        face_transform: finite_array(s.face_transform).filter(|_| tracked),
        look_at: finite_array(s.look_at).filter(|_| tracked),
        left_eye_transform: finite_array(s.left_eye_transform).filter(|_| tracked),
        right_eye_transform: finite_array(s.right_eye_transform).filter(|_| tracked),
        eye_blink_left: finite_f32(s.eye_blink_left).filter(|_| tracked),
        eye_blink_right: finite_f32(s.eye_blink_right).filter(|_| tracked),
        ambient_intensity: finite(s.ambient_intensity),
        ambient_color_temp: finite(s.ambient_color_temp),
        light_direction: finite_array(s.light_direction),
        clip_media_time_sec: finite(s.clip_media_time_sec),
        affect,
        ..s
    })
}

/// RFL-1: one line of the research frame log (`research.jsonl`, RFL-2) from a
/// `RawTrackingSample` JSON object (snake_case, PIPE-2, with the frame's `affect` on recorded
/// frames). The same canonical form as [`gaze_raw_log_line`]; a non-finite `t_sec` or a
/// malformed object is refused. The kit never formats a line itself.
#[uniffi::export]
pub fn gaze_research_log_sample_line(sample_json: String) -> Result<String, AsdError> {
    let s: RawTrackingSample = serde_json::from_str(&sample_json)
        .map_err(|e| crate::invalid_json("research log sample", e))?;
    if !s.t_sec.is_finite() {
        return Err(AsdError::InvalidArgument {
            message: "t_sec must be finite".to_owned(),
        });
    }
    encode(&canonical(s)?, "research log sample")
}

/// RFL-1: the header line of a research frame log from a `RawLogHeader` JSON object
/// (camelCase), written at session end when the clips and their AOIs are known. `format`
/// must be `asd-raw-tracking/1` and `sessionId` present (the backend checks it against the
/// upload path, RFL-3).
#[uniffi::export]
pub fn gaze_research_log_header_line(header_json: String) -> Result<String, AsdError> {
    let header: RawLogHeader = serde_json::from_str(&header_json)
        .map_err(|e| crate::invalid_json("research log header", e))?;
    if header.format != RAW_LOG_FORMAT {
        return Err(AsdError::InvalidArgument {
            message: format!("format must be {RAW_LOG_FORMAT}"),
        });
    }
    if header
        .session_id
        .as_deref()
        .is_none_or(|s| s.trim().is_empty())
    {
        return Err(AsdError::InvalidArgument {
            message: "sessionId is required in a research log header".to_owned(),
        });
    }
    encode(&header, "research log header")
}

/// BEN-4 conditions in spec order, for the bench screen's picker.
#[uniffi::export]
pub fn gaze_bench_conditions() -> Vec<GazeBenchCondition> {
    BenchCondition::ALL.into_iter().map(Into::into).collect()
}

/// The camelCase name of a condition as the header writes it (also safe in a file name).
#[uniffi::export]
pub fn gaze_bench_condition_name(condition: GazeBenchCondition) -> String {
    BenchCondition::from(condition).as_str().to_owned()
}

/// One `RawTrackingSample` line (snake_case, PIPE-2), no trailing newline.
#[uniffi::export]
pub fn gaze_raw_log_line(record: RawSampleRecord) -> Result<String, AsdError> {
    encode(&sample(record)?, "raw tracking sample")
}

/// The `RawLogHeader` line (REP-1), no trailing newline.
#[uniffi::export]
pub fn gaze_raw_log_header_line(header_input: GazeRawLogHeaderInput) -> Result<String, AsdError> {
    encode(&header(header_input), "raw log header")
}

/// The whole log: header, then every usable frame in order with `t_sec` strictly
/// increasing. Frames with a repeated, older or non-finite time are dropped and counted; a
/// matrix that is not 16 numbers is `InvalidArgument` (a programming error, not a frame).
#[uniffi::export]
pub fn gaze_raw_log_document(
    header_input: GazeRawLogHeaderInput,
    records: Vec<RawSampleRecord>,
) -> Result<GazeRawLogDocument, AsdError> {
    let mut samples: Vec<RawTrackingSample> = Vec::with_capacity(records.len());
    let mut dropped: u64 = 0;
    for record in records {
        let increasing =
            record.t_sec.is_finite() && samples.last().is_none_or(|last| record.t_sec > last.t_sec);
        if !increasing {
            dropped += 1;
            continue;
        }
        samples.push(sample(record)?);
    }
    let log = RawLog {
        header: header(header_input),
        samples,
    };
    let jsonl = raw_log::to_jsonl(&log).map_err(|e| AsdError::InvalidJson {
        message: format!("raw log: {e}"),
    })?;
    Ok(GazeRawLogDocument {
        jsonl,
        sample_count: log.samples.len() as u64,
        dropped_sample_count: dropped,
    })
}

/// The bench target log (`<base>.targets.json`): the schedule as a JSON array of
/// `ScheduledTarget`, the same objects as the header's `targets`.
#[uniffi::export]
pub fn gaze_scheduled_targets_json(targets: Vec<GazeScheduledTarget>) -> Result<String, AsdError> {
    let targets: Vec<ScheduledTarget> = targets.into_iter().map(Into::into).collect();
    encode(&targets, "scheduled targets")
}
