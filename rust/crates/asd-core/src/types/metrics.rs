//! owner: WP-R.0
//!
//! Port of `PerVideoMetricSet` (`PerVideoMetrics.swift`).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Extracted metrics for one stimulus clip.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerVideoMetricSet {
    pub stimulus_id: String,
    pub clip_frame_count: i64,
    pub valid_frame_count: i64,
    pub valid_sample_ratio: f64,
    pub observed_duration_sec: f64,
    /// AOI name to dwell ratio over valid frames.
    pub aoi_dwell_ratios: BTreeMap<String, f64>,
    /// Metric key (`biomarker` or `biomarker::AOI`) to value.
    pub metrics: BTreeMap<String, f64>,
}
