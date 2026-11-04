//! owner: WP-X.0c
//!
//! Display bands (DSP-3): the traffic-light decision the iOS result and detail views made
//! with literal cut-offs (`EngagementRiskStratificationView`, `EngagementDetailViews`,
//! `EngagementAffectAndQualityViews`), moved into the core so every surface reads the
//! profile's `display` and `sessionQuality` sections (profile 1.3.0) instead of constants.
//!
//! Boundaries match the Swift operators exactly, so the screens do not change colour on the
//! default profile: `>=` for every minimum, `<` for the latency bounds, `<=` for
//! `flatAffect.okMax`, the non-social `100 - socialPreferencePct.okMin` bound and the
//! inclusive mouth range. Owner question 16 unified the variants on 60/50, 25/18 and
//! 550/750 ms.
//!
//! [`DisplayParams`] is the core twin of `asd_contract::config::DisplayParams` (flat: the
//! contract's nested `display.bands.<metric>.<bound>` leaves converted field by field); its
//! defaults are the default source of the contract's.

use serde::{Deserialize, Serialize};

use crate::quality::SessionQualityParams;

/// The metrics a result screen colours (DSP-3). Wire spelling is camelCase, also the
/// `display_metric.<metric>` catalogue key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DisplayMetric {
    /// Social-preference dwell share, percent.
    SocialPreferencePct,
    /// Non-social dwell share, percent (`100 - socialPreferencePct.okMin` as an upper bound).
    NonSocialPct,
    /// Eye-region dwell share, percent.
    EyeDwellPct,
    /// Mouth dwell share, percent (a range).
    MouthDwellPct,
    /// Latency of the first look at the eyes, milliseconds.
    FirstEyeLatencyMs,
    /// Smile intensity, percent.
    SmileIntensityPct,
    /// Smile reactivity index, 0..1.
    SmileReactivity,
    /// Surprise share, percent.
    SurprisePct,
    /// Flat-affect score, 0..1.
    FlatAffect,
    /// Valid-frame share, percent (`sessionQuality.optimalMinValidRatio x 100`).
    ValidFramePct,
}

impl DisplayMetric {
    /// Every metric, in declaration order.
    pub const ALL: [DisplayMetric; 10] = [
        DisplayMetric::SocialPreferencePct,
        DisplayMetric::NonSocialPct,
        DisplayMetric::EyeDwellPct,
        DisplayMetric::MouthDwellPct,
        DisplayMetric::FirstEyeLatencyMs,
        DisplayMetric::SmileIntensityPct,
        DisplayMetric::SmileReactivity,
        DisplayMetric::SurprisePct,
        DisplayMetric::FlatAffect,
        DisplayMetric::ValidFramePct,
    ];

    /// The wire spelling (`socialPreferencePct`).
    pub fn as_str(self) -> &'static str {
        match self {
            DisplayMetric::SocialPreferencePct => "socialPreferencePct",
            DisplayMetric::NonSocialPct => "nonSocialPct",
            DisplayMetric::EyeDwellPct => "eyeDwellPct",
            DisplayMetric::MouthDwellPct => "mouthDwellPct",
            DisplayMetric::FirstEyeLatencyMs => "firstEyeLatencyMs",
            DisplayMetric::SmileIntensityPct => "smileIntensityPct",
            DisplayMetric::SmileReactivity => "smileReactivity",
            DisplayMetric::SurprisePct => "surprisePct",
            DisplayMetric::FlatAffect => "flatAffect",
            DisplayMetric::ValidFramePct => "validFramePct",
        }
    }

    /// The metric of a wire spelling.
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.as_str() == s)
    }
}

/// The band a value falls in. Wire spelling snake_case (`not_observed`), also the
/// `display_band.<status>` catalogue key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BandStatus {
    Ok,
    Monitor,
    Outside,
    /// No value (the metric was not observed in the session).
    NotObserved,
}

