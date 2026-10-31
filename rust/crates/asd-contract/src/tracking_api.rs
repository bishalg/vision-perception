//! owner: WP-G.11
//!
//! `/v1` view of a session's tracking-quality record (SCH-4 of
//! `docs/spec/modules/asd-gaze-robustness/technical.md`): the QS-1 object of
//! `rawPayload.trackingQuality` (schema 1.7) with snake_case keys, as every `/v1` body.
//! `SessionDetail.tracking_quality` carries it; `null` for a 1.6 session (recorded by an app
//! version before tracking quality) or a 1.7 session without the record.
//!
//! Values are the record's, unchanged: enum values keep their record spelling
//! (`screenAffine`, `landscapeLeft`), and a value the device did not measure is `null`,
//! never defaulted (CON-14). Each type converts from its asd-core original with an
//! exhaustive destructuring, so a field added to the record fails to compile here until the
//! API decides how to expose it.
//!
//! Type names carry a `Session`/`Tracking` prefix so they cannot collide in `api-v1.json`
//! with the camelCase schema mirrors of the 1.7 ingest document.

use asd_core::types::tracking::{
    CalModelKind, CalibrationPose, CalibrationQuality, CalibrationRecord, ConfidenceSummary,
    DriftCheckRecord, GeometrySource, LossSummary, Orientation, RecalReason, RecalibrationRecord,
    TrackingConditions, TrackingDevice, TrackingQuality, TrackingVerdict, ValidationRecord,
    ValidationTargetRecord,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::mirror;

/// `SessionDetail.tracking_quality` (QS-1, snake_case).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SessionTrackingQuality {
    /// asd-core gaze maths version (`asd-gaze/1.0.0`).
    pub pipeline_version: String,
    /// `TrackingQualityParams.schemaVersion` the session ran with.
    pub params_version: String,
    pub device: SessionTrackingDevice,
    pub calibration: SessionCalibration,
    #[serde(default)]
    pub validation: Option<SessionValidation>,
    pub drift_checks: Vec<SessionDriftCheck>,
    #[serde(default)]
    pub max_drift_deg: Option<f64>,
    pub recalibrations: Vec<SessionRecalibration>,
    pub conditions: SessionTrackingConditions,
    pub loss: SessionTrackingLoss,
    pub confidence: SessionTrackingConfidence,
    /// QS-2, degrees; `null` when not measured.
    #[serde(default)]
    pub estimated_accuracy_deg: Option<f64>,
    /// QS-4.
    #[schemars(with = "mirror::TrackingVerdict")]
    pub verdict: TrackingVerdict,
}

/// `tracking_quality.device`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SessionTrackingDevice {
    /// Hardware model identifier, e.g. `iPad13,18`.
    pub model_identifier: String,
    #[schemars(with = "mirror::GeometrySource")]
    pub geometry_source: GeometrySource,
    #[schemars(with = "mirror::Orientation")]
    pub orientation: Orientation,
    pub screen_width_mm: f64,
    pub screen_height_mm: f64,
    #[serde(default)]
    pub sample_rate_hz: Option<f64>,
}

/// Head pose at calibration.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SessionCalibrationPose {
    pub distance_cm: f64,
    pub yaw_deg: f64,
    pub pitch_deg: f64,
}

/// `tracking_quality.calibration`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SessionCalibration {
    #[serde(default)]
    #[schemars(with = "Option<mirror::CalModelKind>")]
    pub model: Option<CalModelKind>,
    pub layout_points: u32,
    pub usable_points: u32,
    pub missing_points: u32,
    #[serde(default)]
    pub residual_deg: Option<f64>,
    #[serde(default)]
    pub loo_cv_deg: Option<f64>,
    #[serde(default)]
    pub condition_number: Option<f64>,
    #[schemars(with = "mirror::CalibrationQuality")]
    pub quality: CalibrationQuality,
    #[serde(default)]
    pub pose: Option<SessionCalibrationPose>,
    #[serde(default)]
    pub age_sec_at_start: Option<f64>,
    pub attempts: u32,
}

