//! owner: WP-X.1
//!
//! The parts of the 1.2.0 `ScoringProfile` the core reads (PRM-7, PRM-9), as core types.
//!
//! `asd-contract` depends on `asd-core`, so the core cannot name
//! `asd_contract::config::ScoringProfile`. These structs carry the same members with the same
//! serde spelling (camelCase, every section `serde(default)` to today's constant), so a
//! `ScoringProfile` body decodes into [`AnalysisProfile`] directly and the contract's sections
//! serialise to the same JSON as the ones here (`tests/analysis.rs` asserts both directions
//! and that the defaults are equal). The defaults below are the default source of
//! `asd_contract::config::ScoringProfile::default()`; a product caller always passes the
//! published (or experiment) profile's values.

use serde::{Deserialize, Serialize};

use crate::flag::FlagThresholds;
use crate::metrics::{
    FLAT_AFFECT_GAIN, FRAME_INTERVAL_SEC, MIN_FIXATION_DURATION_SEC, SMILE_REACTIVITY_WINDOW_SEC,
    VELOCITY_THRESHOLD_UNITS_PER_SEC,
};
use crate::quality::QualityGate;
use crate::types::tracking::{ScoringCouplingParams, TrackingQualityParams};
use crate::types::AgeBracket;

/// `aoi.dwellCapMs` default: the PLC-5 per-frame dwell cap of the kit (`maxFrameDwellSec`).
pub const DEFAULT_DWELL_CAP_MS: u32 = 100;
/// `review.lowQualityValidRatio` default (backend `auto_review`, INV-1).
pub const DEFAULT_LOW_QUALITY_VALID_RATIO: f64 = 0.80;

/// Unit of the I-VT velocity threshold (`metrics.ivtUnit`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IvtUnit {
    /// Normalised viewport units per second (`metrics.ivtVelocityUnitsPerSec`): today's rule.
    #[default]
    Norm,
    /// Degrees of visual angle per second (`metrics.ivtVelocityDegPerSec`, SCR-3, owner gate
    /// H-G2). Not applied by [`crate::metrics::extract_with`] until WP-G.13: frames carry no
    /// eye position; [`crate::analysis::evaluate`] reports `param_not_applied: metrics.ivtUnit`.
    Deg,
}

/// `ScoringProfile.metrics` (PRM-7): fixation and affect extraction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MetricsParams {
    pub ivt_velocity_units_per_sec: f64,
    pub ivt_velocity_deg_per_sec: f64,
    pub ivt_unit: IvtUnit,
    pub min_fixation_duration_sec: f64,
    pub fallback_frame_interval_sec: f64,
    pub smile_reactivity_window_sec: f64,
    pub flat_affect_gain: f64,
}

impl Default for MetricsParams {
    fn default() -> Self {
        Self {
            ivt_velocity_units_per_sec: VELOCITY_THRESHOLD_UNITS_PER_SEC,
            ivt_velocity_deg_per_sec: ScoringCouplingParams::default().ivt_velocity_deg_per_sec,
            ivt_unit: IvtUnit::Norm,
            min_fixation_duration_sec: MIN_FIXATION_DURATION_SEC,
            fallback_frame_interval_sec: FRAME_INTERVAL_SEC,
            smile_reactivity_window_sec: SMILE_REACTIVITY_WINDOW_SEC,
            flat_affect_gain: FLAT_AFFECT_GAIN,
        }
    }
}

/// `ScoringProfile.aoi` (PRM-7): AOI hit testing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AoiParams {
    /// Grows (positive) or shrinks (negative) every AOI box on each side, screen-normalised.
    pub margin_norm: f64,
    /// Longest interval one frame contributes to a dwell **time** (PLC-5). The core's dwell
    /// metrics are frame-count ratios (the Swift oracle), so no core metric reads it yet; it
    /// travels with the view for the kit (WP-M.*) and for any time-weighted dwell to come.
    pub dwell_cap_ms: u32,
}

