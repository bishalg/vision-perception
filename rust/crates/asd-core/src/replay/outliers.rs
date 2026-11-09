//! Outlier and quality screening of dataset rows (ADR-0015 V-5).
//!
//! [`screen`] applies the rules **in the given order**, each to the rows still kept after the
//! rules before it, so a statistical rule placed after the quality gates computes its bounds
//! on the rows that passed them. Every exclusion names its rule; the outcome of each rule
//! (bounds, n before and after) is reported for the "live n" read-out.
//!
//! - `tukey`: quartiles `Q1`, `Q3` of the metric (linear interpolation between order
//!   statistics, `norms::percentile`, R type 7); excluded when outside
//!   `[Q1 - k IQR, Q3 + k IQR]` (strictly), `k` = 1.5 by default.
//! - `mad`: robust z = `(x - median) / (1.4826 MAD)`, the 1.4826 factor making the MAD a
//!   consistent estimator of the normal standard deviation; excluded when `|z| > z` (3.5 by
//!   default, Iglewicz and Hoaglin). With `MAD = 0` nothing is excluded.
//! - `minValidRatio`, `minClipsCompleted`: excluded below the minimum.
//! - `minQualityVerdict`: excluded below the verdict (`rejected < degraded < acceptable <
//!   optimal`) or without one.
//! - `explicit`: the listed session ids.
//!
//! A row without the metric (or with a non-finite value) is not used by a statistical rule and
//! not excluded by it. Pure and deterministic.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::ReplayError;
use crate::norms::percentile;
use crate::types::GazeQualityVerdict;

/// MAD to standard deviation under normality.
const MAD_CONSISTENCY: f64 = 1.4826;

/// One session as the screening sees it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricRow {
    /// The session id.
    pub session_id: String,
    /// Metric key to value (`social_dwell_ratio`, `aoi_dwell_ratio::Actor_Face`, ...).
    #[serde(default)]
    pub metrics: BTreeMap<String, f64>,
    /// Valid-sample ratio of the session.
    pub valid_ratio: f64,
    /// Clips completed.
    pub clips_completed: u32,
    /// Session quality verdict, if known.
    #[serde(default)]
    pub quality_verdict: Option<GazeQualityVerdict>,
}

fn default_tukey_k() -> f64 {
    1.5
}

fn default_mad_z() -> f64 {
    3.5
}

/// One screening rule; JSON `{"kind": "tukey", "id": "...", "metric": "...", "k": 1.5}`, ...
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Rule {
    /// Tukey fences.
    #[serde(rename_all = "camelCase")]
    Tukey {
        /// Rule id, recorded with each exclusion.
        id: String,
        /// Metric key.
        metric: String,
        /// Fence multiplier (default 1.5).
        #[serde(default = "default_tukey_k")]
        k: f64,
    },
    /// MAD-based robust z.
    #[serde(rename_all = "camelCase")]
    Mad {
        /// Rule id.
        id: String,
        /// Metric key.
        metric: String,
        /// Largest |robust z| kept (default 3.5).
        #[serde(default = "default_mad_z")]
        z: f64,
    },
    /// Minimum valid-sample ratio.
    #[serde(rename_all = "camelCase")]
    MinValidRatio {
        /// Rule id.
        id: String,
        /// The minimum.
        min: f64,
    },
    /// Minimum clips completed.
    #[serde(rename_all = "camelCase")]
    MinClipsCompleted {
        /// Rule id.
        id: String,
        /// The minimum.
        min: u32,
    },
    /// Minimum quality verdict.
    #[serde(rename_all = "camelCase")]
    MinQualityVerdict {
        /// Rule id.
        id: String,
        /// The lowest verdict kept.
        min: GazeQualityVerdict,
    },
    /// Sessions excluded by hand.
    #[serde(rename_all = "camelCase")]
    Explicit {
        /// Rule id.
        id: String,
        /// The excluded session ids.
        session_ids: Vec<String>,
    },
}

impl Rule {
    /// The rule id.
    pub fn id(&self) -> &str {
        match self {
            Self::Tukey { id, .. }
            | Self::Mad { id, .. }
            | Self::MinValidRatio { id, .. }
            | Self::MinClipsCompleted { id, .. }
            | Self::MinQualityVerdict { id, .. }
            | Self::Explicit { id, .. } => id,
        }
    }
}

