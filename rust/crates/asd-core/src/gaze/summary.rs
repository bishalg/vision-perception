//! owner: WP-G.4
//!
//! Session `TrackingQuality` summary (QS-1), `estimatedAccuracyDeg` (QS-2) and the verdict
//! (QS-4).
//!
//! The pipeline (WP-G.3) owns the per-frame accounting (loss sums, confidence, `validRatio`)
//! and hands the finished pieces to [`tracking_quality`], which adds the derived fields.
//! Unmeasured values stay `None` and serialise as absent keys (CON-14); nothing is defaulted.

use serde::{Deserialize, Serialize};

use crate::gaze::drift::max_drift_deg;
use crate::gaze::validation::{median, quantile};
use crate::gaze::PIPELINE_VERSION;
use crate::types::tracking::{
    CalibrationQuality, CalibrationRecord, ConfidenceSummary, DriftCheckRecord, GeometrySource,
    LossSummary, ProcessedSample, RecalibrationRecord, TrackState, TrackingConditions,
    TrackingDevice, TrackingQuality, TrackingQualityParams, TrackingVerdict, ValidationRecord,
    VerdictParams,
};

/// QS-2. `max(validation accuracy, median hit check offset)` when both exist; the validation
/// accuracy when no check was hit; the check median when there is no validation accuracy and
/// at least two checks were hit; otherwise absent. A validation record without an accuracy
/// (no target hit) counts as no validation.
pub fn estimated_accuracy_deg(
    validation: Option<&ValidationRecord>,
    checks: &[DriftCheckRecord],
) -> Option<f64> {
    let val = validation.and_then(|v| v.accuracy_deg);
    let offsets: Vec<f64> = checks
        .iter()
        .filter(|c| !c.missed)
        .filter_map(|c| c.offset_deg)
        .collect();
    let check_median = median(offsets.iter().copied());
    match (val, check_median) {
        (Some(v), Some(c)) => Some(v.max(c)),
        (Some(v), None) => Some(v),
        (None, Some(c)) if offsets.len() >= 2 => Some(c),
        _ => None,
    }
}

/// Inputs of the QS-4 verdict.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VerdictInput {
    pub estimated_accuracy_deg: Option<f64>,
    /// The session's `validRatio` (existing quality summary).
    pub valid_ratio: f64,
    /// Share of recorded frame time not `tracked`, in [0,1].
    pub data_loss: f64,
    pub geometry_source: GeometrySource,
    pub calibration_quality: CalibrationQuality,
    /// A `complete` validation (VAL-2) followed the calibration in effect. Without one the
    /// accuracy is not verified by a validation (absent, incomplete, or only the QS-2
    /// drift-check fallback) and the verdict is at most `fair`.
    pub validation_complete: bool,
}

/// QS-4 verdict. Band upper bounds are inclusive; non-finite inputs fail towards the worse
/// verdict (a NaN accuracy or valid ratio is `unusable`, a NaN data loss is at least `poor`).
/// Besides the QS-4 `fair` caps, a session without a `complete` validation is at most `fair`
/// (an accuracy taken from drift checks alone, or from fewer than `minTargetsHit` validation
/// targets, is not enough for `good`).
pub fn verdict(input: &VerdictInput, params: &VerdictParams) -> TrackingVerdict {
    let bands = &params.accuracy_bands;
    let acc = match input.estimated_accuracy_deg {
        Some(a) if a.is_finite() => a,
        _ => return TrackingVerdict::Unusable,
    };
    if acc > bands.poor_max_deg
        || input.valid_ratio.is_nan()
        || input.valid_ratio < params.unusable_below_valid_ratio
    {
        return TrackingVerdict::Unusable;
    }
    if acc > bands.fair_max_deg
        || input.data_loss.is_nan()
        || input.data_loss > params.poor_above_data_loss
    {
        return TrackingVerdict::Poor;
    }
    if acc > bands.good_max_deg
        || input.data_loss > params.fair_above_data_loss
        || input.geometry_source == GeometrySource::IdiomDefault
        || input.calibration_quality == CalibrationQuality::Skipped
        || !input.validation_complete
    {
        return TrackingVerdict::Fair;
    }
    TrackingVerdict::Good
}

/// QS-1 `conditions` over the `tracked` frames: distance median and IQR (p75 - p25, linear
/// interpolation), head angular speed p90, ambient intensity median. `lightAsymmetry` is not
/// derivable from processed samples and stays absent; `glasses` is the operator's answer.
pub fn conditions(samples: &[ProcessedSample], glasses: Option<bool>) -> TrackingConditions {
    let tracked = || {
        samples
            .iter()
            .filter(|s| s.track_state == TrackState::Tracked)
    };
    let distances = || tracked().filter_map(|s| s.distance_cm);
    let iqr = match (quantile(distances(), 0.75), quantile(distances(), 0.25)) {
        (Some(hi), Some(lo)) => Some(hi - lo),
        _ => None,
    };
    TrackingConditions {
        distance_cm_median: median(distances()),
        distance_cm_iqr: iqr,
        head_speed_deg_per_sec_p90: quantile(
            tracked().filter_map(|s| s.head_speed_deg_per_sec),
            0.9,
        ),
        ambient_intensity_median: median(tracked().filter_map(|s| s.ambient_intensity)),
        light_asymmetry: None,
        glasses,
    }
}

/// `device.sampleRateHz` = 1 / median frame interval over all frames; absent with fewer than
/// two frames or a non-positive median interval.
pub fn sample_rate_hz(samples: &[ProcessedSample]) -> Option<f64> {
    let interval = median(samples.windows(2).map(|w| w[1].t_sec - w[0].t_sec))?;
    (interval > 0.0).then(|| 1.0 / interval)
}

/// The finished pieces of a session summary (the pipeline's accounting).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SummaryInput {
    pub device: TrackingDevice,
    pub calibration: CalibrationRecord,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validation: Option<ValidationRecord>,
    #[serde(default)]
    pub drift_checks: Vec<DriftCheckRecord>,
    #[serde(default)]
    pub recalibrations: Vec<RecalibrationRecord>,
    pub conditions: TrackingConditions,
    pub loss: LossSummary,
    pub confidence: ConfidenceSummary,
    /// The session's `validRatio`.
    pub valid_ratio: f64,
    /// Share of recorded frame time not `tracked`.
    pub data_loss: f64,
}

/// Assembles `trackingQuality` (QS-1): versions, `maxDriftDeg`, `estimatedAccuracyDeg` (QS-2)
/// and the verdict (QS-4, with the profile's `verdict` params).
pub fn tracking_quality(input: SummaryInput, params: &TrackingQualityParams) -> TrackingQuality {
    let estimated = estimated_accuracy_deg(input.validation.as_ref(), &input.drift_checks);
    let v = verdict(
        &VerdictInput {
            estimated_accuracy_deg: estimated,
            valid_ratio: input.valid_ratio,
            data_loss: input.data_loss,
            geometry_source: input.device.geometry_source,
            calibration_quality: input.calibration.quality,
            validation_complete: input.validation.as_ref().is_some_and(|v| v.complete),
        },
        &params.verdict,
    );
    TrackingQuality {
        pipeline_version: PIPELINE_VERSION.to_string(),
        params_version: params.schema_version.clone(),
        max_drift_deg: max_drift_deg(&input.drift_checks),
        device: input.device,
        calibration: input.calibration,
        validation: input.validation,
        drift_checks: input.drift_checks,
        recalibrations: input.recalibrations,
        conditions: input.conditions,
        loss: input.loss,
        confidence: input.confidence,
        estimated_accuracy_deg: estimated,
        verdict: v,
    }
}
