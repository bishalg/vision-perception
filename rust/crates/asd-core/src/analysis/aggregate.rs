//! owner: WP-X.1
//!
//! RUN-4: the summary of an analysis run. [`Aggregator`] takes one [`SessionRow`] at a time
//! (published and draft evaluation of a session) and [`Aggregator::finish`] builds the
//! [`RunSummary`]: counts, flag and quality transitions, and per age band (bands from the
//! profile's `ageBands`, AGE-2) and overall a [`DistComparison`] per metric and for the
//! composite.
//!
//! Distributions ([`dist`]) are exact: values are kept and sorted (a run holds at most a few
//! thousand sessions, RUN-9). Quantiles are linear-interpolated (`norms::percentile`, type 7);
//! `sd` is the sample standard deviation (`n - 1`); the 95 % CI of the mean is a percentile
//! bootstrap whose generator is seeded from the run seed and the cell's path, so a cell's CI
//! does not depend on which other cells exist and two runs with the same seed give the same
//! bytes (RUN-5). No suppression happens here: [`super::suppress()`] applies PRV-1.
//!
//! The summary types mirror `asd_contract::research` field for field (see [`super`]).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::evaluate::{Evaluation, QualityStatus};
use super::Level;
use crate::metrics::{sum, PerVideoMetricKey};
use crate::norms::{bracket_id_for, percentile};
use crate::types::{AgeBracket, AttentionFlag};

/// PRV-1: `p10`/`p90` are reported only from this many observations
/// (`asd_contract::research::MIN_N_FOR_DECILES`).
pub const MIN_N_FOR_DECILES: u32 = 20;
/// Default bootstrap resamples per CI (run option, not a profile knob).
pub const DEFAULT_BOOTSTRAP_RESAMPLES: u32 = 1000;
/// Default histogram bins (run option, not a profile knob).
pub const DEFAULT_HIST_BINS: u32 = 10;
/// Key of the composite in the cell map.
const COMPOSITE: &str = "composite";
/// Band id of the all-bands block.
const OVERALL: &str = "overall";

// ---- summary types (mirror asd_contract::research) -----------------------------------------

/// Histogram: `counts[i]` values in `[edges[i], edges[i + 1])` (the last bin is closed).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hist {
    pub edges: Vec<f64>,
    pub counts: Vec<u32>,
}

/// A distribution summary (RUN-4). Suppressed: `suppressed: true` and every other field null.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Dist {
    pub n: Option<u32>,
    #[serde(default)]
    pub suppressed: bool,
    pub mean: Option<f64>,
    pub sd: Option<f64>,
    pub median: Option<f64>,
    /// `[q1, q3]`.
    pub iqr: Option<[f64; 2]>,
    pub p10: Option<f64>,
    pub p90: Option<f64>,
    pub ci95_mean: Option<[f64; 2]>,
    pub hist: Option<Hist>,
}

/// Published, draft and draft-minus-published distributions of one quantity.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DistComparison {
    pub published: Dist,
    pub draft: Dist,
    pub delta: Dist,
}

/// Flag counts; `null` when suppressed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlagCounts {
    pub green: Option<u32>,
    pub yellow: Option<u32>,
    pub red: Option<u32>,
    pub unreliable: Option<u32>,
}

/// Flag shares in `[0, 1]`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct FlagRates {
    pub green: f64,
    pub yellow: f64,
    pub red: f64,
    pub unreliable: f64,
}