impl Default for AoiParams {
    fn default() -> Self {
        Self {
            margin_norm: 0.0,
            dwell_cap_ms: DEFAULT_DWELL_CAP_MS,
        }
    }
}

/// `ScoringProfile.norms` (PRM-7, AGE-2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct NormsParams {
    pub min_n: u32,
    pub include_imputed: bool,
}

impl Default for NormsParams {
    fn default() -> Self {
        Self {
            min_n: crate::norms::DEFAULT_MIN_N as u32,
            include_imputed: false,
        }
    }
}

/// `ScoringProfile.review` (INV-1 backend rows).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ReviewParams {
    pub low_quality_valid_ratio: f64,
}

impl Default for ReviewParams {
    fn default() -> Self {
        Self {
            low_quality_valid_ratio: DEFAULT_LOW_QUALITY_VALID_RATIO,
        }
    }
}

fn default_low_confidence_valid_ratio() -> f64 {
    crate::scoring::LOW_CONFIDENCE_VALID_RATIO
}

/// `ageBands[].representativeMonths` of the five default bands (DSP-2, profile 1.3.0): the
/// iOS `midpointMonths` sent as `ageMonths` (`ChildProfile.swift:73-80`). The default source
/// of `asd_contract::config::default_age_bands`; the norm brackets carry none.
pub const DEFAULT_REPRESENTATIVE_MONTHS: [i64; 5] = [14, 20, 28, 40, 54];

/// The profile default `ageBands`: the norm brackets with [`DEFAULT_REPRESENTATIVE_MONTHS`].
pub fn default_age_bands() -> Vec<AgeBracket> {
    crate::norms::default_age_brackets()
        .into_iter()
        .zip(DEFAULT_REPRESENTATIVE_MONTHS)
        .map(|(b, months)| AgeBracket {
            representative_months: Some(months),
            ..b
        })
        .collect()
}

/// The members of a 1.2.0 `ScoringProfile` the core reads. Decodes from a full profile body
/// (`schemaVersion` and any member the core does not read are ignored); an absent section
/// takes today's constants, exactly like the contract type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisProfile {
    #[serde(default)]
    pub flag: FlagThresholds,
    #[serde(default = "default_low_confidence_valid_ratio")]
    pub low_confidence_valid_ratio: f64,
    #[serde(default)]
    pub quality_gate: QualityGate,
    #[serde(default)]
    pub tracking_quality: TrackingQualityParams,
    #[serde(default)]
    pub metrics: MetricsParams,
    #[serde(default)]
    pub aoi: AoiParams,
    #[serde(default = "default_age_bands")]
    pub age_bands: Vec<AgeBracket>,
    #[serde(default)]
    pub norms: NormsParams,
    #[serde(default)]
    pub review: ReviewParams,
}

impl Default for AnalysisProfile {
    fn default() -> Self {
        Self {
            flag: FlagThresholds::default(),
            low_confidence_valid_ratio: default_low_confidence_valid_ratio(),
            quality_gate: QualityGate::default(),
            tracking_quality: TrackingQualityParams::default(),
            metrics: MetricsParams::default(),
            aoi: AoiParams::default(),
            age_bands: default_age_bands(),
            norms: NormsParams::default(),
            review: ReviewParams::default(),
        }
    }
}

impl AnalysisProfile {
    /// The four gaze-processing sections, borrowed (PRM-8).
    pub fn gaze_processing(&self) -> GazeProcessingView<'_> {
        GazeProcessingView {
            quality_gate: &self.quality_gate,
            tracking_quality: &self.tracking_quality,
            metrics: &self.metrics,
            aoi: &self.aoi,
        }
    }
}

/// The sections a gaze-only caller needs (PRM-8), borrowed: the core twin of
/// `asd_contract::config::GazeProcessingView`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GazeProcessingView<'a> {
    pub quality_gate: &'a QualityGate,
    pub tracking_quality: &'a TrackingQualityParams,
    pub metrics: &'a MetricsParams,
    pub aoi: &'a AoiParams,
}
