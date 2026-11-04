//! owner: WP-R.3
//!
//! Config documents and the signed config bundle (CON-5, ADR-B6).
//!
//! `StimulusCatalog` and `NormativeBaselineSet` are the `asd-core` domain types (CON-1,
//! camelCase like the bundled JSON). `ScoringProfile` is new; its defaults equal today's
//! Swift constants (`AttentionFlagAlgorithm`, `PerVideoScoringEngine.lowConfidenceValidRatio`,
//! `GazeQualityGate.init`). The bundle envelope is snake_case like the rest of `/v1`.
//!
//! Profile schema 1.1.0 (SCH-3, WP-G.6) adds `trackingQuality`: the asd-core
//! [`TrackingQualityParams`] (re-exported, not copied), `serde(default)`, so a 1.0.0
//! profile or an older bundle decodes to the asd-core defaults.
//!
//! Profile schema 1.2.0 (PRM-7, WP-X.0) adds the knobs that were constants in asd-core and the
//! backend (`metrics`, `aoi`, `ageBands`, `norms`, `review`; INV-1). Every new member is
//! `serde(default)` to today's constant, so a 1.1.0 or 1.0.0 body decodes to exactly the
//! behaviour it had. The parameter registry ([`crate::params`]) describes every leaf.
//!
//! Profile schema 1.3.0 (DSP-1, DSP-2, WP-X.0c) adds `display` (the result screens' band
//! cut-offs, [`DisplayParams`]) and `sessionQuality` (the session verdict ratios,
//! [`SessionQualityParams`]), both `serde(default)` to today's iOS constants, and the optional
//! `ageBands[].representativeMonths` (the default profile sets 14, 20, 28, 40, 54). An older
//! body decodes to the same behaviour; no scoring input changes.

use std::collections::BTreeMap;

pub use asd_core::types::tracking::TrackingQualityParams;
use asd_core::types::{AgeBracket, NormativeBaselineSet, StimulusCatalog};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::mirror;

/// `$id` of the exported bundle schema.
pub const CONFIG_BUNDLE_SCHEMA_ID: &str = "https://schemas.curv.org/asd/config-bundle-v1.json";
/// Version written into a default [`ScoringProfile`].
pub const SCORING_PROFILE_SCHEMA_VERSION: &str = "1.3.0";

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

/// The core gate of the profile's parameters. Both sides are destructured / built field by
/// field, so a field added to either struct fails to compile here instead of being dropped
/// on the way to the pipeline (`asd-ffi::gaze_config_json_for_profile`).
impl From<&QualityGateParams> for asd_core::quality::QualityGate {
    fn from(p: &QualityGateParams) -> Self {
        let QualityGateParams {
            min_distance_cm,
            max_distance_cm,
            max_yaw_rad,
            max_pitch_rad,
            min_confidence,
            blink_threshold,
        } = *p;
        Self {
            min_distance_cm,
            max_distance_cm,
            max_yaw_rad,
            max_pitch_rad,
            min_confidence,
            blink_threshold,
        }
    }
}