/// `[from, to, n]`; `n` is `null` when suppressed.
pub type Transition = (String, String, Option<u32>);

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct FlagSummary {
    pub published: FlagCounts,
    pub draft: FlagCounts,
    pub transitions: Vec<Transition>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct QualitySummary {
    pub transitions: Vec<Transition>,
}

/// Sessions by evaluated level.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LevelCounts {
    pub frames: u32,
    pub score: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RunCounts {
    pub sessions: u32,
    pub by_level: LevelCounts,
    pub excluded: BTreeMap<String, u32>,
    pub params_not_applied: BTreeMap<String, u32>,
    #[serde(default)]
    pub suppressed_cells: u32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct FlagRateComparison {
    pub published: Option<FlagRates>,
    pub draft: Option<FlagRates>,
}

/// Per-band (and `overall`) block.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BandSummary {
    pub band: String,
    pub n: Option<u32>,
    #[serde(default)]
    pub suppressed: bool,
    pub metrics: BTreeMap<String, DistComparison>,
    pub composite: DistComparison,
    pub flag_rate: FlagRateComparison,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct MeanMedian {
    pub mean: Option<f64>,
    pub median: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SweepAxis {
    pub param_id: String,
    pub from: f64,
    pub to: f64,
    pub step: f64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SweepCell {
    pub values: Vec<f64>,
    pub n: Option<u32>,
    pub flag_rate: Option<FlagRates>,
    pub quality_rate: BTreeMap<String, Option<f64>>,
    pub metrics: BTreeMap<String, MeanMedian>,
    pub composite: MeanMedian,
    pub changed_vs_published: Option<u32>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SweepSummary {
    pub axes: Vec<SweepAxis>,
    /// The published profile's value per axis (the marked cell, SWP-2; WP-X.9).
    #[serde(default)]
    pub published_values: Vec<f64>,
    pub cells: Vec<SweepCell>,
}

/// `analysis_runs.summary` (RUN-4).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RunSummary {
    pub counts: RunCounts,
    pub flags: FlagSummary,
    pub quality: QualitySummary,
    pub by_age_band: Vec<BandSummary>,
    pub overall: BandSummary,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sweep: Option<SweepSummary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub age_profile: Option<Value>,
    /// Set by [`super::suppress()`]; `0` before.
    pub small_cell_k: u32,
    #[serde(default)]
    pub includes_synthetic: bool,
}

// ---- per-session rows ---------------------------------------------------------------------

/// A session's flag as the run counts it (`unreliable` is the backend rule, CON-15).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RowFlag {
    Green,
    Yellow,
    Red,
    Unreliable,
}

impl RowFlag {
    pub fn as_str(self) -> &'static str {
        match self {
            RowFlag::Green => "green",
            RowFlag::Yellow => "yellow",
            RowFlag::Red => "red",
            RowFlag::Unreliable => "unreliable",
        }
    }
}

/// One side (published or draft) of a session row.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RowSide {
    pub composite: Option<f64>,
    pub flag: Option<RowFlag>,
    pub quality_status: Option<QualityStatus>,
    /// Metric key -> session value.
    pub metrics: BTreeMap<String, Option<f64>>,
}

impl RowSide {
    /// The row side of an evaluation: the composite, the flag (`unreliable` when the session
    /// is unscorable or its quality status is `unreliable`), and per metric key (the 15
    /// `PerVideoMetricKey`s) the mean over the clips that carry it.
    pub fn from_evaluation(e: &Evaluation) -> Self {
        let flag = match (&e.flag, e.unscorable, e.quality_status) {
            (_, Some(_), _) | (_, _, QualityStatus::Unreliable) | (None, _, _) => {
                RowFlag::Unreliable
            }
            (Some(f), None, _) => match f.flag {
                AttentionFlag::Green => RowFlag::Green,
                AttentionFlag::Yellow => RowFlag::Yellow,
                AttentionFlag::Red => RowFlag::Red,
            },
        };
        let metrics = PerVideoMetricKey::ALL
            .iter()
            .map(|k| {
                let key = k.as_str();
                let values: Vec<f64> = e
                    .metrics
                    .iter()
                    .filter_map(|s| s.metrics.get(key).copied())
                    .collect();
                let mean =
                    (!values.is_empty()).then(|| sum(values.iter().copied()) / values.len() as f64);
                (key.to_owned(), mean)
            })
            .collect();
        Self {
            composite: e.score.as_ref().map(|s| s.composite_score),
            flag: Some(flag),
            quality_status: Some(e.quality_status),
            metrics,
        }
    }
}

/// One evaluated session of a run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionRow {
    pub level: Level,
    pub age_months: Option<i64>,
    pub published: RowSide,
    pub draft: RowSide,
    /// The draft side's warnings (`param_not_applied: <id>` are counted).
    #[serde(default)]
    pub warnings: Vec<String>,
}

