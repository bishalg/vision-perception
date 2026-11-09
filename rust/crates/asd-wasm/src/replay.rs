//! owner: WP-X.19
//!
//! The replay exports (ADR-0015 V-1..V-5, V-9) as plain Rust, like [`crate::api`]: each
//! decodes, calls `asd_core::replay` and returns the core types, so native tests run exactly
//! what the wasm exports run. No logic of its own beyond picking a clip's frames
//! (`replay::clip_frames`) and the profile's parameters.

use asd_core::analysis::{AnalysisProfile, IvtUnit};
use asd_core::replay::{
    self, ClipEvents, EventParams, ExpectedBand, Grid, HitParams, MetricRow, ReplayError,
    RibbonSegment, Rule, Screening, Trace, TraceMetric, COHORT_MIN_N,
};
use asd_core::scoring;
use asd_core::types::{Clip, GazeFrame, NormativeBaselineSet};
use serde::Serialize;

use crate::api::{decode, decode_profile};
use crate::error::BoundaryError;

impl From<ReplayError> for BoundaryError {
    fn from(e: ReplayError) -> Self {
        BoundaryError::InvalidArgument(e.to_string())
    }
}

fn profile_or_default(profile_json: Option<&str>) -> Result<AnalysisProfile, BoundaryError> {
    profile_json.map_or_else(|| Ok(AnalysisProfile::default()), decode_profile)
}

/// Result of [`replay_events`].
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplayEvents {
    /// The parameters the events were detected with (from the profile's `metrics`).
    pub params: EventParams,
    /// Events per clip, clips in first-appearance order.
    pub clips: Vec<ClipEvents>,
    /// `param_not_applied: metrics.ivtUnit` when the profile asks for degrees (frames carry
    /// no eye position, so the normalised threshold is used, as the metrics do).
    pub warnings: Vec<String>,
}

/// Fixations, saccades, blinks and loss of every clip in `[GazeFrame]`, with the profile's
/// parameters.
pub fn replay_events(frames_json: &str, profile_json: &str) -> Result<ReplayEvents, BoundaryError> {
    let frames: Vec<GazeFrame> = decode(frames_json, "frames")?;
    let profile = decode_profile(profile_json)?;
    let params = EventParams::from(&profile);
    let mut warnings = Vec::new();
    if profile.metrics.ivt_unit == IvtUnit::Deg {
        warnings.push("param_not_applied: metrics.ivtUnit".to_owned());
    }
    Ok(ReplayEvents {
        clips: replay::events_by_clip(&frames, &params),
        params,
        warnings,
    })
}

/// Result of [`replay_ribbon`].
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplayRibbon {
    /// The clip.
    pub stimulus_id: String,
    /// The ribbon of its frames.
    pub segments: Vec<RibbonSegment>,
}

/// The AOI-hit ribbon of one catalog clip's frames (`aoi.marginNorm`, `aoi.dwellCapMs` from
/// the profile, defaults without one).
pub fn replay_ribbon(
    frames_json: &str,
    clip_json: &str,
    profile_json: Option<&str>,
) -> Result<ReplayRibbon, BoundaryError> {
    let frames: Vec<GazeFrame> = decode(frames_json, "frames")?;
    let clip: Clip = decode(clip_json, "catalog clip")?;
    let profile = profile_or_default(profile_json)?;
    let frames = replay::clip_frames(&frames, &clip.id);
    Ok(ReplayRibbon {
        segments: replay::aoi_ribbon(&frames, &clip.areas, &HitParams::from(&profile)),
        stimulus_id: clip.id,
    })
}

/// Result of [`replay_trace`].
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplayTrace {
    /// The clip.
    pub stimulus_id: String,
    /// The windowed trace.
    pub trace: Trace,
    /// The bracket the bands were read for (`scoring::resolved_bracket_id`); `null` without
    /// an age.
    pub bracket_id: Option<String>,
    /// The published bands of the metric's baseline expectation, one per segment; empty
    /// without an age or when the metric has no baseline.
    pub bands: Vec<ExpectedBand>,
}

