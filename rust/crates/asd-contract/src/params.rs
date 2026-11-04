//! owner: WP-X.0
//!
//! The parameter registry (PRM-1..4, ADR-0012 D1, ADR-0013 principle 1): one self-describing
//! row per tunable leaf of the [`ScoringProfile`] (id, kind and range, unit, default, stage,
//! what a change needs to take effect, message keys). The console, iOS and Android render
//! tuning forms from it; [`validate`] is the one range check every publisher uses (backend
//! `config_docs::lifecycle::validate_body`, wasm `validateScoringProfile`) and
//! [`validate_for_device`] its tolerant twin for `asd_sync::accept_bundle` (DSP-6: unknown
//! members ignored, known ones checked); `asd-cli schema export` writes it to
//! `docs/spec/schemas/asd/param-registry-v1.json`.
//!
//! Rules:
//! - An id is the camelCase dotted wire path of the leaf (`metrics.ivtVelocityUnitsPerSec`).
//!   An array is one leaf (`calibration.layout5`, `ageBands`).
//! - Completeness (PRM-2): the leaves of `ScoringProfile::default()` serialised are exactly
//!   the registry ids plus [`NON_PARAMETER_LEAVES`] (document versions, not knobs). Adding a
//!   field without a row, or a row without a field, fails `cargo test -p asd-contract`.
//! - Each row's `default` is read from `ScoringProfile::default()`, so the registry and the
//!   type cannot disagree about a default.
//! - Labels and help texts are message keys (`param.<id>.label`, `param.<id>.help`, PRM-5);
//!   the texts live in the surfaces' catalogues, never here.
//! - [`REGISTRY_VERSION`] changes when a row changes meaning (range, kind, stage), not when a
//!   default changes.

use std::collections::BTreeSet;
use std::sync::OnceLock;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::config::ScoringProfile;
use crate::profile_checks::{scoring_profile_problems, ProfileProblem};

/// Version of the registry's meaning (PRM-1).
pub const REGISTRY_VERSION: &str = "1.0.0";
/// File name of the exported registry (PRM-4).
pub const PARAM_REGISTRY_FILE: &str = "param-registry-v1.json";
/// `$id` of the exported registry.
pub const PARAM_REGISTRY_ID: &str = "https://schemas.curv.org/asd/param-registry-v1.json";
/// Leaves of the profile that are document metadata, not parameters: never in the registry
/// and never refused as unknown.
pub const NON_PARAMETER_LEAVES: &[&str] = &["schemaVersion", "trackingQuality.schemaVersion"];

/// The config document a parameter lives in. Only the scoring profile carries knobs today.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DocumentKind {
    ScoringProfile,
}

/// Display unit of a numeric parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Unit {
    None,
    Ratio,
    Percent,
    Seconds,
    Millis,
    Cm,
    Mm,
    Deg,
    DegPerSec,
    Rad,
    Hz,
    NormUnits,
    NormUnitsPerSec,
    Months,
    Count,
}

/// Value type and allowed range (PRM-1). `AgeBands` is the one structured kind: the profile's
/// `ageBands` list (contiguous, ascending, inclusive month bounds).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum ParamKind {
    Number {
        min: f64,
        max: f64,
        step: f64,
        unit: Unit,
    },
    Integer {
        min: i64,
        max: i64,
        unit: Unit,
    },
    Bool,
    Enum {
        values: Vec<String>,
    },
    /// Screen-normalised `[x, y]` points, each coordinate in `[0, 1]`.
    PointList {
        min_points: u32,
        max_points: u32,
    },
    /// `[{id, minMonths, maxMonths, representativeMonths?}]`: ids unique, bands contiguous and
    /// ascending, `minMonths <= representativeMonths <= maxMonths` (DSP-2, 1.3.0).
    AgeBands {
        min_bands: u32,
        max_bands: u32,
        min_months: i64,
        max_months: i64,
    },
}

/// Pipeline stage a parameter belongs to (groups and ordering in the UI).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    Capture,
    Preprocess,
    Fixation,
    Metrics,
    Scoring,
    Flag,
    Norms,
    Display,
}

/// What a changed value needs before it shows in a result (RPX-3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Reprocess {
    /// Re-scoring the stored per-clip rows is enough.
    Score,
    /// Metrics must be re-extracted from frames.
    Metrics,
    /// The raw samples must be replayed (research frame log).
    Frames,
    /// Only a new capture can show it (device behaviour during the session).
    CaptureOnly,
}