impl BandStatus {
    pub const ALL: [BandStatus; 4] = [
        BandStatus::Ok,
        BandStatus::Monitor,
        BandStatus::Outside,
        BandStatus::NotObserved,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            BandStatus::Ok => "ok",
            BandStatus::Monitor => "monitor",
            BandStatus::Outside => "outside",
            BandStatus::NotObserved => "not_observed",
        }
    }
}

/// How the `ok` target reads (`>= 60 %`, `<= 0.25`, `< 550 ms`, `10 % - 30 %`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Comparator {
    /// `value >= low`.
    AtLeast,
    /// `value <= high`.
    AtMost,
    /// `value < high` (strict: the latency bounds).
    Below,
    /// `low <= value <= high`.
    Between,
}

impl Comparator {
    pub fn as_str(self) -> &'static str {
        match self {
            Comparator::AtLeast => "at_least",
            Comparator::AtMost => "at_most",
            Comparator::Below => "below",
            Comparator::Between => "between",
        }
    }
}

/// The `ok` target of a metric under the active parameters, for the target text.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BandTarget {
    pub comparator: Comparator,
    pub low: Option<f64>,
    pub high: Option<f64>,
    /// Registry unit wire spelling: `percent`, `millis` or `ratio`.
    pub unit: &'static str,
}

/// `display.bands` and `display.scanpathMaxGapMs` of a profile (DSP-1), flat.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DisplayParams {
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
    /// The scanpath line breaks across a sample gap longer than this.
    pub scanpath_max_gap_ms: u32,
}

impl Default for DisplayParams {
    /// Today's iOS cut-offs (technical §5.1 #4-#17, #22; owner question 16).
    fn default() -> Self {
        Self {
            social_preference_pct_ok_min: 60.0,
            social_preference_pct_monitor_min: 50.0,
            eye_dwell_pct_ok_min: 25.0,
            eye_dwell_pct_monitor_min: 18.0,
            mouth_dwell_pct_ok_min: 10.0,
            mouth_dwell_pct_ok_max: 30.0,
            first_eye_latency_ms_ok_max: 550.0,
            first_eye_latency_ms_monitor_max: 750.0,
            smile_intensity_pct_ok_min: 20.0,
            smile_intensity_pct_monitor_min: 10.0,
            smile_reactivity_ok_min: 0.30,
            surprise_pct_ok_min: 15.0,
            flat_affect_ok_max: 0.25,
            scanpath_max_gap_ms: 120,
        }
    }
}

const PERCENT: &str = "percent";
const MILLIS: &str = "millis";
const RATIO: &str = "ratio";

fn at_least(v: f64, ok: f64, monitor: Option<f64>) -> BandStatus {
    if v >= ok {
        BandStatus::Ok
    } else if monitor.is_some_and(|m| v >= m) {
        BandStatus::Monitor
    } else {
        BandStatus::Outside
    }
}

