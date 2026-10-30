//! owner: WP-R.3
//!
//! Config documents and the signed config bundle (CON-5, ADR-B6).
//!
//! `StimulusCatalog` and `NormativeBaselineSet` are the `asd-core` domain types (CON-1,
//! camelCase like the bundled JSON). `ScoringProfile` is new; its defaults equal today's
//! Swift constants (`AttentionFlagAlgorithm`, `PerVideoScoringEngine.lowConfidenceValidRatio`,
//! `GazeQualityGate.init`). The bundle envelope is snake_case like the rest of `/v1`.

use std::collections::BTreeMap;

use asd_core::types::{NormativeBaselineSet, StimulusCatalog};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::mirror;

/// `$id` of the exported bundle schema.
pub const CONFIG_BUNDLE_SCHEMA_ID: &str = "https://schemas.curv.org/asd/config-bundle-v1.json";
/// Version written into a default [`ScoringProfile`].
pub const SCORING_PROFILE_SCHEMA_VERSION: &str = "1.0.0";

/// Traffic-light thresholds (`AttentionFlagAlgorithm`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FlagThresholds {
    /// Composite at or above which the flag can be green.
    pub green_floor: f64,
    /// Composite below which the flag is red.
    pub yellow_floor: f64,
    /// Share of scored clips below the quality gate that forces yellow.
    pub low_confidence_share: f64,
    /// Share of atypical rows that forces red even with a high composite.
    pub atypical_share_for_red: f64,
}

impl Default for FlagThresholds {
    fn default() -> Self {
        Self {
            green_floor: 0.65,
            yellow_floor: 0.35,
            low_confidence_share: 0.50,
            atypical_share_for_red: 0.40,
        }
    }
}

/// Per-sample quality gate parameters (`GazeQualityGate.init` defaults).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct QualityGateParams {
    pub min_distance_cm: f64,
    pub max_distance_cm: f64,
    pub max_yaw_rad: f64,
    pub max_pitch_rad: f64,
    pub min_confidence: f64,
    pub blink_threshold: f64,
}

impl Default for QualityGateParams {
    fn default() -> Self {
        Self {
            min_distance_cm: 35.0,
            max_distance_cm: 65.0,
            max_yaw_rad: 0.61,
            max_pitch_rad: 0.52,
            min_confidence: 0.50,
            blink_threshold: 0.45,
        }
    }
}

/// Scoring parameters delivered in the bundle (`scoring_profile` config document).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(example = ScoringProfile::default())]
pub struct ScoringProfile {
    pub schema_version: String,
    pub flag: FlagThresholds,
    /// Clips below this valid-sample ratio are low-confidence.
    pub low_confidence_valid_ratio: f64,
    pub quality_gate: QualityGateParams,
}

impl Default for ScoringProfile {
    fn default() -> Self {
        Self {
            schema_version: SCORING_PROFILE_SCHEMA_VERSION.into(),
            flag: FlagThresholds::default(),
            low_confidence_valid_ratio: 0.60,
            quality_gate: QualityGateParams::default(),
        }
    }
}

/// The signed bundle served by `GET /v1/config/bundle` (CON-5, CON-6).
///
/// `signature` is `ed25519:` + base64url (no padding) of the Ed25519 signature over the
/// RFC 8785 canonical form of this object with the `signature` member removed; `key_id`
/// names the signing key so keys can rotate. Use [`crate::signing`] to sign and verify.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[schemars(title = "ConfigBundle")]
pub struct ConfigBundle {
    pub bundle_version: String,
    pub key_id: String,
    #[schemars(with = "mirror::StimulusCatalog")]
    pub catalog: StimulusCatalog,
    #[schemars(with = "mirror::NormativeBaselineSet")]
    pub baselines: NormativeBaselineSet,
    pub scoring_profile: ScoringProfile,
    pub feature_flags: BTreeMap<String, bool>,
    #[schemars(extend("format" = "date-time"))]
    pub signed_at: String,
    #[schemars(regex(pattern = r"^ed25519:[A-Za-z0-9_-]+$"))]
    pub signature: String,
}