/// One registry row (PRM-1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ParamSpec {
    /// camelCase dotted wire path in the document.
    pub id: String,
    pub document: DocumentKind,
    pub kind: ParamKind,
    /// The value in `ScoringProfile::default()`.
    pub default: Value,
    pub stage: Stage,
    pub reprocess: Reprocess,
    /// `param.<id>.label` (PRM-5).
    pub label_key: String,
    /// `param.<id>.help`.
    pub help_key: String,
    /// Profile schema version that introduced the parameter.
    pub since: String,
    /// Owner decision fencing the knob for clinical use (e.g. `H-G2`); analysis may still
    /// vary it (ADR-0012 open question 9).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_gate: Option<String>,
}

/// The exported registry document (`param-registry-v1.json`, PRM-4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ParamRegistry {
    pub registry_version: String,
    pub params: Vec<ParamSpec>,
}

// ---- the table -----------------------------------------------------------------------------

/// Kind of a row, as written in [`ROWS`].
#[derive(Clone, Copy)]
enum K {
    Num(f64, f64, f64, Unit),
    Int(i64, i64, Unit),
    Bool,
    Enum(&'static [&'static str]),
    Points(u32, u32),
    AgeBands,
}

type Row = (
    &'static str,
    K,
    Stage,
    Reprocess,
    &'static str,
    Option<&'static str>,
);

const V100: &str = "1.0.0";
const V110: &str = "1.1.0";
const V120: &str = "1.2.0";
const V130: &str = "1.3.0";
const HG2: Option<&str> = Some("H-G2");

const CAL_MODELS: &[&str] = &[
    "screenAffine",
    "angularAffine",
    "screenHomography",
    "screenPoly2",
];
const IVT_UNITS: &[&str] = &["norm", "deg"];

use Reprocess::{CaptureOnly, Frames, Score};
use Stage::{Capture, Display, Fixation, Flag, Metrics, Norms, Preprocess, Scoring};
use Unit::{
    Count, Deg, DegPerSec, Hz, Millis, NormUnits, NormUnitsPerSec, Percent, Rad, Ratio, Seconds,
};

