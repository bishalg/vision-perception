//! owner: WP-R.0
//!
//! Port of `NormativeBaselineLibrary.swift` model types.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Age-conditional expected range for one biomarker expectation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NormativeRange {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acceptable_low: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acceptable_high: Option<f64>,
}

/// Direction of "better" for an expectation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NormativeDirection {
    #[serde(rename = "higherBetter")]
    HigherBetter,
    #[serde(rename = "lowerBetter")]
    LowerBetter,
    #[serde(rename = "band")]
    Band,
}

/// One expected-response row inside a stimulus segment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NormativeExpectation {
    pub biomarker: String,
    pub direction: NormativeDirection,
    #[serde(default, rename = "targetAOI", skip_serializing_if = "Option::is_none")]
    pub target_aoi: Option<String>,
    /// Keyed by age-bracket id (`"18-23"`) or `"default"`.
    pub by_age: BTreeMap<String, NormativeRange>,
    pub citation: String,
    /// `"literature"` or `"interpolated"`.
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// A time-coded chunk of a stimulus video.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NormativeSegment {
    pub id: String,
    pub description: String,
    pub start_sec: f64,
    pub end_sec: f64,
    pub expectations: Vec<NormativeExpectation>,
}

/// Per-video normative profile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NormativeVideo {
    pub title: String,
    pub category: String,
    pub weight: f64,
    pub segments: Vec<NormativeSegment>,
}

/// `NormativeBaselineSet.AgeBracket`, also the scoring profile's `ageBands[]` item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgeBracket {
    pub id: String,
    pub min_months: i64,
    pub max_months: i64,
    /// DSP-2 (profile 1.3.0): the age in months a session in this band carries as `ageMonths`
    /// when only the band is known (the iOS age picker). Absent: [`Self::representative_or_midpoint`].
    /// A baselines document's `ageBrackets` never carry it (the norm job strips it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub representative_months: Option<i64>,
}

impl AgeBracket {
    /// A bracket without `representativeMonths`.
    pub fn new(id: impl Into<String>, min_months: i64, max_months: i64) -> Self {
        Self {
            id: id.into(),
            min_months,
            max_months,
            representative_months: None,
        }
    }

    /// `representativeMonths`, else `(minMonths + maxMonths) / 2` (integer division, DSP-2).
    pub fn representative_or_midpoint(&self) -> i64 {
        self.representative_months
            .unwrap_or((self.min_months + self.max_months).div_euclid(2))
    }
}

/// Versioned baseline set bundled with the app.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NormativeBaselineSet {
    pub schema_version: String,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<String>,
    pub age_brackets: Vec<AgeBracket>,
    pub videos: BTreeMap<String, NormativeVideo>,
}
