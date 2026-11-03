//! owner: WP-X.9
//!
//! SWP-1..3: parameter sweeps over a cohort. A sweep varies one or two registry knobs of the
//! experiment profile (every other value stays as the experiment has it) on a grid
//! `from, from + step, .., <= to` per axis, and reports per cell the flag rates, the quality
//! status rates, the mean and median of the chosen metrics and of the composite, and how many
//! sessions changed flag against the published profile.
//!
//! - [`SweepGrid`] builds the cells (row-major, the last axis fastest). The published value is
//!   always a cell: when the published point is not on the grid it is appended as one extra
//!   cell (SWP-2), so a 625-cell grid may have 626 cells. [`SweepGrid::nearest_cell`] is the
//!   grid cell nearest the experiment's own values (per axis, ties to the lower value); its
//!   evaluation is the run's "draft" side (the RUN-4 blocks and the per-session blob, SWP-3).
//! - [`with_values`] / [`param_value`] read and write a registry id (a camelCase dotted wire
//!   path, PRM-2) of an [`AnalysisProfile`] through its JSON, so the core needs no table of
//!   knobs: an id the core does not read has no value and cannot be swept. Which ids are
//!   sweepable at which level is registry data the caller checks (`asd_contract::params`).
//! - [`SweepAggregator`] takes, per session and cell, the published row side and the cell's
//!   row side, and [`SweepAggregator::finish`] gives the [`SweepSummary`]. No suppression
//!   here: [`super::suppress()`] applies PRV-1 per cell.
//!
//! Grid values are rounded to the decimals of `from` and `step` (at most
//! [`MAX_GRID_DECIMALS`]) so `0.5 + 3 x 0.1` is `0.8`, not `0.8000000000000002`.

use std::collections::BTreeMap;

use serde_json::Value;

use super::aggregate::{
    FlagRates, MeanMedian, RowFlag, RowSide, SweepAxis, SweepCell, SweepSummary,
};
use super::evaluate::QualityStatus;
use super::profile::AnalysisProfile;
use crate::metrics::{sum, PerVideoMetricKey};
use crate::norms::percentile;

/// Decimal places a grid value is rounded to at most (f64 carries ~15 significant digits).
pub const MAX_GRID_DECIMALS: i32 = 12;
/// Relative tolerance of floating-point grid arithmetic (a step count of `2.9999999999` is 3).
const GRID_EPS: f64 = 1e-9;

/// Why a sweep cannot be built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SweepError {
    /// No axis, or a value count that does not match the axes.
    Shape(String),
    /// `from > to`, a step that is not positive, or a value that is not finite.
    Axis(String),
    /// The profile has no number at this registry id.
    NotANumber(String),
    /// The profile with the swept values does not decode.
    Profile(String),
}

impl std::fmt::Display for SweepError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Shape(m) | Self::Axis(m) | Self::Profile(m) => f.write_str(m),
            Self::NotANumber(id) => write!(f, "{id} is not a number the analysis core reads"),
        }
    }
}

impl std::error::Error for SweepError {}

fn check_axis(axis: &SweepAxis) -> Result<(), SweepError> {
    let finite = axis.from.is_finite() && axis.to.is_finite() && axis.step.is_finite();
    if !finite || axis.step <= 0.0 || axis.from > axis.to {
        return Err(SweepError::Axis(format!(
            "{}: needs finite from <= to and a positive step",
            axis.param_id
        )));
    }
    Ok(())
}

/// Steps of an axis: `floor((to - from) / step)` with a tolerance, so the grid has
/// `steps + 1` values (SWP-1, SWP-2).
pub fn axis_steps(axis: &SweepAxis) -> Result<u64, SweepError> {
    check_axis(axis)?;
    let steps = ((axis.to - axis.from) / axis.step + GRID_EPS).floor();
    if !steps.is_finite() || steps < 0.0 || steps > u32::MAX as f64 {
        return Err(SweepError::Axis(format!(
            "{}: too many steps",
            axis.param_id
        )));
    }
    Ok(steps as u64)
}