/// Every parameter, in display order (grouped by the first path segment).
#[rustfmt::skip]
const ROWS: &[Row] = &[
    // ---- flag (traffic light, AttentionFlagAlgorithm)
    ("flag.greenFloor", K::Num(0.0, 1.0, 0.01, Ratio), Flag, Score, V100, None),
    ("flag.yellowFloor", K::Num(0.0, 1.0, 0.01, Ratio), Flag, Score, V100, None),
    ("flag.lowConfidenceShare", K::Num(0.0, 1.0, 0.01, Ratio), Flag, Score, V100, None),
    ("flag.atypicalShareForRed", K::Num(0.0, 1.0, 0.01, Ratio), Flag, Score, V100, None),
    // ---- scoring
    ("lowConfidenceValidRatio", K::Num(0.0, 1.0, 0.01, Ratio), Scoring, Score, V100, None),
    // ---- per-sample quality gate
    ("qualityGate.minDistanceCm", K::Num(10.0, 150.0, 1.0, Unit::Cm), Preprocess, Frames, V100, None),
    ("qualityGate.maxDistanceCm", K::Num(10.0, 150.0, 1.0, Unit::Cm), Preprocess, Frames, V100, None),
    ("qualityGate.maxYawRad", K::Num(0.05, 1.57, 0.01, Rad), Preprocess, Frames, V100, None),
    ("qualityGate.maxPitchRad", K::Num(0.05, 1.57, 0.01, Rad), Preprocess, Frames, V100, None),
    ("qualityGate.minConfidence", K::Num(0.0, 1.0, 0.01, Ratio), Preprocess, Frames, V100, None),
    ("qualityGate.blinkThreshold", K::Num(0.0, 1.0, 0.01, Ratio), Preprocess, Frames, V100, None),
    // ---- gaze robustness (SCH-3)
    ("trackingQuality.provisional", K::Bool, Display, Score, V110, None),
    ("trackingQuality.calibration.settleMs", K::Num(0.0, 5000.0, 10.0, Millis), Preprocess, Frames, V110, None),
    ("trackingQuality.calibration.collectMs", K::Num(100.0, 10000.0, 10.0, Millis), Preprocess, Frames, V110, None),
    ("trackingQuality.calibration.maxTargetWaitMs", K::Num(0.0, 30000.0, 100.0, Millis), Capture, CaptureOnly, V110, None),
    ("trackingQuality.calibration.minSamplesPerTarget", K::Int(1, 600, Count), Preprocess, Frames, V110, None),
    ("trackingQuality.calibration.maxRetriesPerTarget", K::Int(0, 5, Count), Capture, CaptureOnly, V110, None),
    ("trackingQuality.calibration.madRejectK", K::Num(0.5, 10.0, 0.1, Unit::None), Preprocess, Frames, V110, None),
    ("trackingQuality.calibration.defaultModel", K::Enum(CAL_MODELS), Preprocess, Frames, V110, None),
    ("trackingQuality.calibration.autoSelect", K::Bool, Preprocess, Frames, V110, None),
    ("trackingQuality.calibration.minImprovementDeg", K::Num(0.0, 5.0, 0.05, Deg), Preprocess, Frames, V110, None),
    ("trackingQuality.calibration.eligibilityMarginPoints", K::Int(0, 10, Count), Preprocess, Frames, V110, None),
    ("trackingQuality.calibration.bands.goodMaxDeg", K::Num(0.1, 20.0, 0.1, Deg), Preprocess, Frames, V110, None),
    ("trackingQuality.calibration.bands.fairMaxDeg", K::Num(0.1, 20.0, 0.1, Deg), Preprocess, Frames, V110, None),
    ("trackingQuality.calibration.bands.poorMaxDeg", K::Num(0.1, 20.0, 0.1, Deg), Preprocess, Frames, V110, None),
    ("trackingQuality.calibration.minUsablePoints", K::Int(1, 20, Count), Preprocess, Frames, V110, None),
    ("trackingQuality.calibration.maxConditionNumber", K::Num(1.0, 1e12, 1.0, Unit::None), Preprocess, Frames, V110, None),
    ("trackingQuality.calibration.layout5", K::Points(1, 25), Capture, CaptureOnly, V110, None),
    ("trackingQuality.calibration.layout9", K::Points(1, 25), Capture, CaptureOnly, V110, None),
    ("trackingQuality.recalibration.poseShiftDistancePct", K::Num(1.0, 100.0, 1.0, Percent), Capture, CaptureOnly, V110, None),
    ("trackingQuality.recalibration.poseShiftAngleDeg", K::Num(1.0, 90.0, 1.0, Deg), Capture, CaptureOnly, V110, None),
    ("trackingQuality.recalibration.poseWindowSec", K::Num(0.5, 30.0, 0.5, Seconds), Capture, CaptureOnly, V110, None),
    ("trackingQuality.recalibration.maxCalibrationAgeSec", K::Num(30.0, 7200.0, 10.0, Seconds), Capture, CaptureOnly, V110, None),
    ("trackingQuality.recalibration.maxRecalibrationsPerSession", K::Int(0, 10, Count), Capture, CaptureOnly, V110, None),
    ("trackingQuality.recalibration.maxRecalibrationsInBattery", K::Int(0, 10, Count), Capture, CaptureOnly, V110, None),
    ("trackingQuality.validation.layout", K::Points(1, 25), Capture, CaptureOnly, V110, None),
    ("trackingQuality.validation.settleMs", K::Num(0.0, 5000.0, 10.0, Millis), Preprocess, Frames, V110, None),
    ("trackingQuality.validation.collectMs", K::Num(100.0, 10000.0, 10.0, Millis), Preprocess, Frames, V110, None),
    ("trackingQuality.validation.minSamplesPerTarget", K::Int(1, 600, Count), Preprocess, Frames, V110, None),
    ("trackingQuality.validation.minTargetsHit", K::Int(1, 25, Count), Preprocess, Frames, V110, None),
    ("trackingQuality.validation.warnAccuracyDeg", K::Num(0.1, 20.0, 0.1, Deg), Preprocess, Frames, V110, None),
    ("trackingQuality.validation.maxAccuracyDeg", K::Num(0.1, 20.0, 0.1, Deg), Preprocess, Frames, V110, None),
    ("trackingQuality.validation.benchLayout13", K::Points(1, 25), Capture, CaptureOnly, V110, None),
    ("trackingQuality.drift.checkLayout", K::Points(1, 25), Capture, CaptureOnly, V110, None),
    ("trackingQuality.drift.settleMs", K::Num(0.0, 5000.0, 10.0, Millis), Preprocess, Frames, V110, None),
    ("trackingQuality.drift.collectMs", K::Num(100.0, 10000.0, 10.0, Millis), Preprocess, Frames, V110, None),
    ("trackingQuality.drift.minSamples", K::Int(1, 600, Count), Preprocess, Frames, V110, None),
    ("trackingQuality.drift.warnDeg", K::Num(0.1, 20.0, 0.1, Deg), Preprocess, Frames, V110, None),
    ("trackingQuality.drift.warnConsecutive", K::Int(1, 10, Count), Capture, CaptureOnly, V110, None),
    ("trackingQuality.drift.hardDeg", K::Num(0.1, 20.0, 0.1, Deg), Preprocess, Frames, V110, None),
    ("trackingQuality.trackState.blinkRecoveryMs", K::Num(0.0, 1000.0, 10.0, Millis), Preprocess, Frames, V110, None),
    ("trackingQuality.trackState.maxEyeSpeedDegPerSec", K::Num(10.0, 5000.0, 10.0, DegPerSec), Preprocess, Frames, V110, None),
    ("trackingQuality.trackState.offscreenMarginNorm", K::Num(0.0, 0.5, 0.005, NormUnits), Preprocess, Frames, V110, None),
    ("trackingQuality.confidence.eyeOpennessComfort", K::Num(0.0, 1.0, 0.01, Ratio), Preprocess, Frames, V110, None),
    ("trackingQuality.confidence.poseComfortFraction", K::Num(0.0, 1.0, 0.01, Ratio), Preprocess, Frames, V110, None),
    ("trackingQuality.confidence.distanceComfortMinCm", K::Num(10.0, 150.0, 1.0, Unit::Cm), Preprocess, Frames, V110, None),
    ("trackingQuality.confidence.distanceComfortMaxCm", K::Num(10.0, 150.0, 1.0, Unit::Cm), Preprocess, Frames, V110, None),
    ("trackingQuality.confidence.headSpeedComfortDegPerSec", K::Num(1.0, 1000.0, 1.0, DegPerSec), Preprocess, Frames, V110, None),
    ("trackingQuality.confidence.maxHeadSpeedDegPerSec", K::Num(1.0, 1000.0, 1.0, DegPerSec), Preprocess, Frames, V110, None),
    ("trackingQuality.confidence.poseShiftComfortPct", K::Num(0.0, 100.0, 1.0, Percent), Preprocess, Frames, V110, None),
    ("trackingQuality.gapFill.maxMs", K::Num(0.0, 1000.0, 5.0, Millis), Preprocess, Frames, V110, None),
    ("trackingQuality.gapFill.maxDeg", K::Num(0.0, 20.0, 0.1, Deg), Preprocess, Frames, V110, None),
    ("trackingQuality.gapFill.lossIntervalCapMs", K::Num(1.0, 1000.0, 5.0, Millis), Preprocess, Frames, V110, None),
    ("trackingQuality.filter.minCutoffHz", K::Num(0.01, 30.0, 0.01, Hz), Preprocess, Frames, V110, None),
    ("trackingQuality.filter.beta", K::Num(0.0, 1.0, 0.001, Unit::None), Preprocess, Frames, V110, None),
    ("trackingQuality.filter.dCutoffHz", K::Num(0.01, 30.0, 0.01, Hz), Preprocess, Frames, V110, None),
    ("trackingQuality.verdict.accuracyBands.goodMaxDeg", K::Num(0.1, 20.0, 0.1, Deg), Metrics, Frames, V110, None),
    ("trackingQuality.verdict.accuracyBands.fairMaxDeg", K::Num(0.1, 20.0, 0.1, Deg), Metrics, Frames, V110, None),
    ("trackingQuality.verdict.accuracyBands.poorMaxDeg", K::Num(0.1, 20.0, 0.1, Deg), Metrics, Frames, V110, None),
    ("trackingQuality.verdict.unusableBelowValidRatio", K::Num(0.0, 1.0, 0.01, Ratio), Metrics, Frames, V110, None),
    ("trackingQuality.verdict.poorAboveDataLoss", K::Num(0.0, 1.0, 0.01, Ratio), Metrics, Frames, V110, None),
    ("trackingQuality.verdict.fairAboveDataLoss", K::Num(0.0, 1.0, 0.01, Ratio), Metrics, Frames, V110, None),
    ("trackingQuality.scoring.maxAccuracyDegForScoring", K::Num(0.1, 20.0, 0.1, Deg), Scoring, Score, V110, HG2),
    ("trackingQuality.scoring.aoiResolutionFactor", K::Num(0.1, 10.0, 0.1, Unit::None), Scoring, Frames, V110, HG2),
    ("trackingQuality.scoring.ivtVelocityDegPerSec", K::Num(1.0, 1000.0, 1.0, DegPerSec), Fixation, Frames, V110, HG2),
    // ---- 1.2.0: metrics (INV-1 asd-core rows)
    ("metrics.ivtVelocityUnitsPerSec", K::Num(0.5, 50.0, 0.1, NormUnitsPerSec), Fixation, Frames, V120, None),
    ("metrics.ivtVelocityDegPerSec", K::Num(1.0, 1000.0, 1.0, DegPerSec), Fixation, Frames, V120, HG2),
    ("metrics.ivtUnit", K::Enum(IVT_UNITS), Fixation, Frames, V120, HG2),
    ("metrics.minFixationDurationSec", K::Num(0.02, 1.0, 0.01, Seconds), Fixation, Frames, V120, None),
    ("metrics.fallbackFrameIntervalSec", K::Num(0.001, 0.1, 0.001, Seconds), Metrics, Frames, V120, None),
    ("metrics.smileReactivityWindowSec", K::Num(0.1, 5.0, 0.1, Seconds), Metrics, Frames, V120, None),
    ("metrics.flatAffectGain", K::Num(1.0, 500.0, 1.0, Unit::None), Metrics, Frames, V120, None),
    // ---- 1.2.0: AOI hit testing
    ("aoi.marginNorm", K::Num(-0.1, 0.1, 0.005, NormUnits), Metrics, Frames, V120, None),
    ("aoi.dwellCapMs", K::Int(16, 1000, Millis), Metrics, Frames, V120, None),
    // ---- 1.2.0: age bands and norms (AGE-2)
    ("ageBands", K::AgeBands, Norms, Score, V120, None),
    ("norms.minN", K::Int(5, 100_000, Count), Norms, Score, V120, None),
    ("norms.includeImputed", K::Bool, Norms, Score, V120, None),
    // ---- 1.2.0: backend review rule (INV-1 backend rows)
    ("review.lowQualityValidRatio", K::Num(0.0, 1.0, 0.01, Ratio), Display, Score, V120, None),
    // ---- 1.3.0: result-screen bands (DSP-1, technical §5.1 #4-#17, #22)
    ("display.bands.socialPreferencePct.okMin", K::Num(0.0, 100.0, 1.0, Percent), Display, Score, V130, None),
    ("display.bands.socialPreferencePct.monitorMin", K::Num(0.0, 100.0, 1.0, Percent), Display, Score, V130, None),
    ("display.bands.eyeDwellPct.okMin", K::Num(0.0, 100.0, 1.0, Percent), Display, Score, V130, None),
    ("display.bands.eyeDwellPct.monitorMin", K::Num(0.0, 100.0, 1.0, Percent), Display, Score, V130, None),
    ("display.bands.mouthDwellPct.okMin", K::Num(0.0, 100.0, 1.0, Percent), Display, Score, V130, None),
    ("display.bands.mouthDwellPct.okMax", K::Num(0.0, 100.0, 1.0, Percent), Display, Score, V130, None),
    ("display.bands.firstEyeLatencyMs.okMax", K::Num(50.0, 5000.0, 10.0, Millis), Display, Score, V130, None),
    ("display.bands.firstEyeLatencyMs.monitorMax", K::Num(50.0, 5000.0, 10.0, Millis), Display, Score, V130, None),
    ("display.bands.smileIntensityPct.okMin", K::Num(0.0, 100.0, 1.0, Percent), Display, Score, V130, None),
    ("display.bands.smileIntensityPct.monitorMin", K::Num(0.0, 100.0, 1.0, Percent), Display, Score, V130, None),
    ("display.bands.smileReactivity.okMin", K::Num(0.0, 1.0, 0.01, Ratio), Display, Score, V130, None),
    ("display.bands.surprisePct.okMin", K::Num(0.0, 100.0, 1.0, Percent), Display, Score, V130, None),
    ("display.bands.flatAffect.okMax", K::Num(0.0, 1.0, 0.01, Ratio), Display, Score, V130, None),
    ("display.scanpathMaxGapMs", K::Int(16, 1000, Millis), Display, Score, V130, None),
    // ---- 1.3.0: session verdict (DSP-1, DSP-4, technical §5.1 #18-#20)
    ("sessionQuality.optimalMinValidRatio", K::Num(0.0, 1.0, 0.01, Ratio), Metrics, Score, V130, None),
    ("sessionQuality.acceptableMinValidRatio", K::Num(0.0, 1.0, 0.01, Ratio), Metrics, Score, V130, None),
];