impl From<QualityGateParams> for asd_core::quality::QualityGate {
    fn from(p: QualityGateParams) -> Self {
        Self::from(&p)
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
    /// Gaze-robustness thresholds (SCH-3). Absent in a 1.0.0 profile: the asd-core defaults.
    #[serde(default)]
    #[schemars(with = "mirror::TrackingQualityParams")]
    pub tracking_quality: TrackingQualityParams,
    /// Fixation and affect extraction (PRM-7, 1.2.0). Absent before 1.2.0: today's constants.
    #[serde(default)]
    pub metrics: MetricsParams,
    /// AOI hit testing (PRM-7, 1.2.0).
    #[serde(default)]
    pub aoi: AoiParams,
    /// Age bands of norms and analysis (PRM-7, AGE-2): contiguous, ascending, 1..=12 bands,
    /// months 0..=240. Absent before 1.2.0: the five `asd_core::norms` brackets.
    #[serde(default = "default_age_bands")]
    #[schemars(with = "Vec<mirror::AgeBracket>")]
    pub age_bands: Vec<AgeBracket>,
    /// Norm aggregation (PRM-7, AGE-2).
    #[serde(default)]
    pub norms: NormsParams,
    /// Backend review rules (INV-1 backend rows, 1.2.0).
    #[serde(default)]
    pub review: ReviewParams,
    /// Result-screen band cut-offs (DSP-1, 1.3.0). Display only: no stored value depends on it.
    #[serde(default)]
    pub display: DisplayParams,
    /// Session verdict ratios (DSP-1, DSP-4, 1.3.0): the stored `qualityVerdict` of new
    /// sessions.
    #[serde(default)]
    pub session_quality: SessionQualityParams,
}

impl Default for ScoringProfile {
    fn default() -> Self {
        Self {
            schema_version: SCORING_PROFILE_SCHEMA_VERSION.into(),
            flag: FlagThresholds::default(),
            low_confidence_valid_ratio: asd_core::scoring::LOW_CONFIDENCE_VALID_RATIO,
            quality_gate: QualityGateParams::default(),
            tracking_quality: TrackingQualityParams::default(),
            metrics: MetricsParams::default(),
            aoi: AoiParams::default(),
            age_bands: default_age_bands(),
            norms: NormsParams::default(),
            review: ReviewParams::default(),
            display: DisplayParams::default(),
            session_quality: SessionQualityParams::default(),
        }
    }
}

impl ScoringProfile {
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

/// `representativeMonths` of the five default bands (DSP-2): the `midpointMonths` the iOS app
/// sends as `ageMonths` today (`ChildProfile.swift:73-80`), so sessions keep the same age.
pub use asd_core::analysis::profile::DEFAULT_REPRESENTATIVE_MONTHS;

/// The five `asd_core::norms` default brackets (12-17 .. 48-60 months) with their
/// representative months (DSP-2; `asd_core::analysis::profile::default_age_bands`).
pub fn default_age_bands() -> Vec<AgeBracket> {
    asd_core::analysis::profile::default_age_bands()
}

/// The age in months a session in `band` carries (DSP-2, DSP-10): its `representativeMonths`;
/// absent, the default band's value when the band is one of the five defaults (same id and
/// bounds, so a pre-1.3.0 profile keeps today's 14, 20, 28, 40, 54), else
/// `(minMonths + maxMonths) / 2`.
pub fn representative_months(band: &AgeBracket) -> i64 {
    if let Some(m) = band.representative_months {
        return m;
    }
    default_age_bands()
        .into_iter()
        .find(|d| {
            d.id == band.id && d.min_months == band.min_months && d.max_months == band.max_months
        })
        .and_then(|d| d.representative_months)
        .unwrap_or_else(|| band.representative_or_midpoint())
}

/// Unit of the I-VT velocity threshold (`metrics.ivtUnit`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum IvtUnit {
    /// Normalised viewport units per second (`metrics.ivtVelocityUnitsPerSec`): today's rule.
    #[default]
    Norm,
    /// Degrees of visual angle per second (`metrics.ivtVelocityDegPerSec`, SCR-3; owner gate
    /// H-G2).
    Deg,
}

impl IvtUnit {
    /// The wire values in declaration order.
    pub const ALL: [IvtUnit; 2] = [IvtUnit::Norm, IvtUnit::Deg];

    pub fn as_str(self) -> &'static str {
        match self {
            IvtUnit::Norm => "norm",
            IvtUnit::Deg => "deg",
        }
    }
}

/// Fixation and affect extraction parameters (PRM-7). Defaults are the asd-core constants.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct MetricsParams {
    /// I-VT threshold in normalised viewport units per second (`ivtUnit = norm`).
    pub ivt_velocity_units_per_sec: f64,
    /// I-VT threshold in degrees per second (`ivtUnit = deg`).
    pub ivt_velocity_deg_per_sec: f64,
    pub ivt_unit: IvtUnit,
    /// Shortest fixation counted, seconds.
    pub min_fixation_duration_sec: f64,
    /// Frame interval assumed where timestamps do not give one (60 Hz).
    pub fallback_frame_interval_sec: f64,
    /// Window after a clip onset in which a smile counts as reactive, seconds.
    pub smile_reactivity_window_sec: f64,
    /// Gain of the flat-affect score (`1 - mean expressivity x gain`, clamped to [0, 1]).
    pub flat_affect_gain: f64,
}