/// Grid cells of the axes: the product of `steps + 1` (without the published cell).
pub fn grid_cells(axes: &[SweepAxis]) -> Result<u64, SweepError> {
    axes.iter().try_fold(1u64, |acc, a| {
        Ok(acc.saturating_mul(axis_steps(a)?.saturating_add(1)))
    })
}

fn decimals(x: f64) -> i32 {
    (0..=MAX_GRID_DECIMALS)
        .find(|&d| {
            let s = x * 10f64.powi(d);
            (s - s.round()).abs() <= GRID_EPS * s.abs().max(1.0)
        })
        .unwrap_or(MAX_GRID_DECIMALS)
}

fn round_to(x: f64, d: i32) -> f64 {
    let s = 10f64.powi(d);
    let r = (x * s).round() / s;
    if r == 0.0 {
        0.0 // no "-0"
    } else {
        r
    }
}

/// The values of one axis: `from + i * step` for `i in 0..=steps`, rounded to the decimals of
/// `from` and `step`.
pub fn axis_values(axis: &SweepAxis) -> Result<Vec<f64>, SweepError> {
    let steps = axis_steps(axis)?;
    let d = decimals(axis.from).max(decimals(axis.step));
    Ok((0..=steps)
        .map(|i| round_to(axis.from + i as f64 * axis.step, d).min(axis.to.max(axis.from)))
        .collect())
}

fn same(a: f64, b: f64) -> bool {
    (a - b).abs() <= GRID_EPS * a.abs().max(b.abs()).max(1.0)
}

/// The cells of a sweep (module docs).
#[derive(Debug, Clone, PartialEq)]
pub struct SweepGrid {
    pub axes: Vec<SweepAxis>,
    /// One value per axis per cell; grid cells first (row-major), then the published cell
    /// when it is off the grid.
    pub cells: Vec<Vec<f64>>,
    /// The published profile's value per axis.
    pub published_values: Vec<f64>,
    /// Index of the cell whose values are the published ones.
    pub published_cell: usize,
    /// Index of the grid cell nearest the experiment's values.
    pub nearest_cell: usize,
}

impl SweepGrid {
    /// `published` and `experiment`: one value per axis (the profiles' values at the ids).
    pub fn new(
        axes: &[SweepAxis],
        published: &[f64],
        experiment: &[f64],
    ) -> Result<Self, SweepError> {
        if axes.is_empty() || published.len() != axes.len() || experiment.len() != axes.len() {
            return Err(SweepError::Shape(
                "a sweep needs at least one axis and one value per axis".into(),
            ));
        }
        let per_axis: Vec<Vec<f64>> = axes.iter().map(axis_values).collect::<Result<_, _>>()?;
        let mut cells: Vec<Vec<f64>> = vec![Vec::new()];
        for values in &per_axis {
            cells = cells
                .iter()
                .flat_map(|prefix| {
                    values.iter().map(move |v| {
                        let mut c = prefix.clone();
                        c.push(*v);
                        c
                    })
                })
                .collect();
        }
        let index_of = |cells: &[Vec<f64>], point: &[f64]| {
            cells
                .iter()
                .position(|c| c.iter().zip(point).all(|(a, b)| same(*a, *b)))
        };
        let nearest: Vec<f64> = per_axis
            .iter()
            .zip(experiment)
            .map(|(values, e)| {
                values
                    .iter()
                    .copied()
                    .fold(None::<f64>, |best, v| match best {
                        Some(b) if (b - e).abs() <= (v - e).abs() + GRID_EPS => Some(b),
                        _ => Some(v),
                    })
                    .unwrap_or(*e)
            })
            .collect();
        let nearest_cell = index_of(&cells, &nearest).unwrap_or(0);
        let published_cell = match index_of(&cells, published) {
            Some(i) => i,
            None => {
                cells.push(published.to_vec());
                cells.len() - 1
            }
        };
        Ok(Self {
            axes: axes.to_vec(),
            cells,
            published_values: published.to_vec(),
            published_cell,
            nearest_cell,
        })
    }