/// `ageBands` limits (PRM-7).
const AGE_BANDS_MIN: u32 = 1;
const AGE_BANDS_MAX: u32 = 12;
const AGE_MONTHS_MIN: i64 = 0;
const AGE_MONTHS_MAX: i64 = 240;

fn kind_of(k: K) -> ParamKind {
    match k {
        K::Num(min, max, step, unit) => ParamKind::Number {
            min,
            max,
            step,
            unit,
        },
        K::Int(min, max, unit) => ParamKind::Integer { min, max, unit },
        K::Bool => ParamKind::Bool,
        K::Enum(values) => ParamKind::Enum {
            values: values.iter().map(|v| (*v).to_owned()).collect(),
        },
        K::Points(min_points, max_points) => ParamKind::PointList {
            min_points,
            max_points,
        },
        K::AgeBands => ParamKind::AgeBands {
            min_bands: AGE_BANDS_MIN,
            max_bands: AGE_BANDS_MAX,
            min_months: AGE_MONTHS_MIN,
            max_months: AGE_MONTHS_MAX,
        },
    }
}

/// The value at a dotted path, or `None`.
pub fn get_path<'a>(body: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.').try_fold(body, |v, key| v.get(key))
}

/// Every registered parameter, in display order (PRM-1).
pub fn registry() -> &'static [ParamSpec] {
    static REGISTRY: OnceLock<Vec<ParamSpec>> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        let defaults = serde_json::to_value(ScoringProfile::default())
            .expect("the default profile serialises");
        ROWS.iter()
            .map(|&(id, k, stage, reprocess, since, owner_gate)| ParamSpec {
                id: id.to_owned(),
                document: DocumentKind::ScoringProfile,
                kind: kind_of(k),
                // A row without a field is caught by the completeness test; `null` keeps the
                // registry printable meanwhile.
                default: get_path(&defaults, id).cloned().unwrap_or(Value::Null),
                stage,
                reprocess,
                label_key: format!("param.{id}.label"),
                help_key: format!("param.{id}.help"),
                since: since.to_owned(),
                owner_gate: owner_gate.map(str::to_owned),
            })
            .collect()
    })
}