/// One validation target.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SessionValidationTarget {
    pub x_norm: f64,
    pub y_norm: f64,
    #[serde(default)]
    pub accuracy_deg: Option<f64>,
    #[serde(default)]
    pub precision_rms_deg: Option<f64>,
    #[serde(default)]
    pub sd_deg: Option<f64>,
    #[schemars(range(min = 0.0, max = 1.0))]
    pub data_loss: f64,
    pub valid_samples: u32,
    pub missed: bool,
}

/// `tracking_quality.validation`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SessionValidation {
    pub complete: bool,
    pub targets: Vec<SessionValidationTarget>,
    #[serde(default)]
    pub accuracy_deg: Option<f64>,
    #[serde(default)]
    pub precision_rms_deg: Option<f64>,
    #[serde(default)]
    pub precision_rms_raw_deg: Option<f64>,
    #[schemars(range(min = 0.0, max = 1.0))]
    pub data_loss: f64,
}

/// One drift check.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SessionDriftCheck {
    pub at_sec: f64,
    pub x_norm: f64,
    pub y_norm: f64,
    #[serde(default)]
    pub offset_deg: Option<f64>,
    #[serde(default)]
    pub drift_deg: Option<f64>,
    pub missed: bool,
}

/// One re-calibration that happened.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SessionRecalibration {
    pub at_sec: f64,
    #[schemars(with = "mirror::RecalReason")]
    pub reason: RecalReason,
    #[schemars(with = "mirror::CalibrationQuality")]
    pub new_quality: CalibrationQuality,
}

/// `tracking_quality.conditions`; each value `null` when not measured.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SessionTrackingConditions {
    #[serde(default)]
    pub distance_cm_median: Option<f64>,
    #[serde(default)]
    pub distance_cm_iqr: Option<f64>,
    #[serde(default)]
    pub head_speed_deg_per_sec_p90: Option<f64>,
    #[serde(default)]
    pub ambient_intensity_median: Option<f64>,
    #[serde(default)]
    pub light_asymmetry: Option<f64>,
    /// Operator answer at the positioning step; `null` when not asked.
    #[serde(default)]
    pub glasses: Option<bool>,
}

/// `tracking_quality.loss`, seconds.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SessionTrackingLoss {
    pub track_loss_sec: f64,
    pub blink_sec: f64,
    pub offscreen_sec: f64,
    pub artifact_sec: f64,
    pub gap_filled_sec: f64,
    pub longest_gap_sec: f64,
}

/// `tracking_quality.confidence`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SessionTrackingConfidence {
    #[schemars(range(min = 0.0, max = 1.0))]
    pub mean_sample: f64,
    #[serde(default)]
    #[schemars(range(min = 0.0, max = 1.0))]
    pub mean_fixation: Option<f64>,
    #[schemars(range(min = 0.0, max = 1.0))]
    pub low_confidence_sample_ratio: f64,
}

impl From<&TrackingQuality> for SessionTrackingQuality {
    fn from(tq: &TrackingQuality) -> Self {
        let TrackingQuality {
            pipeline_version,
            params_version,
            device,
            calibration,
            validation,
            drift_checks,
            max_drift_deg,
            recalibrations,
            conditions,
            loss,
            confidence,
            estimated_accuracy_deg,
            verdict,
        } = tq;
        Self {
            pipeline_version: pipeline_version.clone(),
            params_version: params_version.clone(),
            device: device.into(),
            calibration: calibration.into(),
            validation: validation.as_ref().map(Into::into),
            drift_checks: drift_checks.iter().map(Into::into).collect(),
            max_drift_deg: *max_drift_deg,
            recalibrations: recalibrations.iter().map(Into::into).collect(),
            conditions: conditions.into(),
            loss: loss.into(),
            confidence: confidence.into(),
            estimated_accuracy_deg: *estimated_accuracy_deg,
            verdict: *verdict,
        }
    }
}

impl From<&TrackingDevice> for SessionTrackingDevice {
    fn from(d: &TrackingDevice) -> Self {
        let TrackingDevice {
            model_identifier,
            geometry_source,
            orientation,
            screen_width_mm,
            screen_height_mm,
            sample_rate_hz,
        } = d;
        Self {
            model_identifier: model_identifier.clone(),
            geometry_source: *geometry_source,
            orientation: *orientation,
            screen_width_mm: *screen_width_mm,
            screen_height_mm: *screen_height_mm,
            sample_rate_hz: *sample_rate_hz,
        }
    }
}