/// One excluded session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Exclusion {
    /// The session.
    pub session_id: String,
    /// The rule that excluded it.
    pub rule_id: String,
    /// The value tested (metric, valid ratio or clips); `null` for verdict and explicit rules.
    pub value: Option<f64>,
    /// The bound it crossed; `null` for verdict and explicit rules.
    pub bound: Option<f64>,
}

/// What one rule did.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleOutcome {
    /// The rule id.
    pub rule_id: String,
    /// Rows kept before the rule.
    pub n_before: u32,
    /// Rows kept after it.
    pub n_after: u32,
    /// Rows the statistic was computed on (statistical rules), else `null`.
    pub n_used: Option<u32>,
    /// Lower bound kept (inclusive), when the rule has one.
    pub low: Option<f64>,
    /// Upper bound kept (inclusive), when the rule has one.
    pub high: Option<f64>,
}

/// The result of [`screen`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Screening {
    /// Kept session ids, in input order.
    pub kept: Vec<String>,
    /// Exclusions, in rule order then input order.
    pub excluded: Vec<Exclusion>,
    /// One outcome per rule, in rule order.
    pub rules: Vec<RuleOutcome>,
}

/// `Some((value, bound))`: the row is excluded.
type Verdict = Option<(Option<f64>, Option<f64>)>;

fn verdict_rank(v: GazeQualityVerdict) -> u8 {
    match v {
        GazeQualityVerdict::Rejected => 0,
        GazeQualityVerdict::Degraded => 1,
        GazeQualityVerdict::Acceptable => 2,
        GazeQualityVerdict::Optimal => 3,
    }
}

fn check(rule: &Rule) -> Result<(), ReplayError> {
    let bad = |what: &str, v: f64| {
        Err(ReplayError::InvalidArgument(format!(
            "rule {}: {what} must be a non-negative number, got {v}",
            rule.id()
        )))
    };
    match rule {
        Rule::Tukey { k, .. } if !(k.is_finite() && *k >= 0.0) => bad("k", *k),
        Rule::Mad { z, .. } if !(z.is_finite() && *z > 0.0) => bad("z", *z),
        Rule::MinValidRatio { min, .. } if !min.is_finite() => bad("min", *min),
        _ => Ok(()),
    }
}

