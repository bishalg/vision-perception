//! owner: WP-X.0c
//!
//! DSP-5: the profile 1.3.0 `display` and `sessionQuality` sections and the `ageBands` for
//! the device views (WP-M.1), as UniFFI records (no JSON on the hot path): decode once per
//! configuration with [`display_params_from_profile`] / [`age_bands_from_profile`], then
//! call [`display_band`] and [`session_quality_verdict`] per value. The decisions are
//! `asd_core::display::band` and `asd_core::quality::session_verdict`; nothing is decided
//! here.
//!
//! Tolerant by design: a profile without the 1.3.0 sections (1.2.0 or older), or with a
//! section that does not decode, gives that section's defaults, never an error. Only text
//! that is not a JSON object is an `AsdError::InvalidJson`.

use asd_contract::config::{self as contract, AoiParams, DisplayParams, SessionQualityParams};
use asd_core::display::{self, BandStatus, Comparator, DisplayMetric};
use asd_core::quality;
use asd_core::types::AgeBracket;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::{invalid_json, AsdError};

/// `asd_core::display::DisplayMetric`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum AsdDisplayMetric {
    SocialPreferencePct,
    NonSocialPct,
    EyeDwellPct,
    MouthDwellPct,
    FirstEyeLatencyMs,
    SmileIntensityPct,
    SmileReactivity,
    SurprisePct,
    FlatAffect,
    ValidFramePct,
}

impl From<AsdDisplayMetric> for DisplayMetric {
    fn from(m: AsdDisplayMetric) -> Self {
        match m {
            AsdDisplayMetric::SocialPreferencePct => DisplayMetric::SocialPreferencePct,
            AsdDisplayMetric::NonSocialPct => DisplayMetric::NonSocialPct,
            AsdDisplayMetric::EyeDwellPct => DisplayMetric::EyeDwellPct,
            AsdDisplayMetric::MouthDwellPct => DisplayMetric::MouthDwellPct,
            AsdDisplayMetric::FirstEyeLatencyMs => DisplayMetric::FirstEyeLatencyMs,
            AsdDisplayMetric::SmileIntensityPct => DisplayMetric::SmileIntensityPct,
            AsdDisplayMetric::SmileReactivity => DisplayMetric::SmileReactivity,
            AsdDisplayMetric::SurprisePct => DisplayMetric::SurprisePct,
            AsdDisplayMetric::FlatAffect => DisplayMetric::FlatAffect,
            AsdDisplayMetric::ValidFramePct => DisplayMetric::ValidFramePct,
        }
    }
}

/// `asd_core::display::BandStatus` (catalogue key `display_band.<status>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum AsdBandStatus {
    Ok,
    Monitor,
    Outside,
    NotObserved,
}

/// `asd_core::display::Comparator`: how the `ok` target reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum AsdComparator {
    /// `>= low`.
    AtLeast,
    /// `<= high`.
    AtMost,
    /// `< high` (the latency bounds).
    Below,
    /// `low ... high`, inclusive.
    Between,
}

/// The band of one value and the `ok` target to show next to it.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct AsdBand {
    pub status: AsdBandStatus,
    pub comparator: AsdComparator,
    pub low: Option<f64>,
    pub high: Option<f64>,
    /// Registry unit wire spelling: `percent`, `millis` or `ratio`.
    pub unit: String,
}

/// The 16 1.3.0 leaves (`display.*`, `sessionQuality.*`) plus `aoi.dwellCapMs`, flat, so the
/// kit decodes one record per configuration.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct AsdDisplayParams {
    pub social_preference_pct_ok_min: f64,
    pub social_preference_pct_monitor_min: f64,
    pub eye_dwell_pct_ok_min: f64,
    pub eye_dwell_pct_monitor_min: f64,
    pub mouth_dwell_pct_ok_min: f64,
    pub mouth_dwell_pct_ok_max: f64,
    pub first_eye_latency_ms_ok_max: f64,
    pub first_eye_latency_ms_monitor_max: f64,
    pub smile_intensity_pct_ok_min: f64,
    pub smile_intensity_pct_monitor_min: f64,
    pub smile_reactivity_ok_min: f64,
    pub surprise_pct_ok_min: f64,
    pub flat_affect_ok_max: f64,
    pub scanpath_max_gap_ms: u32,
    pub optimal_min_valid_ratio: f64,
    pub acceptable_min_valid_ratio: f64,
    /// `aoi.dwellCapMs` (1.2.0): the kit's per-frame dwell cap.
    pub aoi_dwell_cap_ms: u32,
}