impl From<&CalibrationPose> for SessionCalibrationPose {
    fn from(p: &CalibrationPose) -> Self {
        let CalibrationPose {
            distance_cm,
            yaw_deg,
            pitch_deg,
        } = p;
        Self {
            distance_cm: *distance_cm,
            yaw_deg: *yaw_deg,
            pitch_deg: *pitch_deg,
        }
    }
}

impl From<&CalibrationRecord> for SessionCalibration {
    fn from(c: &CalibrationRecord) -> Self {
        let CalibrationRecord {
            model,
            layout_points,
            usable_points,
            missing_points,
            residual_deg,
            loo_cv_deg,
            condition_number,
            quality,
            pose,
            age_sec_at_start,
            attempts,
        } = c;
        Self {
            model: *model,
            layout_points: *layout_points,
            usable_points: *usable_points,
            missing_points: *missing_points,
            residual_deg: *residual_deg,
            loo_cv_deg: *loo_cv_deg,
            condition_number: *condition_number,
            quality: *quality,
            pose: pose.as_ref().map(Into::into),
            age_sec_at_start: *age_sec_at_start,
            attempts: *attempts,
        }
    }
}

impl From<&ValidationTargetRecord> for SessionValidationTarget {
    fn from(t: &ValidationTargetRecord) -> Self {
        let ValidationTargetRecord {
            x_norm,
            y_norm,
            accuracy_deg,
            precision_rms_deg,
            sd_deg,
            data_loss,
            valid_samples,
            missed,
        } = t;
        Self {
            x_norm: *x_norm,
            y_norm: *y_norm,
            accuracy_deg: *accuracy_deg,
            precision_rms_deg: *precision_rms_deg,
            sd_deg: *sd_deg,
            data_loss: *data_loss,
            valid_samples: *valid_samples,
            missed: *missed,
        }
    }
}

impl From<&ValidationRecord> for SessionValidation {
    fn from(v: &ValidationRecord) -> Self {
        let ValidationRecord {
            complete,
            targets,
            accuracy_deg,
            precision_rms_deg,
            precision_rms_raw_deg,
            data_loss,
        } = v;
        Self {
            complete: *complete,
            targets: targets.iter().map(Into::into).collect(),
            accuracy_deg: *accuracy_deg,
            precision_rms_deg: *precision_rms_deg,
            precision_rms_raw_deg: *precision_rms_raw_deg,
            data_loss: *data_loss,
        }
    }
}

impl From<&DriftCheckRecord> for SessionDriftCheck {
    fn from(d: &DriftCheckRecord) -> Self {
        let DriftCheckRecord {
            at_sec,
            x_norm,
            y_norm,
            offset_deg,
            drift_deg,
            missed,
        } = d;
        Self {
            at_sec: *at_sec,
            x_norm: *x_norm,
            y_norm: *y_norm,
            offset_deg: *offset_deg,
            drift_deg: *drift_deg,
            missed: *missed,
        }
    }
}

impl From<&RecalibrationRecord> for SessionRecalibration {
    fn from(r: &RecalibrationRecord) -> Self {
        let RecalibrationRecord {
            at_sec,
            reason,
            new_quality,
        } = r;
        Self {
            at_sec: *at_sec,
            reason: *reason,
            new_quality: *new_quality,
        }
    }
}

impl From<&TrackingConditions> for SessionTrackingConditions {
    fn from(c: &TrackingConditions) -> Self {
        let TrackingConditions {
            distance_cm_median,
            distance_cm_iqr,
            head_speed_deg_per_sec_p90,
            ambient_intensity_median,
            light_asymmetry,
            glasses,
        } = c;
        Self {
            distance_cm_median: *distance_cm_median,
            distance_cm_iqr: *distance_cm_iqr,
            head_speed_deg_per_sec_p90: *head_speed_deg_per_sec_p90,
            ambient_intensity_median: *ambient_intensity_median,
            light_asymmetry: *light_asymmetry,
            glasses: *glasses,
        }
    }
}