/// Screens `rows` with `rules` (see the module docs).
pub fn screen(rows: &[MetricRow], rules: &[Rule]) -> Result<Screening, ReplayError> {
    for r in rules {
        check(r)?;
    }
    let mut kept: Vec<usize> = (0..rows.len()).collect();
    let mut excluded = Vec::new();
    let mut outcomes = Vec::new();
    for rule in rules {
        let n_before = kept.len() as u32;
        let mut n_used = None;
        // The bounds kept, and per kept row `Some((value, bound))` when it is excluded.
        let (low, high, verdicts): (Option<f64>, Option<f64>, Vec<Verdict>) = match rule {
            Rule::Tukey { metric, k, .. } | Rule::Mad { metric, z: k, .. } => {
                let mut values: Vec<f64> = kept
                    .iter()
                    .filter_map(|&i| rows[i].metrics.get(metric).copied())
                    .filter(|v| v.is_finite())
                    .collect();
                values.sort_by(f64::total_cmp);
                n_used = Some(values.len() as u32);
                let bounds = if values.is_empty() {
                    None
                } else if matches!(rule, Rule::Tukey { .. }) {
                    let q1 = percentile(&values, 0.25);
                    let q3 = percentile(&values, 0.75);
                    let iqr = q3 - q1;
                    Some((q1 - k * iqr, q3 + k * iqr))
                } else {
                    let median = percentile(&values, 0.5);
                    let mut dev: Vec<f64> = values.iter().map(|v| (v - median).abs()).collect();
                    dev.sort_by(f64::total_cmp);
                    let scale = MAD_CONSISTENCY * percentile(&dev, 0.5);
                    (scale > 0.0).then(|| (median - k * scale, median + k * scale))
                };
                let tests = kept
                    .iter()
                    .map(|&i| {
                        let (lo, hi) = bounds?;
                        let v = *rows[i].metrics.get(metric)?;
                        if !v.is_finite() {
                            None
                        } else if v < lo {
                            Some((Some(v), Some(lo)))
                        } else if v > hi {
                            Some((Some(v), Some(hi)))
                        } else {
                            None
                        }
                    })
                    .collect();
                (bounds.map(|b| b.0), bounds.map(|b| b.1), tests)
            }
            Rule::MinValidRatio { min, .. } => (
                Some(*min),
                None,
                kept.iter()
                    .map(|&i| {
                        let v = rows[i].valid_ratio;
                        (v.is_nan() || v < *min).then_some((Some(v), Some(*min)))
                    })
                    .collect(),
            ),
            Rule::MinClipsCompleted { min, .. } => (
                Some(f64::from(*min)),
                None,
                kept.iter()
                    .map(|&i| {
                        let v = rows[i].clips_completed;
                        (v < *min).then_some((Some(f64::from(v)), Some(f64::from(*min))))
                    })
                    .collect(),
            ),
            Rule::MinQualityVerdict { min, .. } => (
                None,
                None,
                kept.iter()
                    .map(|&i| {
                        let pass = rows[i]
                            .quality_verdict
                            .is_some_and(|v| verdict_rank(v) >= verdict_rank(*min));
                        (!pass).then_some((None, None))
                    })
                    .collect(),
            ),
            Rule::Explicit { session_ids, .. } => {
                let ids: BTreeSet<&str> = session_ids.iter().map(String::as_str).collect();
                (
                    None,
                    None,
                    kept.iter()
                        .map(|&i| {
                            ids.contains(rows[i].session_id.as_str())
                                .then_some((None, None))
                        })
                        .collect(),
                )
            }
        };
        let mut next = Vec::with_capacity(kept.len());
        for (&i, verdict) in kept.iter().zip(verdicts) {
            match verdict {
                Some((value, bound)) => excluded.push(Exclusion {
                    session_id: rows[i].session_id.clone(),
                    rule_id: rule.id().to_owned(),
                    value,
                    bound,
                }),
                None => next.push(i),
            }
        }
        kept = next;
        outcomes.push(RuleOutcome {
            rule_id: rule.id().to_owned(),
            n_before,
            n_after: kept.len() as u32,
            n_used,
            low,
            high,
        });
    }
    Ok(Screening {
        kept: kept.iter().map(|&i| rows[i].session_id.clone()).collect(),
        excluded,
        rules: outcomes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: &str, x: f64) -> MetricRow {
        MetricRow {
            session_id: id.into(),
            metrics: BTreeMap::from([("m".to_owned(), x)]),
            valid_ratio: 0.9,
            clips_completed: 10,
            quality_verdict: Some(GazeQualityVerdict::Optimal),
        }
    }

    /// Values 1..=9 and 30.
    fn rows() -> Vec<MetricRow> {
        let mut r: Vec<MetricRow> = (1..=9).map(|i| row(&format!("s{i}"), i as f64)).collect();
        r.push(row("s10", 30.0));
        r
    }

    #[test]
    fn tukey_bounds_are_hand_computed() {
        // Sorted 1..9, 30 (n = 10): Q1 at index 2.25 = 3.25, Q3 at index 6.75 = 7.75,
        // IQR 4.5, fences -3.5 and 14.5.
        let s = screen(
            &rows(),
            &[Rule::Tukey {
                id: "t".into(),
                metric: "m".into(),
                k: 1.5,
            }],
        )
        .unwrap();
        assert_eq!(s.rules[0].low, Some(-3.5));
        assert_eq!(s.rules[0].high, Some(14.5));
        assert_eq!(
            s.excluded,
            vec![Exclusion {
                session_id: "s10".into(),
                rule_id: "t".into(),
                value: Some(30.0),
                bound: Some(14.5),
            }]
        );
        assert_eq!(s.kept.len(), 9);
        assert_eq!((s.rules[0].n_before, s.rules[0].n_after), (10, 9));
    }

    #[test]
    fn mad_bounds_are_hand_computed() {
        // Median 5.5; |x - 5.5| = 4.5 3.5 2.5 1.5 0.5 0.5 1.5 2.5 3.5 24.5 -> sorted median
        // (2.5 + 2.5) / 2 = 2.5; scale 1.4826 * 2.5 = 3.7065; bounds 5.5 -+ 3.5 * 3.7065.
        let s = screen(
            &rows(),
            &[Rule::Mad {
                id: "mad".into(),
                metric: "m".into(),
                z: 3.5,
            }],
        )
        .unwrap();
        let half = 3.5 * 1.4826 * 2.5;
        assert!((s.rules[0].low.unwrap() - (5.5 - half)).abs() < 1e-12);
        assert!((s.rules[0].high.unwrap() - (5.5 + half)).abs() < 1e-12);
        assert_eq!(s.excluded.len(), 1);
        assert_eq!(s.excluded[0].session_id, "s10");
        // MAD 0: nothing is excluded.
        let flat: Vec<MetricRow> = (0..5).map(|i| row(&i.to_string(), 1.0)).collect();
        let s = screen(
            &flat,
            &[Rule::Mad {
                id: "mad".into(),
                metric: "m".into(),
                z: 3.5,
            }],
        )
        .unwrap();
        assert!(s.excluded.is_empty());
        assert_eq!(s.rules[0].low, None);
    }

    #[test]
    fn rules_run_in_order_on_the_survivors() {
        let mut r = rows();
        r[0].valid_ratio = 0.5; // s1
        r[1].clips_completed = 2; // s2
        r[2].quality_verdict = Some(GazeQualityVerdict::Degraded); // s3
        r[3].quality_verdict = None; // s4
        let rules = vec![
            Rule::MinValidRatio {
                id: "valid".into(),
                min: 0.6,
            },
            Rule::MinClipsCompleted {
                id: "clips".into(),
                min: 5,
            },
            Rule::MinQualityVerdict {
                id: "verdict".into(),
                min: GazeQualityVerdict::Acceptable,
            },
            Rule::Explicit {
                id: "manual".into(),
                session_ids: vec!["s9".into(), "unknown".into()],
            },
            Rule::Tukey {
                id: "tukey".into(),
                metric: "m".into(),
                k: 1.5,
            },
        ];
        let s = screen(&r, &rules).unwrap();
        let ex: Vec<(&str, &str)> = s
            .excluded
            .iter()
            .map(|e| (e.session_id.as_str(), e.rule_id.as_str()))
            .collect();
        assert_eq!(
            ex,
            vec![
                ("s1", "valid"),
                ("s2", "clips"),
                ("s3", "verdict"),
                ("s4", "verdict"),
                ("s9", "manual"),
                ("s10", "tukey"),
            ]
        );
        // Tukey on 5, 6, 7, 8, 30: Q1 6, Q3 8, IQR 2, fences 3 and 11.
        let t = &s.rules[4];
        assert_eq!(
            (t.n_before, t.n_used, t.low, t.high),
            (5, Some(5), Some(3.0), Some(11.0))
        );
        assert_eq!(s.kept, vec!["s5", "s6", "s7", "s8"]);
        assert_eq!(s.excluded[1].value, Some(2.0));
        assert_eq!(s.excluded[1].bound, Some(5.0));
    }

    #[test]
    fn rows_without_the_metric_are_kept_and_bad_rules_refused() {
        let mut r = rows();
        r[4].metrics.clear();
        let s = screen(
            &r,
            &[Rule::Tukey {
                id: "t".into(),
                metric: "m".into(),
                k: 1.5,
            }],
        )
        .unwrap();
        assert!(s.kept.contains(&"s5".to_owned()));
        assert_eq!(s.rules[0].n_used, Some(9));
        let bad = Rule::Mad {
            id: "x".into(),
            metric: "m".into(),
            z: 0.0,
        };
        assert!(screen(&r, &[bad]).is_err());
        assert!(screen(
            &r,
            &[Rule::Tukey {
                id: "x".into(),
                metric: "m".into(),
                k: f64::NAN
            }]
        )
        .is_err());
    }

    #[test]
    fn rule_json_defaults() {
        let rules: Vec<Rule> = serde_json::from_str(
            r#"[{"kind":"tukey","id":"a","metric":"m"},{"kind":"mad","id":"b","metric":"m"},
                {"kind":"minQualityVerdict","id":"c","min":"acceptable"},
                {"kind":"explicit","id":"d","sessionIds":["x"]}]"#,
        )
        .unwrap();
        assert_eq!(
            rules[0],
            Rule::Tukey {
                id: "a".into(),
                metric: "m".into(),
                k: 1.5
            }
        );
        assert_eq!(
            rules[1],
            Rule::Mad {
                id: "b".into(),
                metric: "m".into(),
                z: 3.5
            }
        );
    }
}
