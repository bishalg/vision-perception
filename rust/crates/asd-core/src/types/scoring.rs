//! owner: WP-R.0
//!
//! Port of the score models in `PerVideoScoringEngine.swift`.
//!
//! `PerVideoScoreRow` / `PerVideoScoreReport` are camelCase and are what
//! `rawPayload.perVideo[]` carries. The snake_case `perVideo` rows of the session request
//! are a different struct that belongs to `asd-contract` (trap 4); do not reuse these there.

use serde::{Deserialize, Serialize};

use super::baselines::NormativeDirection;

/// Status band of one scored cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ExpectationStatus {
    OnTarget,
    Monitor,
    Atypical,
}

/// One scored expected-vs-recorded cell.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerVideoScoreRow {
    pub segment_id: String,
    pub segment_description: String,
    pub biomarker: String,
    #[serde(default, rename = "targetAOI", skip_serializing_if = "Option::is_none")]
    pub target_aoi: Option<String>,
    pub metric_key: String,
    pub direction: NormativeDirection,
    pub observed: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acceptable_low: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acceptable_high: Option<f64>,
    pub status: ExpectationStatus,
    pub score: f64,
    pub imputed: bool,
    pub citation: String,
    pub source: String,
}

/// Result for one stimulus clip.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerVideoScoreReport {
    pub stimulus_id: String,
    pub title: String,
    pub category: String,
    pub rows: Vec<PerVideoScoreRow>,
    pub subscore: f64,
    pub valid_sample_ratio: f64,
    pub low_confidence: bool,
    pub has_baseline: bool,
    pub weight: f64,
}

/// Session-level per-stimulus matrix with the weighted composite.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerVideoSessionScore {
    pub age_bracket_id: String,
    pub age_months: i64,
    pub composite_score: f64,
    pub videos: Vec<PerVideoScoreReport>,
    pub baseline_status: String,
    pub baseline_schema_version: String,
    pub scored_at_utc: String,
}