/// [`REGISTRY_VERSION`] (PRM-1).
pub fn registry_version() -> &'static str {
    REGISTRY_VERSION
}

/// One row by id.
pub fn spec(id: &str) -> Option<&'static ParamSpec> {
    registry().iter().find(|s| s.id == id)
}

/// The exported document (PRM-4).
pub fn registry_document() -> ParamRegistry {
    ParamRegistry {
        registry_version: REGISTRY_VERSION.to_owned(),
        params: registry().to_vec(),
    }
}

/// `param-registry-v1.json`: the registry document with its `$id`.
pub fn registry_json() -> Value {
    let mut v = serde_json::to_value(registry_document()).expect("the registry serialises");
    if let Value::Object(map) = &mut v {
        map.insert("$id".into(), json!(PARAM_REGISTRY_ID));
    }
    v
}

/// Every leaf path of a JSON document (arrays are one leaf), in key order.
pub fn leaf_paths(body: &Value) -> Vec<String> {
    fn walk(v: &Value, prefix: &str, out: &mut Vec<String>) {
        match v {
            Value::Object(map) if !map.is_empty() => {
                for (k, child) in map {
                    let path = if prefix.is_empty() {
                        k.clone()
                    } else {
                        format!("{prefix}.{k}")
                    };
                    walk(child, &path, out);
                }
            }
            _ => out.push(prefix.to_owned()),
        }
    }
    let mut out = Vec::new();
    walk(body, "", &mut out);
    out
}