    /// Every cell's profile: `base` with the cell's values at the axes' ids.
    pub fn profiles(&self, base: &AnalysisProfile) -> Result<Vec<AnalysisProfile>, SweepError> {
        self.cells
            .iter()
            .map(|values| with_values(base, &self.axes, values))
            .collect()
    }
}

fn at<'a>(v: &'a Value, id: &str) -> Option<&'a Value> {
    id.split('.').try_fold(v, |node, key| node.get(key))
}

/// The number at registry id `id` of `profile` (`None`: not a number the core reads).
pub fn param_value(profile: &AnalysisProfile, id: &str) -> Option<f64> {
    let v = serde_json::to_value(profile).ok()?;
    at(&v, id).and_then(Value::as_f64)
}

/// `profile` with `values[i]` at `axes[i].param_id`. Whole values are written as integers so
/// integer knobs decode.
pub fn with_values(
    profile: &AnalysisProfile,
    axes: &[SweepAxis],
    values: &[f64],
) -> Result<AnalysisProfile, SweepError> {
    if axes.len() != values.len() {
        return Err(SweepError::Shape("one value per axis".into()));
    }
    let mut v = serde_json::to_value(profile).map_err(|e| SweepError::Profile(e.to_string()))?;
    for (axis, value) in axes.iter().zip(values) {
        let mut node = &mut v;
        for key in axis.param_id.split('.') {
            node = node
                .get_mut(key)
                .ok_or_else(|| SweepError::NotANumber(axis.param_id.clone()))?;
        }
        if !node.is_number() {
            return Err(SweepError::NotANumber(axis.param_id.clone()));
        }
        let whole = value.fract() == 0.0 && value.abs() < (1u64 << 53) as f64;
        *node = if whole && (node.is_i64() || node.is_u64()) {
            Value::from(*value as i64)
        } else {
            serde_json::Number::from_f64(*value)
                .map(Value::Number)
                .ok_or_else(|| SweepError::Axis(format!("{}: not finite", axis.param_id)))?
        };
    }
    serde_json::from_value(v).map_err(|e| SweepError::Profile(e.to_string()))
}

// ---- aggregation --------------------------------------------------------------------------

const QUALITY: [QualityStatus; 4] = [
    QualityStatus::Good,
    QualityStatus::LowQuality,
    QualityStatus::LowTrackingQuality,
    QualityStatus::Unreliable,
];

#[derive(Debug, Clone, Default)]
struct CellAcc {
    n: u32,
    flags: BTreeMap<RowFlag, u32>,
    quality: BTreeMap<QualityStatus, u32>,
    metrics: BTreeMap<String, Vec<f64>>,
    composite: Vec<f64>,
    changed: u32,
}

/// Incremental SWP-3 aggregation (module docs).
#[derive(Debug, Clone)]
pub struct SweepAggregator {
    axes: Vec<SweepAxis>,
    cell_values: Vec<Vec<f64>>,
    published_values: Vec<f64>,
    metric_keys: Vec<String>,
    cells: Vec<CellAcc>,
}

impl SweepAggregator {
    /// `metric_keys`: the run's chosen metrics (`options.metrics`); empty = the composite only,
    /// so a 625-cell grid stays inside the summary cap (RUN-4).
    pub fn new(grid: &SweepGrid, metric_keys: &[String]) -> Self {
        Self {
            axes: grid.axes.clone(),
            cell_values: grid.cells.clone(),
            published_values: grid.published_values.clone(),
            metric_keys: metric_keys.to_vec(),
            cells: vec![CellAcc::default(); grid.cells.len()],
        }
    }