/// The windowed trace of `metric_json` (`TraceMetric`) over one catalog clip's frames and,
/// with `age_months`, the published expected bands for the child's bracket.
#[allow(clippy::too_many_arguments)]
pub fn replay_trace(
    frames_json: &str,
    clip_json: &str,
    baselines_json: &str,
    profile_json: &str,
    metric_json: &str,
    window_sec: f64,
    step_sec: f64,
    age_months: Option<i64>,
) -> Result<ReplayTrace, BoundaryError> {
    let frames: Vec<GazeFrame> = decode(frames_json, "frames")?;
    let clip: Clip = decode(clip_json, "catalog clip")?;
    let baselines: NormativeBaselineSet = decode(baselines_json, "baseline set")?;
    let profile = decode_profile(profile_json)?;
    let metric: TraceMetric = decode(metric_json, "trace metric")?;
    if age_months.is_some_and(|a| a < 0) {
        return Err(BoundaryError::InvalidArgument(
            "ageMonths must not be negative".into(),
        ));
    }
    let frames = replay::clip_frames(&frames, &clip.id);
    let trace = replay::windowed_trace(
        &frames,
        &clip.areas,
        &HitParams::from(&profile),
        &metric,
        window_sec,
        step_sec,
    )?;
    let bracket_id = age_months.map(|a| scoring::resolved_bracket_id(&baselines, a));
    let bands = match (&bracket_id, metric.baseline_target()) {
        (Some(bracket), Some((biomarker, target))) => {
            replay::expected_bands(&baselines, &clip.id, bracket, biomarker, target)
        }
        _ => Vec::new(),
    };
    Ok(ReplayTrace {
        stimulus_id: clip.id,
        trace,
        bracket_id,
        bands,
    })
}

/// The expected bands of any baseline expectation (`biomarker`, optional `targetAOI`) of a
/// clip for a child's age: the clip-card bar for metrics without a trace (latency, affect).
pub fn expected_bands(
    baselines_json: &str,
    clip_id: &str,
    age_months: i64,
    biomarker: &str,
    target_aoi: Option<&str>,
) -> Result<Vec<ExpectedBand>, BoundaryError> {
    let baselines: NormativeBaselineSet = decode(baselines_json, "baseline set")?;
    if age_months < 0 {
        return Err(BoundaryError::InvalidArgument(
            "ageMonths must not be negative".into(),
        ));
    }
    Ok(replay::expected_bands_for_age(
        &baselines, clip_id, age_months, biomarker, target_aoi,
    ))
}

/// The session density grid of `[GazeFrame]` (only clip `clip_id`'s frames when given, only
/// media times in `[from_sec, to_sec]` when both are given).
pub fn replay_heatmap(
    frames_json: &str,
    clip_id: Option<&str>,
    w: u32,
    h: u32,
    bandwidth_norm: f64,
    from_sec: Option<f64>,
    to_sec: Option<f64>,
) -> Result<Grid, BoundaryError> {
    let frames: Vec<GazeFrame> = decode(frames_json, "frames")?;
    let frames = match clip_id {
        Some(id) => replay::clip_frames(&frames, id),
        None => frames,
    };
    let window = match (from_sec, to_sec) {
        (Some(a), Some(b)) => Some((a, b)),
        (None, None) => None,
        _ => {
            return Err(BoundaryError::InvalidArgument(
                "fromSec and toSec go together".into(),
            ))
        }
    };
    Ok(replay::heatmap_grid(&frames, w, h, bandwidth_norm, window)?)
}

/// Result of [`heatmap_cohort`].
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CohortHeatmap {
    /// Session grids given.
    pub n: u32,
    /// The minimum applied.
    pub min_n: u32,
    /// `true`: fewer than `minN` grids (or mismatched sizes), so no grid.
    pub suppressed: bool,
    /// The mean grid.
    pub grid: Option<Grid>,
}

/// The cohort mean of session grids (`[Grid]`), suppressed below `min_n` (default 12).
pub fn heatmap_cohort(
    grids_json: &str,
    min_n: Option<u32>,
) -> Result<CohortHeatmap, BoundaryError> {
    let grids: Vec<Grid> = decode(grids_json, "grids")?;
    let min_n = min_n.map_or(COHORT_MIN_N, |m| m as usize);
    let grid = replay::cohort_grid(&grids, min_n);
    Ok(CohortHeatmap {
        n: grids.len() as u32,
        min_n: min_n as u32,
        suppressed: grid.is_none(),
        grid,
    })
}

/// `a - b` of two grids, clamped to [-1, 1].
pub fn heatmap_difference(a_json: &str, b_json: &str) -> Result<Grid, BoundaryError> {
    let a: Grid = decode(a_json, "grid a")?;
    let b: Grid = decode(b_json, "grid b")?;
    replay::difference(&a, &b)
        .ok_or_else(|| BoundaryError::InvalidArgument("the grids differ in size".into()))
}

/// Outlier and quality screening of `[MetricRow]` with `[Rule]`.
pub fn outlier_screen(rows_json: &str, rules_json: &str) -> Result<Screening, BoundaryError> {
    let rows: Vec<MetricRow> = decode(rows_json, "rows")?;
    let rules: Vec<Rule> = decode(rules_json, "rules")?;
    Ok(replay::screen(&rows, &rules)?)
}