// ---- validation (PRM-3) ------------------------------------------------------------------

fn problem(path: &str, message: String) -> ProfileProblem {
    ProfileProblem {
        path: path.to_owned(),
        message,
    }
}

fn check_value(spec: &ParamSpec, v: &Value, out: &mut Vec<ProfileProblem>) {
    let id = spec.id.as_str();
    match &spec.kind {
        ParamKind::Number { min, max, .. } => match v.as_f64() {
            Some(x) if x.is_finite() && *min <= x && x <= *max => {}
            Some(_) => out.push(problem(id, format!("{id} must be in [{min}, {max}]"))),
            None => out.push(problem(id, format!("{id} must be a number"))),
        },
        ParamKind::Integer { min, max, .. } => {
            let n = v.as_i64().or_else(|| v.as_u64().map(|_| i64::MAX));
            match n {
                Some(x) if *min <= x && x <= *max => {}
                Some(_) => out.push(problem(id, format!("{id} must be in [{min}, {max}]"))),
                None => out.push(problem(id, format!("{id} must be an integer"))),
            }
        }
        ParamKind::Bool => {
            if !v.is_boolean() {
                out.push(problem(id, format!("{id} must be true or false")));
            }
        }
        ParamKind::Enum { values } => {
            if !v.as_str().is_some_and(|s| values.iter().any(|x| x == s)) {
                out.push(problem(
                    id,
                    format!("{id} must be one of: {}", values.join(", ")),
                ));
            }
        }
        ParamKind::PointList {
            min_points,
            max_points,
        } => {
            let Some(points) = v.as_array() else {
                out.push(problem(id, format!("{id} must be a list of [x, y] points")));
                return;
            };
            let n = points.len();
            if n < *min_points as usize || n > *max_points as usize {
                out.push(problem(
                    id,
                    format!("{id} must have {min_points} to {max_points} points"),
                ));
            } else if !points.iter().all(|p| {
                p.as_array().is_some_and(|xy| {
                    xy.len() == 2
                        && xy
                            .iter()
                            .all(|c| c.as_f64().is_some_and(|c| (0.0..=1.0).contains(&c)))
                })
            }) {
                out.push(problem(
                    id,
                    format!("{id} points must be [x, y] pairs in [0, 1]"),
                ));
            }
        }
        ParamKind::AgeBands {
            min_bands,
            max_bands,
            min_months,
            max_months,
        } => check_age_bands(id, v, *min_bands, *max_bands, *min_months, *max_months, out),
    }
}