    /// Adds one session's evaluation in cell `cell` (`published`: the same session under the
    /// published profile). An index outside the grid is ignored.
    pub fn push(&mut self, cell: usize, published: &RowSide, side: &RowSide) {
        let keys = &self.metric_keys;
        let Some(acc) = self.cells.get_mut(cell) else {
            return;
        };
        acc.n += 1;
        if let Some(f) = side.flag {
            *acc.flags.entry(f).or_insert(0) += 1;
        }
        if published.flag != side.flag {
            acc.changed += 1;
        }
        if let Some(q) = side.quality_status {
            *acc.quality.entry(q).or_insert(0) += 1;
        }
        if let Some(c) = side.composite.filter(|c| c.is_finite()) {
            acc.composite.push(c);
        }
        for key in keys {
            if let Some(v) = side.metrics.get(key).copied().flatten() {
                if v.is_finite() {
                    acc.metrics.entry(key.clone()).or_default().push(v);
                }
            }
        }
    }

    /// The SWP-3 block (no suppression).
    pub fn finish(&self) -> SweepSummary {
        let cells = self
            .cells
            .iter()
            .zip(&self.cell_values)
            .map(|(acc, values)| {
                let n = acc.n;
                let share = |c: u32| f64::from(c) / f64::from(n);
                let flag = |f: RowFlag| share(acc.flags.get(&f).copied().unwrap_or(0));
                SweepCell {
                    values: values.clone(),
                    n: Some(n),
                    flag_rate: (n > 0).then(|| FlagRates {
                        green: flag(RowFlag::Green),
                        yellow: flag(RowFlag::Yellow),
                        red: flag(RowFlag::Red),
                        unreliable: flag(RowFlag::Unreliable),
                    }),
                    quality_rate: QUALITY
                        .iter()
                        .map(|q| {
                            let r =
                                (n > 0).then(|| share(acc.quality.get(q).copied().unwrap_or(0)));
                            (q.as_str().to_owned(), r)
                        })
                        .collect(),
                    metrics: self
                        .metric_keys
                        .iter()
                        .map(|k| {
                            let v = acc.metrics.get(k).map_or(&[][..], Vec::as_slice);
                            (k.clone(), mean_median(v))
                        })
                        .collect(),
                    composite: mean_median(&acc.composite),
                    changed_vs_published: Some(acc.changed),
                }
            })
            .collect();
        SweepSummary {
            axes: self.axes.clone(),
            published_values: self.published_values.clone(),
            cells,
        }
    }
}

fn mean_median(values: &[f64]) -> MeanMedian {
    if values.is_empty() {
        return MeanMedian::default();
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    MeanMedian {
        mean: Some(sum(sorted.iter().copied()) / sorted.len() as f64),
        median: Some(percentile(&sorted, 0.5)),
    }
}

/// The metric keys a sweep may summarise: the 15 `PerVideoMetricKey`s.
pub fn is_metric_key(key: &str) -> bool {
    PerVideoMetricKey::ALL.iter().any(|k| k.as_str() == key)
}

/// An upper bound of the JSON bytes of a sweep block with `cells` cells, `axes` axes and
/// `metrics` chosen metrics, every number at full precision (the RUN-4 summary cap is checked
/// with it before a run is queued).
pub fn worst_case_bytes(axes: usize, cells: usize, metrics: &[String]) -> usize {
    const WIDE: f64 = -0.123_456_789_012_345_67;
    let mm = MeanMedian {
        mean: Some(WIDE),
        median: Some(WIDE),
    };
    let cell = SweepCell {
        values: vec![WIDE; axes],
        n: Some(u32::MAX),
        flag_rate: Some(FlagRates {
            green: WIDE,
            yellow: WIDE,
            red: WIDE,
            unreliable: WIDE,
        }),
        quality_rate: QUALITY
            .iter()
            .map(|q| (q.as_str().to_owned(), Some(WIDE)))
            .collect(),
        metrics: metrics.iter().map(|k| (k.clone(), mm)).collect(),
        composite: mm,
        changed_vs_published: Some(u32::MAX),
    };
    let one = serde_json::to_vec(&cell).map_or(0, |b| b.len() + 1);
    one.saturating_mul(cells)
}