impl AsdDisplayParams {
    fn new(display: &DisplayParams, sq: &SessionQualityParams, aoi: &AoiParams) -> Self {
        let d = display::DisplayParams::from(display);
        let q = quality::SessionQualityParams::from(sq);
        Self {
            social_preference_pct_ok_min: d.social_preference_pct_ok_min,
            social_preference_pct_monitor_min: d.social_preference_pct_monitor_min,
            eye_dwell_pct_ok_min: d.eye_dwell_pct_ok_min,
            eye_dwell_pct_monitor_min: d.eye_dwell_pct_monitor_min,
            mouth_dwell_pct_ok_min: d.mouth_dwell_pct_ok_min,
            mouth_dwell_pct_ok_max: d.mouth_dwell_pct_ok_max,
            first_eye_latency_ms_ok_max: d.first_eye_latency_ms_ok_max,
            first_eye_latency_ms_monitor_max: d.first_eye_latency_ms_monitor_max,
            smile_intensity_pct_ok_min: d.smile_intensity_pct_ok_min,
            smile_intensity_pct_monitor_min: d.smile_intensity_pct_monitor_min,
            smile_reactivity_ok_min: d.smile_reactivity_ok_min,
            surprise_pct_ok_min: d.surprise_pct_ok_min,
            flat_affect_ok_max: d.flat_affect_ok_max,
            scanpath_max_gap_ms: d.scanpath_max_gap_ms,
            optimal_min_valid_ratio: q.optimal_min_valid_ratio,
            acceptable_min_valid_ratio: q.acceptable_min_valid_ratio,
            aoi_dwell_cap_ms: aoi.dwell_cap_ms,
        }
    }

    /// The defaults of the bundled profile (`ScoringProfile::default()`).
    pub fn defaults() -> Self {
        Self::new(
            &DisplayParams::default(),
            &SessionQualityParams::default(),
            &AoiParams::default(),
        )
    }

    fn core(&self) -> (display::DisplayParams, quality::SessionQualityParams) {
        (
            display::DisplayParams {
                social_preference_pct_ok_min: self.social_preference_pct_ok_min,
                social_preference_pct_monitor_min: self.social_preference_pct_monitor_min,
                eye_dwell_pct_ok_min: self.eye_dwell_pct_ok_min,
                eye_dwell_pct_monitor_min: self.eye_dwell_pct_monitor_min,
                mouth_dwell_pct_ok_min: self.mouth_dwell_pct_ok_min,
                mouth_dwell_pct_ok_max: self.mouth_dwell_pct_ok_max,
                first_eye_latency_ms_ok_max: self.first_eye_latency_ms_ok_max,
                first_eye_latency_ms_monitor_max: self.first_eye_latency_ms_monitor_max,
                smile_intensity_pct_ok_min: self.smile_intensity_pct_ok_min,
                smile_intensity_pct_monitor_min: self.smile_intensity_pct_monitor_min,
                smile_reactivity_ok_min: self.smile_reactivity_ok_min,
                surprise_pct_ok_min: self.surprise_pct_ok_min,
                flat_affect_ok_max: self.flat_affect_ok_max,
                scanpath_max_gap_ms: self.scanpath_max_gap_ms,
            },
            quality::SessionQualityParams {
                optimal_min_valid_ratio: self.optimal_min_valid_ratio,
                acceptable_min_valid_ratio: self.acceptable_min_valid_ratio,
            },
        )
    }
}