fn check_age_bands(
    id: &str,
    v: &Value,
    min_bands: u32,
    max_bands: u32,
    min_months: i64,
    max_months: i64,
    out: &mut Vec<ProfileProblem>,
) {
    let Some(bands) = v.as_array() else {
        out.push(problem(id, format!("{id} must be a list of age bands")));
        return;
    };
    if bands.len() < min_bands as usize || bands.len() > max_bands as usize {
        out.push(problem(
            id,
            format!("{id} must have {min_bands} to {max_bands} bands"),
        ));
        return;
    }
    let mut ids = BTreeSet::new();
    let mut previous_max: Option<i64> = None;
    for (i, band) in bands.iter().enumerate() {
        let path = format!("{id}.{i}");
        let band_id = band.get("id").and_then(Value::as_str).unwrap_or("");
        let lo = band.get("minMonths").and_then(Value::as_i64);
        let hi = band.get("maxMonths").and_then(Value::as_i64);
        let (Some(lo), Some(hi)) = (lo, hi) else {
            out.push(problem(
                &path,
                format!("{path} needs an id and integer minMonths and maxMonths"),
            ));
            return;
        };
        if band_id.trim().is_empty() || !ids.insert(band_id.to_owned()) {
            out.push(problem(
                &path,
                format!("{path}.id must be unique and non-empty"),
            ));
            return;
        }
        if lo < min_months || hi > max_months || lo > hi {
            out.push(problem(
                &path,
                format!("{path} needs {min_months} <= minMonths <= maxMonths <= {max_months}"),
            ));
            return;
        }
        match band.get("representativeMonths") {
            None | Some(Value::Null) => {}
            Some(r) => {
                if !r.as_i64().is_some_and(|r| lo <= r && r <= hi) {
                    out.push(problem(
                        &path,
                        format!("{path}.representativeMonths must be an integer in [{lo}, {hi}]"),
                    ));
                    return;
                }
            }
        }
        if let Some(prev) = previous_max {
            if lo != prev + 1 {
                out.push(problem(
                    &path,
                    format!("{id} must be contiguous and ascending (band {i} starts at {lo}, expected {})", prev + 1),
                ));
                return;
            }
        }
        previous_max = Some(hi);
    }
}