/// The band of `value` for `metric` (DSP-3), and the `ok` target to show next to it.
///
/// One-sided bands with a monitor bound give `Ok`/`Monitor`/`Outside`, one-sided bands
/// without one and ranges give `Ok`/`Outside`, `None` gives `NotObserved`. A NaN value
/// compares false everywhere, as in Swift: `Outside`.
pub fn band(
    metric: DisplayMetric,
    value: Option<f64>,
    display: &DisplayParams,
    quality: &SessionQualityParams,
) -> (BandStatus, BandTarget) {
    let d = display;
    let min_target = |low: f64, unit| BandTarget {
        comparator: Comparator::AtLeast,
        low: Some(low),
        high: None,
        unit,
    };
    let max_target = |comparator, high: f64, unit| BandTarget {
        comparator,
        low: None,
        high: Some(high),
        unit,
    };
    type Decide = Box<dyn Fn(f64) -> BandStatus>;
    let (target, decide): (BandTarget, Decide) = match metric {
        DisplayMetric::SocialPreferencePct => {
            let (ok, mon) = (
                d.social_preference_pct_ok_min,
                d.social_preference_pct_monitor_min,
            );
            (
                min_target(ok, PERCENT),
                Box::new(move |v| at_least(v, ok, Some(mon))),
            )
        }
        DisplayMetric::NonSocialPct => {
            let high = 100.0 - d.social_preference_pct_ok_min;
            (
                max_target(Comparator::AtMost, high, PERCENT),
                Box::new(move |v| {
                    if v <= high {
                        BandStatus::Ok
                    } else {
                        BandStatus::Outside
                    }
                }),
            )
        }
        DisplayMetric::EyeDwellPct => {
            let (ok, mon) = (d.eye_dwell_pct_ok_min, d.eye_dwell_pct_monitor_min);
            (
                min_target(ok, PERCENT),
                Box::new(move |v| at_least(v, ok, Some(mon))),
            )
        }
        DisplayMetric::MouthDwellPct => {
            let (lo, hi) = (d.mouth_dwell_pct_ok_min, d.mouth_dwell_pct_ok_max);
            (
                BandTarget {
                    comparator: Comparator::Between,
                    low: Some(lo),
                    high: Some(hi),
                    unit: PERCENT,
                },
                Box::new(move |v| {
                    if v >= lo && v <= hi {
                        BandStatus::Ok
                    } else {
                        BandStatus::Outside
                    }
                }),
            )
        }
        DisplayMetric::FirstEyeLatencyMs => {
            let (ok, mon) = (
                d.first_eye_latency_ms_ok_max,
                d.first_eye_latency_ms_monitor_max,
            );
            (
                max_target(Comparator::Below, ok, MILLIS),
                Box::new(move |v| {
                    if v < ok {
                        BandStatus::Ok
                    } else if v < mon {
                        BandStatus::Monitor
                    } else {
                        BandStatus::Outside
                    }
                }),
            )
        }
        DisplayMetric::SmileIntensityPct => {
            let (ok, mon) = (
                d.smile_intensity_pct_ok_min,
                d.smile_intensity_pct_monitor_min,
            );
            (
                min_target(ok, PERCENT),
                Box::new(move |v| at_least(v, ok, Some(mon))),
            )
        }
        DisplayMetric::SmileReactivity => {
            let ok = d.smile_reactivity_ok_min;
            (
                min_target(ok, RATIO),
                Box::new(move |v| at_least(v, ok, None)),
            )
        }
        DisplayMetric::SurprisePct => {
            let ok = d.surprise_pct_ok_min;
            (
                min_target(ok, PERCENT),
                Box::new(move |v| at_least(v, ok, None)),
            )
        }
        DisplayMetric::FlatAffect => {
            let high = d.flat_affect_ok_max;
            (
                max_target(Comparator::AtMost, high, RATIO),
                Box::new(move |v| {
                    if v <= high {
                        BandStatus::Ok
                    } else {
                        BandStatus::Outside
                    }
                }),
            )
        }
        DisplayMetric::ValidFramePct => {
            let ok = quality.optimal_min_valid_ratio * 100.0;
            (
                min_target(ok, PERCENT),
                Box::new(move |v| at_least(v, ok, None)),
            )
        }
    };
    let status = match value {
        None => BandStatus::NotObserved,
        Some(v) => decide(v),
    };
    (status, target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_spellings_round_trip() {
        for m in DisplayMetric::ALL {
            assert_eq!(DisplayMetric::parse(m.as_str()), Some(m));
            assert_eq!(
                serde_json::to_value(m).unwrap(),
                serde_json::Value::from(m.as_str())
            );
        }
        for s in BandStatus::ALL {
            assert_eq!(
                serde_json::to_value(s).unwrap(),
                serde_json::Value::from(s.as_str())
            );
        }
        assert_eq!(DisplayMetric::parse("nope"), None);
    }
}