impl From<&LossSummary> for SessionTrackingLoss {
    fn from(l: &LossSummary) -> Self {
        let LossSummary {
            track_loss_sec,
            blink_sec,
            offscreen_sec,
            artifact_sec,
            gap_filled_sec,
            longest_gap_sec,
        } = l;
        Self {
            track_loss_sec: *track_loss_sec,
            blink_sec: *blink_sec,
            offscreen_sec: *offscreen_sec,
            artifact_sec: *artifact_sec,
            gap_filled_sec: *gap_filled_sec,
            longest_gap_sec: *longest_gap_sec,
        }
    }
}

impl From<&ConfidenceSummary> for SessionTrackingConfidence {
    fn from(c: &ConfidenceSummary) -> Self {
        let ConfidenceSummary {
            mean_sample,
            mean_fixation,
            low_confidence_sample_ratio,
        } = c;
        Self {
            mean_sample: *mean_sample,
            mean_fixation: *mean_fixation,
            low_confidence_sample_ratio: *low_confidence_sample_ratio,
        }
    }
}

/// An example record for `api-v1.json` (the 1.7 golden's values).
pub(crate) fn example_tracking_quality() -> SessionTrackingQuality {
    SessionTrackingQuality {
        pipeline_version: "asd-gaze/1.0.0".into(),
        params_version: "1.0.0".into(),
        device: SessionTrackingDevice {
            model_identifier: "iPad13,18".into(),
            geometry_source: GeometrySource::Table,
            orientation: Orientation::LandscapeLeft,
            screen_width_mm: 238.25,
            screen_height_mm: 165.64,
            sample_rate_hz: Some(60.0),
        },
        calibration: SessionCalibration {
            model: Some(CalModelKind::ScreenAffine),
            layout_points: 5,
            usable_points: 5,
            missing_points: 0,
            residual_deg: Some(1.12),
            loo_cv_deg: Some(1.64),
            condition_number: Some(42.5),
            quality: CalibrationQuality::Good,
            pose: Some(SessionCalibrationPose {
                distance_cm: 38.0,
                yaw_deg: 3.5,
                pitch_deg: -4.25,
            }),
            age_sec_at_start: Some(6.5),
            attempts: 1,
        },
        validation: Some(SessionValidation {
            complete: true,
            targets: vec![SessionValidationTarget {
                x_norm: 0.3,
                y_norm: 0.3,
                accuracy_deg: Some(1.4),
                precision_rms_deg: Some(0.21),
                sd_deg: Some(0.35),
                data_loss: 0.02,
                valid_samples: 58,
                missed: false,
            }],
            accuracy_deg: Some(1.63),
            precision_rms_deg: Some(0.22),
            precision_rms_raw_deg: Some(0.48),
            data_loss: 0.24,
        }),
        drift_checks: vec![SessionDriftCheck {
            at_sec: 11.2,
            x_norm: 0.5,
            y_norm: 0.5,
            offset_deg: Some(1.1),
            drift_deg: Some(0.4),
            missed: false,
        }],
        max_drift_deg: Some(0.4),
        recalibrations: vec![SessionRecalibration {
            at_sec: 23.5,
            reason: RecalReason::Drift,
            new_quality: CalibrationQuality::Fair,
        }],
        conditions: SessionTrackingConditions {
            distance_cm_median: Some(39.5),
            distance_cm_iqr: Some(4.25),
            head_speed_deg_per_sec_p90: Some(18.5),
            ambient_intensity_median: Some(870.0),
            light_asymmetry: Some(0.12),
            glasses: Some(false),
        },
        loss: SessionTrackingLoss {
            track_loss_sec: 0.85,
            blink_sec: 0.6,
            offscreen_sec: 0.4,
            artifact_sec: 0.05,
            gap_filled_sec: 0.3,
            longest_gap_sec: 0.45,
        },
        confidence: SessionTrackingConfidence {
            mean_sample: 0.82,
            mean_fixation: Some(0.86),
            low_confidence_sample_ratio: 0.07,
        },
        estimated_accuracy_deg: Some(1.63),
        verdict: TrackingVerdict::Good,
    }
}
