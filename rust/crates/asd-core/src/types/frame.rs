//! owner: WP-R.0
//!
//! Gaze capture types. `GazeTelemetrySample` ports the full Swift struct; `GazeFrame` keeps
//! only the fields the metric extractors read (CON-1) and ignores every other key Swift
//! writes (blend shapes, `aoiDwellMs`, ...).

use serde::{Deserialize, Serialize};

use super::quality::GazeQualityVerdict;

/// One raw telemetry sample (`GazeTelemetrySample`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GazeTelemetrySample {
    pub timestamp_ms: f64,
    pub video_time_sec: f64,
    pub x_norm: f64,
    pub y_norm: f64,
    pub confidence: f64,
    pub left_eye_openness: f64,
    pub right_eye_openness: f64,
    pub head_pitch_rad: f64,
    pub head_yaw_rad: f64,
    pub head_roll_rad: f64,
    pub distance_to_screen_cm: f64,
    pub is_valid: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stimulus_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clip_media_time_sec: Option<f64>,
}

/// The `gaze` fields of a frame that the extractors read.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GazeFrameSample {
    pub is_valid: bool,
    pub x_norm: f64,
    pub y_norm: f64,
    pub video_time_sec: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
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
