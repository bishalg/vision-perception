//! owner: WP-R.0
//!
//! Gaze capture types. `GazeTelemetrySample` ports the full Swift struct; `GazeFrame` keeps
//! only the fields the metric extractors read (CON-1) and ignores every other key Swift
//! writes (blend shapes, `aoiDwellMs`, ...).
//!
//! Key spelling (CON-1): Swift's `GazeTelemetrySample.CodingKeys` write the gaze sample in
//! snake_case with `valid` for `isValid` (`x_norm`, `video_time_sec`, `clip_media_time_sec`, ...).
//! The sample types serialise in exactly that wire spelling and also accept the camelCase
//! property names (`xNorm`, `isValid`, ...) on decode, so either spelling of one sample
//! decodes to the same value. Frame-level keys (`activeAOINames`, `qualityVerdict`,
//! `stimulusId`) and `affect` keys are camelCase in Swift and here.

use serde::{Deserialize, Serialize};

use super::quality::GazeQualityVerdict;

/// One raw telemetry sample (`GazeTelemetrySample`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct GazeTelemetrySample {
    #[serde(alias = "timestampMs")]
    pub timestamp_ms: f64,
    #[serde(alias = "videoTimeSec")]
    pub video_time_sec: f64,
    #[serde(alias = "xNorm")]
    pub x_norm: f64,
    #[serde(alias = "yNorm")]
    pub y_norm: f64,
    pub confidence: f64,
    #[serde(alias = "leftEyeOpenness")]
    pub left_eye_openness: f64,
    #[serde(alias = "rightEyeOpenness")]
    pub right_eye_openness: f64,
    #[serde(alias = "headPitchRad")]
    pub head_pitch_rad: f64,
    #[serde(alias = "headYawRad")]
    pub head_yaw_rad: f64,
    #[serde(alias = "headRollRad")]
    pub head_roll_rad: f64,
    #[serde(alias = "distanceToScreenCm")]
    pub distance_to_screen_cm: f64,
    #[serde(rename = "valid", alias = "isValid")]
    pub is_valid: bool,
    #[serde(default, alias = "stimulusId", skip_serializing_if = "Option::is_none")]
    pub stimulus_id: Option<String>,
    #[serde(
        default,
        alias = "clipMediaTimeSec",
        skip_serializing_if = "Option::is_none"
    )]
    pub clip_media_time_sec: Option<f64>,
}

/// The `gaze` fields of a frame that the extractors read (same keys as
/// [`GazeTelemetrySample`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct GazeFrameSample {
    #[serde(rename = "valid", alias = "isValid")]
    pub is_valid: bool,
    #[serde(alias = "xNorm")]
    pub x_norm: f64,
    #[serde(alias = "yNorm")]
    pub y_norm: f64,
    #[serde(alias = "videoTimeSec")]
    pub video_time_sec: f64,
    #[serde(
        default,
        alias = "clipMediaTimeSec",
        skip_serializing_if = "Option::is_none"
    )]
    pub clip_media_time_sec: Option<f64>,
}

/// The `affect` values the extractors read.
///
/// In Swift these three are computed properties of `FacialAffectMetrics`, not stored keys, so
/// a frame JSON built by the app must supply them explicitly; they default to `None`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GazeFrameAffect {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub social_smile_intensity: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expressivity_index: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surprise_score: Option<f64>,
}

/// One 60 Hz frame, reduced to what scoring reads.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GazeFrame {
    pub gaze: GazeFrameSample,
    #[serde(default)]
    pub affect: GazeFrameAffect,
    #[serde(default, rename = "activeAOINames")]
    pub active_aoi_names: Vec<String>,
    pub quality_verdict: GazeQualityVerdict,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stimulus_id: Option<String>,
}