// ---- aggregation --------------------------------------------------------------------------

/// Run options of the aggregation (RUN-2 `options`).
#[derive(Debug, Clone, PartialEq)]
pub struct AggregatorOptions {
    /// Metric keys to summarise; empty = the 15 `PerVideoMetricKey`s.
    pub metrics: Vec<String>,
    /// Bootstrap 95 % CIs of means.
    pub bootstrap: bool,
    pub bootstrap_resamples: u32,
    pub hist_bins: u32,
}

impl Default for AggregatorOptions {
    fn default() -> Self {
        Self {
            metrics: Vec::new(),
            bootstrap: true,
            bootstrap_resamples: DEFAULT_BOOTSTRAP_RESAMPLES,
            hist_bins: DEFAULT_HIST_BINS,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Side {
    Published,
    Draft,
    Delta,
}

impl Side {
    fn as_str(self) -> &'static str {
        match self {
            Side::Published => "published",
            Side::Draft => "draft",
            Side::Delta => "delta",
        }
    }
}

#[derive(Debug, Clone, Default)]
struct FlagTally {
    published: BTreeMap<RowFlag, u32>,
    draft: BTreeMap<RowFlag, u32>,
}

/// Incremental RUN-4 aggregation (module docs).
#[derive(Debug, Clone)]
pub struct Aggregator {
    bands: Vec<AgeBracket>,
    metric_keys: Vec<String>,
    options: AggregatorOptions,
    counts: RunCounts,
    flag_transitions: BTreeMap<(String, String), u32>,
    quality_transitions: BTreeMap<(String, String), u32>,
    /// `(band, key, side)` -> values; band `overall` holds every row.
    cells: BTreeMap<(String, String, Side), Vec<f64>>,
    band_n: BTreeMap<String, u32>,
    band_flags: BTreeMap<String, FlagTally>,
}

impl Aggregator {
    /// `age_bands`: the experiment profile's `ageBands` (AGE-2).
    pub fn new(age_bands: &[AgeBracket], options: AggregatorOptions) -> Self {
        let metric_keys = if options.metrics.is_empty() {
            PerVideoMetricKey::ALL
                .iter()
                .map(|k| k.as_str().to_owned())
                .collect()
        } else {
            options.metrics.clone()
        };
        Self {
            bands: age_bands.to_vec(),
            metric_keys,
            options,
            counts: RunCounts::default(),
            flag_transitions: BTreeMap::new(),
            quality_transitions: BTreeMap::new(),
            cells: BTreeMap::new(),
            band_n: BTreeMap::new(),
            band_flags: BTreeMap::new(),
        }
    }

    /// A cohort session left out of the run (`no_age`, `no_frame_log`, ...).
    pub fn exclude(&mut self, reason: &str) {
        *self.counts.excluded.entry(reason.to_owned()).or_insert(0) += 1;
    }

    /// Adds one evaluated session.
    pub fn push(&mut self, row: &SessionRow) {
        self.counts.sessions += 1;
        match row.level {
            Level::Frames => self.counts.by_level.frames += 1,
            Level::Score => self.counts.by_level.score += 1,
        }
        for w in &row.warnings {
            if let Some(id) = w.strip_prefix("param_not_applied: ") {
                *self
                    .counts
                    .params_not_applied
                    .entry(id.to_owned())
                    .or_insert(0) += 1;
            }
        }
        if let (Some(a), Some(b)) = (row.published.flag, row.draft.flag) {
            *self
                .flag_transitions
                .entry((a.as_str().to_owned(), b.as_str().to_owned()))
                .or_insert(0) += 1;
        }
        if let (Some(a), Some(b)) = (row.published.quality_status, row.draft.quality_status) {
            *self
                .quality_transitions
                .entry((a.as_str().to_owned(), b.as_str().to_owned()))
                .or_insert(0) += 1;
        }

        let band = row
            .age_months
            .and_then(|age| bracket_id_for(age, &self.bands));
        let targets: Vec<String> = std::iter::once(OVERALL.to_owned()).chain(band).collect();
        for b in &targets {
            *self.band_n.entry(b.clone()).or_insert(0) += 1;
            let tally = self.band_flags.entry(b.clone()).or_default();
            if let Some(f) = row.published.flag {
                *tally.published.entry(f).or_insert(0) += 1;
            }
            if let Some(f) = row.draft.flag {
                *tally.draft.entry(f).or_insert(0) += 1;
            }
            self.add_value(b, COMPOSITE, row.published.composite, row.draft.composite);
            for key in self.metric_keys.clone() {
                let p = row.published.metrics.get(&key).copied().flatten();
                let d = row.draft.metrics.get(&key).copied().flatten();
                self.add_value(b, &key, p, d);
            }
        }
    }

    fn add_value(&mut self, band: &str, key: &str, published: Option<f64>, draft: Option<f64>) {
        let mut put = |side: Side, v: f64| {
            if v.is_finite() {
                self.cells
                    .entry((band.to_owned(), key.to_owned(), side))
                    .or_default()
                    .push(v);
            }
        };
        if let Some(p) = published {
            put(Side::Published, p);
        }
        if let Some(d) = draft {
            put(Side::Draft, d);
        }
        if let (Some(p), Some(d)) = (published, draft) {
            put(Side::Delta, d - p);
        }
    }

    /// The summary (no suppression; `small_cell_k` 0). Deterministic for a given `seed`.
    pub fn finish(&self, seed: u64) -> RunSummary {
        let tally_counts = |m: &BTreeMap<RowFlag, u32>| FlagCounts {
            green: Some(m.get(&RowFlag::Green).copied().unwrap_or(0)),
            yellow: Some(m.get(&RowFlag::Yellow).copied().unwrap_or(0)),
            red: Some(m.get(&RowFlag::Red).copied().unwrap_or(0)),
            unreliable: Some(m.get(&RowFlag::Unreliable).copied().unwrap_or(0)),
        };
        let overall_flags = self.band_flags.get(OVERALL).cloned().unwrap_or_default();
        let transitions = |m: &BTreeMap<(String, String), u32>| -> Vec<Transition> {
            m.iter()
                .map(|((a, b), n)| (a.clone(), b.clone(), Some(*n)))
                .collect()
        };
        RunSummary {
            counts: self.counts.clone(),
            flags: FlagSummary {
                published: tally_counts(&overall_flags.published),
                draft: tally_counts(&overall_flags.draft),
                transitions: transitions(&self.flag_transitions),
            },
            quality: QualitySummary {
                transitions: transitions(&self.quality_transitions),
            },
            by_age_band: self
                .bands
                .iter()
                .map(|b| self.band_summary(&b.id, seed))
                .collect(),
            overall: self.band_summary(OVERALL, seed),
            sweep: None,
            age_profile: None,
            small_cell_k: 0,
            includes_synthetic: false,
        }
    }

    fn band_summary(&self, band: &str, seed: u64) -> BandSummary {
        let n = self.band_n.get(band).copied().unwrap_or(0);
        let comparison = |key: &str| {
            let side = |s: Side| {
                let values = self
                    .cells
                    .get(&(band.to_owned(), key.to_owned(), s))
                    .map_or(&[][..], Vec::as_slice);
                let cell_seed = mix(seed, &format!("{band}/{key}/{}", s.as_str()));
                dist(values, &self.options, cell_seed)
            };
            DistComparison {
                published: side(Side::Published),
                draft: side(Side::Draft),
                delta: side(Side::Delta),
            }
        };
        let rates = |m: &BTreeMap<RowFlag, u32>| {
            (n > 0).then(|| {
                let r = |f: RowFlag| f64::from(m.get(&f).copied().unwrap_or(0)) / f64::from(n);
                FlagRates {
                    green: r(RowFlag::Green),
                    yellow: r(RowFlag::Yellow),
                    red: r(RowFlag::Red),
                    unreliable: r(RowFlag::Unreliable),
                }
            })
        };
        let tally = self.band_flags.get(band).cloned().unwrap_or_default();
        BandSummary {
            band: band.to_owned(),
            n: Some(n),
            suppressed: false,
            metrics: self
                .metric_keys
                .iter()
                .map(|k| (k.clone(), comparison(k)))
                .collect(),
            composite: comparison(COMPOSITE),
            flag_rate: FlagRateComparison {
                published: rates(&tally.published),
                draft: rates(&tally.draft),
            },
        }
    }
}

/// The distribution of `values` (non-finite values are ignored): exact sorted quantiles,
/// sample sd, histogram, and with `options.bootstrap` a seeded percentile-bootstrap 95 % CI
/// of the mean. No value gives `n = 0` and nulls.
pub fn dist(values: &[f64], options: &AggregatorOptions, seed: u64) -> Dist {
    let mut sorted: Vec<f64> = values.iter().copied().filter(|v| v.is_finite()).collect();
    sorted.sort_by(f64::total_cmp);
    let n = sorted.len();
    if n == 0 {
        return Dist {
            n: Some(0),
            ..Dist::default()
        };
    }
    let mean = sum(sorted.iter().copied()) / n as f64;
    let sd = (n >= 2).then(|| {
        let ss = sum(sorted.iter().map(|v| (v - mean) * (v - mean)));
        (ss / (n - 1) as f64).sqrt()
    });
    let ci95_mean = (options.bootstrap && n >= 2 && options.bootstrap_resamples > 0)
        .then(|| bootstrap_ci(&sorted, options.bootstrap_resamples, seed));
    Dist {
        n: Some(n as u32),
        suppressed: false,
        mean: Some(mean),
        sd,
        median: Some(percentile(&sorted, 0.5)),
        iqr: Some([percentile(&sorted, 0.25), percentile(&sorted, 0.75)]),
        p10: Some(percentile(&sorted, 0.10)),
        p90: Some(percentile(&sorted, 0.90)),
        ci95_mean,
        hist: Some(hist(&sorted, options.hist_bins.max(1))),
    }
}

fn hist(sorted: &[f64], bins: u32) -> Hist {
    let (lo, hi) = (sorted[0], sorted[sorted.len() - 1]);
    if hi <= lo {
        return Hist {
            edges: vec![lo, hi],
            counts: vec![sorted.len() as u32],
        };
    }
    let width = (hi - lo) / f64::from(bins);
    let mut edges: Vec<f64> = (0..bins).map(|i| lo + f64::from(i) * width).collect();
    edges.push(hi);
    let mut counts = vec![0u32; bins as usize];
    for v in sorted {
        let i = (((v - lo) / width).floor() as usize).min(bins as usize - 1);
        counts[i] += 1;
    }
    Hist { edges, counts }
}

fn bootstrap_ci(sorted: &[f64], resamples: u32, seed: u64) -> [f64; 2] {
    let n = sorted.len();
    let mut rng = SplitMix64(seed);
    let mut means: Vec<f64> = (0..resamples)
        .map(|_| sum((0..n).map(|_| sorted[rng.below(n)])) / n as f64)
        .collect();
    means.sort_by(f64::total_cmp);
    [percentile(&means, 0.025), percentile(&means, 0.975)]
}

/// SplitMix64 (Steele, Lea, Flood 2014): small, seedable, deterministic on every platform.
#[derive(Debug, Clone)]
struct SplitMix64(u64);

impl SplitMix64 {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in `0..n` (Lemire's multiply-shift; the bias is below 2^-40 for run sizes).
    fn below(&mut self, n: usize) -> usize {
        ((u128::from(self.next_u64()) * n as u128) >> 64) as usize
    }
}

/// The seed of one cell: the run seed mixed with an FNV-1a hash of the cell path.
fn mix(seed: u64, path: &str) -> u64 {
    let mut h: u64 = 0xCBF2_9CE4_8422_2325;
    for b in path.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
    }
    SplitMix64(seed ^ h).next_u64()
}
