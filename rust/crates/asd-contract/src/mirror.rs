//! owner: WP-R.3
//!
//! Schema-only mirrors of the `asd-core::types` that appear inside contract documents.
//!
//! `asd-core` stays free of `schemars` (WS-2 keeps it minimal), so the JSON Schema of each
//! embedded core type is described here and attached with `#[schemars(with = "...")]`. These
//! types are never constructed or serialised; serde attributes are present only so
//! `schemars` derives the same key names, optionality and enum values as the real types.
//!
//! Drift guard: `tests/config_defaults.rs` and `tests/session_v1_6.rs` validate real
//! documents (bundled catalog and baselines, the iOS golden payload) against the schema
//! with unknown keys forbidden, and decode-then-validate checks fail when a mirror requires
//! a key the real type omits. When `asd-core` gains an optional `schemars` feature these
//! mirrors can be replaced by derives on the real types.

#![allow(dead_code)]

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Deserialize, JsonSchema)]
pub(crate) enum CaptureSource {
    #[serde(rename = "truedepth")]
    TrueDepth,
    #[serde(rename = "simulated")]
    Simulated,
    #[serde(rename = "synthetic")]
    Synthetic,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum GazeQualityVerdict {
    Optimal,
    Acceptable,
    Degraded,
    Rejected,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum AttentionFlag {
    Green,
    Yellow,
    Red,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ExpectationStatus {
    OnTarget,
    Monitor,
    Atypical,
}

#[derive(Deserialize, JsonSchema)]
pub(crate) enum NormativeDirection {
    #[serde(rename = "higherBetter")]
    HigherBetter,
    #[serde(rename = "lowerBetter")]
    LowerBetter,
    #[serde(rename = "band")]
    Band,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PerVideoScoreRow {
    segment_id: String,
    segment_description: String,
    biomarker: String,
    #[serde(default, rename = "targetAOI")]
    target_aoi: Option<String>,
    metric_key: String,
    direction: NormativeDirection,
    observed: f64,
    #[serde(default)]
    expected: Option<f64>,
    #[serde(default)]
    acceptable_low: Option<f64>,
    #[serde(default)]
    acceptable_high: Option<f64>,
    status: ExpectationStatus,
    score: f64,
    imputed: bool,
    citation: String,
    source: String,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PerVideoScoreReport {
    stimulus_id: String,
    title: String,
    category: String,
    rows: Vec<PerVideoScoreRow>,
    subscore: f64,
    #[schemars(range(min = 0.0, max = 1.0))]
    valid_sample_ratio: f64,
    low_confidence: bool,
    has_baseline: bool,
    weight: f64,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NormativeRange {
    #[serde(default)]
    expected: Option<f64>,
    #[serde(default)]
    acceptable_low: Option<f64>,
    #[serde(default)]
    acceptable_high: Option<f64>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NormativeExpectation {
    biomarker: String,
    direction: NormativeDirection,
    #[serde(default, rename = "targetAOI")]
    target_aoi: Option<String>,
    /// Keyed by age-bracket id (`"18-23"`) or `"default"`.
    by_age: BTreeMap<String, NormativeRange>,
    citation: String,
    /// `"literature"` or `"interpolated"`.
    source: String,
    #[serde(default)]
    note: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NormativeSegment {
    id: String,
    description: String,
    start_sec: f64,
    end_sec: f64,
    expectations: Vec<NormativeExpectation>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NormativeVideo {
    title: String,
    category: String,
    weight: f64,
    segments: Vec<NormativeSegment>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AgeBracket {
    id: String,
    min_months: i64,
    max_months: i64,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NormativeBaselineSet {
    schema_version: String,
    status: String,
    #[serde(default)]
    provenance: Option<String>,
    age_brackets: Vec<AgeBracket>,
    videos: BTreeMap<String, NormativeVideo>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AgePack {
    id: String,
    min_months: i64,
    max_months: i64,
}

#[derive(Deserialize, JsonSchema)]
pub(crate) struct Membership {
    id: String,
    order: i64,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Area {
    name: String,
    start_sec: f64,
    end_sec: f64,
    x_min: f64,
    y_min: f64,
    x_max: f64,
    y_max: f64,
    roles: Vec<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Clip {
    id: String,
    title: String,
    category: String,
    duration_sec: f64,
    min_months: i64,
    max_months: i64,
    protocols: Vec<Membership>,
    areas: Vec<Area>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StimulusCatalog {
    schema_version: String,
    #[serde(default)]
    comment: Option<String>,
    languages: Vec<String>,
    fallback_language: String,
    age_packs: Vec<AgePack>,
    clips: Vec<Clip>,
}