impl Default for MetricsParams {
    fn default() -> Self {
        Self {
            ivt_velocity_units_per_sec: asd_core::metrics::VELOCITY_THRESHOLD_UNITS_PER_SEC,
            ivt_velocity_deg_per_sec: 30.0,
            ivt_unit: IvtUnit::Norm,
            min_fixation_duration_sec: asd_core::metrics::MIN_FIXATION_DURATION_SEC,
            fallback_frame_interval_sec: asd_core::metrics::FRAME_INTERVAL_SEC,
            smile_reactivity_window_sec: 0.5,
            flat_affect_gain: 50.0,
        }
    }
}

/// AOI hit-test parameters (PRM-7).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct AoiParams {
    /// Grows (positive) or shrinks (negative) every AOI box, normalised screen units.
    pub margin_norm: f64,
    /// Longest interval one frame contributes to a dwell sum, milliseconds.
    pub dwell_cap_ms: u32,
}

impl Default for AoiParams {
    fn default() -> Self {
        Self {
            margin_norm: 0.0,
            dwell_cap_ms: 100,
        }
    }
}

/// Norm aggregation parameters (PRM-7, AGE-2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct NormsParams {
    /// Fewest observations a norm cell (and an analysis band, AGE-3) needs.
    pub min_n: u32,
    /// Whether rows flagged `imputed` enter the distributions.
    pub include_imputed: bool,
}

impl Default for NormsParams {
    fn default() -> Self {
        Self {
            min_n: asd_core::norms::DEFAULT_MIN_N as u32,
            include_imputed: false,
        }
    }
}

/// Backend review rules (INV-1 backend rows): the valid-frame ratio below which a session is
/// queued for review as low quality (`auto_review`), also the norm job's session floor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct ReviewParams {
    pub low_quality_valid_ratio: f64,
}

impl Default for ReviewParams {
    fn default() -> Self {
        Self {
            low_quality_valid_ratio: 0.80,
        }
    }
}

// ---- 1.3.0: display bands and session quality (DSP-1) ----------------------------------