/// Unknown keys under registered objects (`<path>: unknown parameter`).
fn unknown_keys(v: &Value, prefix: &str, out: &mut Vec<ProfileProblem>) {
    let Value::Object(map) = v else { return };
    for (k, child) in map {
        let path = if prefix.is_empty() {
            k.clone()
        } else {
            format!("{prefix}.{k}")
        };
        let is_leaf = NON_PARAMETER_LEAVES.contains(&path.as_str())
            || registry().iter().any(|s| s.id == path);
        if is_leaf {
            continue;
        }
        let dotted = format!("{path}.");
        if registry().iter().any(|s| s.id.starts_with(&dotted)) {
            unknown_keys(child, &path, out);
        } else {
            out.push(problem(&path, format!("{path}: unknown parameter")));
        }
    }
}

/// The registry checks alone: kind, range and unknown keys, in registry order.
pub fn registry_problems(document: DocumentKind, body: &Value) -> Vec<ProfileProblem> {
    registry_problems_with(document, body, true)
}

fn registry_problems_with(
    document: DocumentKind,
    body: &Value,
    refuse_unknown: bool,
) -> Vec<ProfileProblem> {
    let DocumentKind::ScoringProfile = document;
    let mut out = Vec::new();
    if !body.is_object() {
        out.push(problem("", "the profile must be a JSON object".to_owned()));
        return out;
    }
    for spec in registry() {
        if let Some(v) = get_path(body, &spec.id) {
            check_value(spec, v, &mut out);
        }
    }
    if refuse_unknown {
        unknown_keys(body, "", &mut out);
    }
    out
}

/// The paths of members the registry does not know (what [`validate`] refuses as
/// `<path>: unknown parameter` and [`validate_for_device`] ignores), in key order.
pub fn unknown_parameters(document: DocumentKind, body: &Value) -> Vec<String> {
    let DocumentKind::ScoringProfile = document;
    let mut out = Vec::new();
    unknown_keys(body, "", &mut out);
    out.into_iter().map(|p| p.path).collect()
}

/// PRM-3: the registry checks of `body`, then the cross-field rules of
/// [`scoring_profile_problems`] (ordering of bands and thresholds, `trackingQuality` rules)
/// for members the registry did not already refuse. Empty when the document is valid.
///
/// Strict: an unknown member is a problem. This is the publish check (backend
/// `config_docs::lifecycle::validate_body`, the console editor); a device uses
/// [`validate_for_device`].
pub fn validate(document: DocumentKind, body: &Value) -> Vec<ProfileProblem> {
    validate_with(document, body, true)
}

/// DSP-6: [`validate`] for a device accepting a delivered profile (`asd_sync::accept_bundle`).
/// Members this build does not know (a section added by a later profile version, e.g. 1.4.0)
/// are **ignored**, so an app keeps working with a newer profile instead of refusing it;
/// kind, range and cross-field checks still apply to every member it knows. The ignored
/// paths are [`unknown_parameters`]. Publishing stays strict ([`validate`]).
pub fn validate_for_device(document: DocumentKind, body: &Value) -> Vec<ProfileProblem> {
    validate_with(document, body, false)
}

fn validate_with(
    document: DocumentKind,
    body: &Value,
    refuse_unknown: bool,
) -> Vec<ProfileProblem> {
    let mut out = registry_problems_with(document, body, refuse_unknown);
    match serde_json::from_value::<ScoringProfile>(body.clone()) {
        Ok(profile) => {
            for p in scoring_profile_problems(&profile) {
                if !out.iter().any(|q| q.path == p.path) {
                    out.push(p);
                }
            }
        }
        Err(e) if out.is_empty() => {
            out.push(problem("", format!("the profile does not decode: {e}")));
        }
        Err(_) => {}
    }
    out
}

/// [`validate`] as messages only (the backend's `invalid_params` reasons).
pub fn validate_messages(document: DocumentKind, body: &Value) -> Vec<String> {
    validate(document, body)
        .into_iter()
        .map(|p| p.message)
        .collect()
}
