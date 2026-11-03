//! owner: WP-X.9
//!
//! AGE-1: the `age_profile` block of a run summary (`summary.age_profile`). Per age band (the
//! experiment profile's `ageBands`, AGE-2) and per normative expectation the sessions were
//! scored against (stimulus, segment, metric key), it reports the cohort's distribution of the
//! observed value against the baseline's range for that band:
//!
//! - `shares`: the share of scored cells `on_target` / `monitor` / `atypical`, taken from the
//!   status the scoring itself gave each row (`scoring::evaluate`; nothing is re-classified
//!   here);
//! - `expected_percentile`: where the band's `expected` value falls in the cohort (mid-rank
//!   empirical percentile in `[0, 100]`), when every row of the cell had the same range;
//! - `range`: that common `{expected, acceptable_low, acceptable_high}`, else `null`.
//!
//! Each figure is given twice: `published` (the published profile with the published
//! baselines) and `draft` (the experiment profile with the experiment's baselines version,
//! the published one when it names none). With an unchanged profile and a draft baselines
//! version this answers "how a draft baseline would move them".
//!
//! Rows with an imputed observation count only when the experiment profile's
//! `norms.includeImputed` is set. PRV-1 / AGE-3 suppression ([`suppress_age_profile`]) uses
//! `max(k, norms.minN)`: a band or cell with fewer sessions keeps no number.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::aggregate::{dist, AggregatorOptions, Dist};
use super::evaluate::Evaluation;
use super::suppress::suppress_dist;
use crate::norms::bracket_id_for;
use crate::types::{AgeBracket, ExpectationStatus, NormativeDirection, PerVideoScoreRow};

/// A baseline range as the scoring resolved it for a row.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct BaselineRange {
    pub expected: Option<f64>,
    pub acceptable_low: Option<f64>,
    pub acceptable_high: Option<f64>,
}

/// Shares of scored cells by status, in `[0, 1]`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct StatusShares {
    pub on_target: f64,
    pub monitor: f64,
    pub atypical: f64,
}

/// One side (published or draft) of an expectation cell.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AgeProfileSide {
    /// Sessions with a scored row for this expectation.
    pub n: Option<u32>,
    #[serde(default)]
    pub suppressed: bool,
    /// The observed values.
    pub dist: Dist,
    /// The common range of the rows; `null` when they differ or there is none.
    pub range: Option<BaselineRange>,
    pub shares: Option<StatusShares>,
    /// Percentile of `range.expected` within the observed values.
    pub expected_percentile: Option<f64>,
}

/// One normative expectation in one band.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgeProfileExpectation {
    pub stimulus_id: String,
    pub segment_id: String,
    pub biomarker: String,
    pub target_aoi: Option<String>,
    /// `biomarker` or `biomarker::AOI` (the scoring key).
    pub metric_key: String,
    pub direction: NormativeDirection,
    pub published: AgeProfileSide,
    pub draft: AgeProfileSide,
}

/// One age band.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AgeProfileBand {
    pub band: String,
    pub min_months: i64,
    pub max_months: i64,
    /// Sessions of the band.
    pub n: Option<u32>,
    #[serde(default)]
    pub suppressed: bool,
    pub expectations: Vec<AgeProfileExpectation>,
}

/// The baselines versions of the two sides.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgeProfileBaselines {
    pub published: String,
    pub draft: String,
}

/// `summary.age_profile` (AGE-1).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AgeProfileSummary {
    pub baselines: AgeProfileBaselines,
    /// The experiment profile's `norms.minN` (AGE-3).
    pub min_n: u32,
    /// `max(k, min_n)`, set by [`suppress_age_profile`]; `0` before.
    #[serde(default)]
    pub suppression_threshold: u32,
    pub bands: Vec<AgeProfileBand>,
}

#[derive(Debug, Clone, Default)]
struct SideAcc {
    values: Vec<f64>,
    on_target: u32,
    monitor: u32,
    atypical: u32,
    /// `None` until the first row; `Some(None)` once rows disagree.
    range: Option<Option<BaselineRange>>,
}

impl SideAcc {
    fn push(&mut self, row: &PerVideoScoreRow) {
        if row.observed.is_finite() {
            self.values.push(row.observed);
        }
        match row.status {
            ExpectationStatus::OnTarget => self.on_target += 1,
            ExpectationStatus::Monitor => self.monitor += 1,
            ExpectationStatus::Atypical => self.atypical += 1,
        }
        let r = BaselineRange {
            expected: row.expected,
            acceptable_low: row.acceptable_low,
            acceptable_high: row.acceptable_high,
        };
        self.range = Some(match self.range {
            None => Some(r),
            Some(Some(prev)) if prev == r => Some(prev),
            Some(_) => None,
        });
    }

    fn finish(&self, options: &AggregatorOptions, seed: u64) -> AgeProfileSide {
        let n = self.on_target + self.monitor + self.atypical;
        let range = self.range.flatten();
        let mut sorted = self.values.clone();
        sorted.sort_by(f64::total_cmp);
        let share = |c: u32| f64::from(c) / f64::from(n);
        AgeProfileSide {
            n: Some(n),
            suppressed: false,
            dist: dist(&sorted, options, seed),
            range,
            shares: (n > 0).then(|| StatusShares {
                on_target: share(self.on_target),
                monitor: share(self.monitor),
                atypical: share(self.atypical),
            }),
            expected_percentile: range
                .and_then(|r| r.expected)
                .and_then(|e| percentile_of(&sorted, e)),
        }
    }
}