macro_rules! band_section {
    ($(#[$meta:meta])* $name:ident { $($(#[$fmeta:meta])* $field:ident = $default:ident),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
        #[serde(rename_all = "camelCase", default)]
        pub struct $name {
            $($(#[$fmeta])* pub $field: f64,)+
        }

        impl Default for $name {
            fn default() -> Self {
                let d = asd_core::display::DisplayParams::default();
                Self { $($field: d.$default,)+ }
            }
        }
    };
}

band_section!(
    /// `display.bands.socialPreferencePct`, percent: `>= okMin` ok, `>= monitorMin` monitor.
    /// Also the non-social bound (`100 - okMin`).
    SocialPreferenceBand {
        ok_min = social_preference_pct_ok_min,
        monitor_min = social_preference_pct_monitor_min,
    }
);
band_section!(
    /// `display.bands.eyeDwellPct`, percent: `>= okMin` ok, `>= monitorMin` monitor.
    EyeDwellBand {
        ok_min = eye_dwell_pct_ok_min,
        monitor_min = eye_dwell_pct_monitor_min,
    }
);
band_section!(
    /// `display.bands.mouthDwellPct`, percent: `okMin <= x <= okMax` ok.
    MouthDwellBand {
        ok_min = mouth_dwell_pct_ok_min,
        ok_max = mouth_dwell_pct_ok_max,
    }
);
band_section!(
    /// `display.bands.firstEyeLatencyMs`, milliseconds: `< okMax` ok, `< monitorMax` monitor.
    FirstEyeLatencyBand {
        ok_max = first_eye_latency_ms_ok_max,
        monitor_max = first_eye_latency_ms_monitor_max,
    }
);
band_section!(
    /// `display.bands.smileIntensityPct`, percent: `>= okMin` ok, `>= monitorMin` monitor.
    SmileIntensityBand {
        ok_min = smile_intensity_pct_ok_min,
        monitor_min = smile_intensity_pct_monitor_min,
    }
);
band_section!(
    /// `display.bands.smileReactivity`, index 0..1: `>= okMin` ok.
    SmileReactivityBand {
        ok_min = smile_reactivity_ok_min,
    }
);
band_section!(
    /// `display.bands.surprisePct`, percent: `>= okMin` ok.
    SurpriseBand {
        ok_min = surprise_pct_ok_min,
    }
);
band_section!(
    /// `display.bands.flatAffect`, score 0..1: `<= okMax` ok.
    FlatAffectBand {
        ok_max = flat_affect_ok_max,
    }
);

/// `display.bands` (DSP-1): one object per coloured metric.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct DisplayBands {
    pub social_preference_pct: SocialPreferenceBand,
    pub eye_dwell_pct: EyeDwellBand,
    pub mouth_dwell_pct: MouthDwellBand,
    pub first_eye_latency_ms: FirstEyeLatencyBand,
    pub smile_intensity_pct: SmileIntensityBand,
    pub smile_reactivity: SmileReactivityBand,
    pub surprise_pct: SurpriseBand,
    pub flat_affect: FlatAffectBand,
}

/// `display` (DSP-1, 1.3.0): the result screens' band cut-offs and the scanpath gap. Read by
/// the device views through `asd_core::display::band`; no stored value depends on it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct DisplayParams {
    pub bands: DisplayBands,
    /// The scanpath line breaks across a sample gap longer than this, milliseconds.
    pub scanpath_max_gap_ms: u32,
}

impl Default for DisplayParams {
    fn default() -> Self {
        Self {
            bands: DisplayBands::default(),
            scanpath_max_gap_ms: asd_core::display::DisplayParams::default().scanpath_max_gap_ms,
        }
    }
}

/// The core twin, field by field (a field added to either side fails to compile here).
impl From<&DisplayParams> for asd_core::display::DisplayParams {
    fn from(p: &DisplayParams) -> Self {
        let DisplayParams {
            bands:
                DisplayBands {
                    social_preference_pct: s,
                    eye_dwell_pct: e,
                    mouth_dwell_pct: m,
                    first_eye_latency_ms: l,
                    smile_intensity_pct: si,
                    smile_reactivity: sr,
                    surprise_pct: su,
                    flat_affect: fa,
                },
            scanpath_max_gap_ms,
        } = *p;
        Self {
            social_preference_pct_ok_min: s.ok_min,
            social_preference_pct_monitor_min: s.monitor_min,
            eye_dwell_pct_ok_min: e.ok_min,
            eye_dwell_pct_monitor_min: e.monitor_min,
            mouth_dwell_pct_ok_min: m.ok_min,
            mouth_dwell_pct_ok_max: m.ok_max,
            first_eye_latency_ms_ok_max: l.ok_max,
            first_eye_latency_ms_monitor_max: l.monitor_max,
            smile_intensity_pct_ok_min: si.ok_min,
            smile_intensity_pct_monitor_min: si.monitor_min,
            smile_reactivity_ok_min: sr.ok_min,
            surprise_pct_ok_min: su.ok_min,
            flat_affect_ok_max: fa.ok_max,
            scanpath_max_gap_ms,
        }
    }
}

/// `sessionQuality` (DSP-1, DSP-4, 1.3.0): the valid-frame ratios of the session verdict.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct SessionQualityParams {
    /// At or above: `optimal`.
    pub optimal_min_valid_ratio: f64,
    /// At or above (and below optimal): `acceptable`; below: `degraded`.
    pub acceptable_min_valid_ratio: f64,
}

impl Default for SessionQualityParams {
    fn default() -> Self {
        let d = asd_core::quality::SessionQualityParams::default();
        Self {
            optimal_min_valid_ratio: d.optimal_min_valid_ratio,
            acceptable_min_valid_ratio: d.acceptable_min_valid_ratio,
        }
    }
}

impl From<&SessionQualityParams> for asd_core::quality::SessionQualityParams {
    fn from(p: &SessionQualityParams) -> Self {
        let SessionQualityParams {
            optimal_min_valid_ratio,
            acceptable_min_valid_ratio,
        } = *p;
        Self {
            optimal_min_valid_ratio,
            acceptable_min_valid_ratio,
        }
    }
}

/// The four sections a gaze-only caller needs (PRM-8: `gaze::reprocess`,
/// `gaze_config_json_for_profile`). A borrowed view, not a document.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GazeProcessingView<'a> {
    pub quality_gate: &'a QualityGateParams,
    pub tracking_quality: &'a TrackingQualityParams,
    pub metrics: &'a MetricsParams,
    pub aoi: &'a AoiParams,
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