/// One profile age band with its effective representative months (DSP-2).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AsdAgeBand {
    pub id: String,
    pub min_months: i64,
    pub max_months: i64,
    /// `representativeMonths`; absent in the profile: the default band's value for one of the
    /// five default bands, else `(minMonths + maxMonths) / 2`. The `ageMonths` of a session.
    pub representative_months: i64,
}

fn profile_object(profile_json: &str) -> Result<serde_json::Map<String, Value>, AsdError> {
    match serde_json::from_str::<Value>(profile_json) {
        Ok(Value::Object(map)) => Ok(map),
        Ok(_) => Err(invalid_json("scoring profile", "not a JSON object")),
        Err(e) => Err(invalid_json("scoring profile", e)),
    }
}

/// The member `key` decoded, or its default when absent or not decodable.
fn section<T: DeserializeOwned + Default>(map: &serde_json::Map<String, Value>, key: &str) -> T {
    map.get(key)
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default()
}

/// DSP-5: the display parameters of a `ScoringProfile` JSON (the bundled default or the
/// bundle's `scoring_profile_json`). Missing or undecodable sections give their defaults.
#[uniffi::export]
pub fn display_params_from_profile(profile_json: String) -> Result<AsdDisplayParams, AsdError> {
    let map = profile_object(&profile_json)?;
    Ok(AsdDisplayParams::new(
        &section::<DisplayParams>(&map, "display"),
        &section::<SessionQualityParams>(&map, "sessionQuality"),
        &section::<AoiParams>(&map, "aoi"),
    ))
}

/// DSP-5: the band of `value` (`None`: not observed) for `metric` under `params`
/// (`asd_core::display::band`).
#[uniffi::export]
pub fn display_band(
    metric: AsdDisplayMetric,
    value: Option<f64>,
    params: AsdDisplayParams,
) -> AsdBand {
    let (d, q) = params.core();
    let (status, target) = display::band(metric.into(), value, &d, &q);
    AsdBand {
        status: match status {
            BandStatus::Ok => AsdBandStatus::Ok,
            BandStatus::Monitor => AsdBandStatus::Monitor,
            BandStatus::Outside => AsdBandStatus::Outside,
            BandStatus::NotObserved => AsdBandStatus::NotObserved,
        },
        comparator: match target.comparator {
            Comparator::AtLeast => AsdComparator::AtLeast,
            Comparator::AtMost => AsdComparator::AtMost,
            Comparator::Below => AsdComparator::Below,
            Comparator::Between => AsdComparator::Between,
        },
        low: target.low,
        high: target.high,
        unit: target.unit.to_owned(),
    }
}

/// DSP-4/5: the session `qualityVerdict` wire spelling (`optimal`, `acceptable`, `degraded`)
/// of a valid-frame ratio under `params` (`asd_core::quality::session_verdict`).
#[uniffi::export]
pub fn session_quality_verdict(
    valid_ratio: f64,
    interrupted: bool,
    params: AsdDisplayParams,
) -> String {
    let (_, q) = params.core();
    let verdict = quality::session_verdict(valid_ratio, interrupted, &q);
    serde_json::to_value(verdict)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}

/// DSP-5: the profile's `ageBands` with their representative months. Missing or undecodable
/// `ageBands` give the five default bands (14, 20, 28, 40, 54).
#[uniffi::export]
pub fn age_bands_from_profile(profile_json: String) -> Result<Vec<AsdAgeBand>, AsdError> {
    let map = profile_object(&profile_json)?;
    let bands: Vec<AgeBracket> = map
        .get("ageBands")
        .and_then(|v| serde_json::from_value::<Vec<AgeBracket>>(v.clone()).ok())
        .filter(|b| !b.is_empty())
        .unwrap_or_else(contract::default_age_bands);
    Ok(bands
        .iter()
        .map(|b| AsdAgeBand {
            id: b.id.clone(),
            min_months: b.min_months,
            max_months: b.max_months,
            representative_months: contract::representative_months(b),
        })
        .collect())
}