/// Mid-rank empirical percentile of `x` within `sorted`, in `[0, 100]`; `None` when empty.
pub fn percentile_of(sorted: &[f64], x: f64) -> Option<f64> {
    if sorted.is_empty() || !x.is_finite() {
        return None;
    }
    let below = sorted.partition_point(|v| *v < x);
    let at_or_below = sorted.partition_point(|v| *v <= x);
    let equal = at_or_below - below;
    Some(100.0 * (below as f64 + 0.5 * equal as f64) / sorted.len() as f64)
}

/// `(stimulus_id, segment_id, metric_key)`.
type Key = (String, String, String);

#[derive(Debug, Clone)]
struct CellAcc {
    biomarker: String,
    target_aoi: Option<String>,
    direction: NormativeDirection,
    published: SideAcc,
    draft: SideAcc,
}

/// Incremental AGE-1 aggregation (module docs).
#[derive(Debug, Clone)]
pub struct AgeProfileAggregator {
    bands: Vec<AgeBracket>,
    include_imputed: bool,
    options: AggregatorOptions,
    band_n: BTreeMap<String, u32>,
    cells: BTreeMap<(String, Key), CellAcc>,
}

impl AgeProfileAggregator {
    /// `bands`: the experiment profile's `ageBands`; `include_imputed`: its
    /// `norms.includeImputed`.
    pub fn new(bands: &[AgeBracket], include_imputed: bool, options: AggregatorOptions) -> Self {
        Self {
            bands: bands.to_vec(),
            include_imputed,
            options,
            band_n: BTreeMap::new(),
            cells: BTreeMap::new(),
        }
    }

    /// Adds one session (its published and draft evaluations). A session outside every band
    /// is ignored.
    pub fn push(&mut self, age_months: Option<i64>, published: &Evaluation, draft: &Evaluation) {
        let Some(band) = age_months.and_then(|a| bracket_id_for(a, &self.bands)) else {
            return;
        };
        *self.band_n.entry(band.clone()).or_insert(0) += 1;
        for (side, evaluation) in [(false, published), (true, draft)] {
            let Some(score) = &evaluation.score else {
                continue;
            };
            for video in &score.videos {
                for row in &video.rows {
                    if row.imputed && !self.include_imputed {
                        continue;
                    }
                    let key = (
                        video.stimulus_id.clone(),
                        row.segment_id.clone(),
                        row.metric_key.clone(),
                    );
                    let cell = self
                        .cells
                        .entry((band.clone(), key))
                        .or_insert_with(|| CellAcc {
                            biomarker: row.biomarker.clone(),
                            target_aoi: row.target_aoi.clone(),
                            direction: row.direction,
                            published: SideAcc::default(),
                            draft: SideAcc::default(),
                        });
                    if side {
                        cell.draft.push(row);
                    } else {
                        cell.published.push(row);
                    }
                }
            }
        }
    }

    /// The block (no suppression). `min_n`: the experiment profile's `norms.minN`.
    pub fn finish(
        &self,
        baselines: AgeProfileBaselines,
        min_n: u32,
        seed: u64,
    ) -> AgeProfileSummary {
        let bands = self
            .bands
            .iter()
            .map(|b| {
                let expectations = self
                    .cells
                    .range((b.id.clone(), Key::default())..)
                    .take_while(|((band, _), _)| *band == b.id)
                    .map(|((band, (stimulus, segment, key)), c)| {
                        let path = format!("age/{band}/{stimulus}/{segment}/{key}");
                        AgeProfileExpectation {
                            stimulus_id: stimulus.clone(),
                            segment_id: segment.clone(),
                            biomarker: c.biomarker.clone(),
                            target_aoi: c.target_aoi.clone(),
                            metric_key: key.clone(),
                            direction: c.direction,
                            published: c
                                .published
                                .finish(&self.options, seed_of(seed, &format!("{path}/published"))),
                            draft: c
                                .draft
                                .finish(&self.options, seed_of(seed, &format!("{path}/draft"))),
                        }
                    })
                    .collect();
                AgeProfileBand {
                    band: b.id.clone(),
                    min_months: b.min_months,
                    max_months: b.max_months,
                    n: Some(self.band_n.get(&b.id).copied().unwrap_or(0)),
                    suppressed: false,
                    expectations,
                }
            })
            .collect();
        AgeProfileSummary {
            baselines,
            min_n,
            suppression_threshold: 0,
            bands,
        }
    }
}

/// The bootstrap seed of one cell: the run seed mixed with an FNV-1a hash of its path (a cell's
/// CI does not depend on which other cells exist, RUN-5).
fn seed_of(seed: u64, path: &str) -> u64 {
    let mut h: u64 = 0xCBF2_9CE4_8422_2325;
    for b in path.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
    }
    seed ^ h
}

fn suppress_side(side: &mut AgeProfileSide, t: u32) {
    if side.suppressed || side.n.is_none_or(|n| n < t) {
        *side = AgeProfileSide {
            suppressed: true,
            // The range is baseline data, not cohort data: it stays.
            range: side.range,
            dist: Dist {
                suppressed: true,
                ..Dist::default()
            },
            ..AgeProfileSide::default()
        };
        return;
    }
    suppress_dist(&mut side.dist, t);
}

/// PRV-1 / AGE-3 on the block with `t = max(k, min_n)`: a band below `t` keeps only its id
/// and edges; a cell side below `t` keeps only its range. Idempotent.
pub fn suppress_age_profile(profile: &mut AgeProfileSummary, k: u32) {
    let t = k.max(profile.min_n);
    profile.suppression_threshold = t;
    for band in &mut profile.bands {
        if band.suppressed || band.n.is_none_or(|n| n < t) {
            band.suppressed = true;
            band.n = None;
        }
        for e in &mut band.expectations {
            if band.suppressed {
                e.published.suppressed = true;
                e.draft.suppressed = true;
            }
            suppress_side(&mut e.published, t);
            suppress_side(&mut e.draft, t);
        }
    }
}
