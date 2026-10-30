//! owner: WP-R.4
//!
//! CORE-8: normative baseline aggregator. Port of `tools/asd_norm_aggregator.py`.
//!
//! Pure functions over already-parsed JSON records; directory walking and file I/O live in
//! `asd-cli` (`asd-core` stays I/O free). Output is a *candidate* baseline for human review.
//!
//! The candidate document is not a plain [`NormativeBaselineSet`](crate::types::NormativeBaselineSet):
//! its `byAge` cells may be `null` (withheld) and carry an `n` count, neither of which
//! `NormativeRange` can express. The candidate types below serialise to the same JSON shape.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::types::{AgeBracket, NormativeDirection};

/// Biomarkers where lower is better (candidate direction inference); all others are
/// higher-is-better.
pub const LOWER_IS_BETTER: [&str; 2] = ["first_entry_latency_ms", "flat_affect_score"];

/// Default minimum observations per cell.
pub const DEFAULT_MIN_N: usize = 12;

/// `(stimulus_id, biomarker, bracket_id)` -> observed values, in encounter order.
pub type Buckets = BTreeMap<(String, String, String), Vec<f64>>;

/// Aggregation parameters (CORE-8).
#[derive(Debug, Clone)]
pub struct AggregateParams {
    pub min_n: usize,
    /// When false (default) rows flagged `imputed` are skipped: sentinels skew distributions.
    pub include_imputed: bool,
    pub brackets: Vec<AgeBracket>,
}

impl Default for AggregateParams {
    fn default() -> Self {
        Self {
            min_n: DEFAULT_MIN_N,
            include_imputed: false,
            brackets: default_age_brackets(),
        }
    }
}

/// The five brackets of `DEFAULT_AGE_BRACKETS`.
pub fn default_age_brackets() -> Vec<AgeBracket> {
    [
        ("12-17", 12, 17),
        ("18-23", 18, 23),
        ("24-35", 24, 35),
        ("36-47", 36, 47),
        ("48-60", 48, 60),
    ]
    .into_iter()
    .map(|(id, min_months, max_months)| AgeBracket {
        id: id.to_string(),
        min_months,
        max_months,
    })
    .collect()
}

/// Candidate cell: median plus one acceptable bound, and the observation count.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateCell {
    pub expected: f64,
    pub n: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acceptable_low: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acceptable_high: Option<f64>,
}

/// Candidate expectation. `target_aoi` serialises as explicit `null` (as the Python tool did).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateExpectation {
    pub biomarker: String,
    pub direction: NormativeDirection,
    #[serde(rename = "targetAOI")]
    pub target_aoi: Option<String>,
    /// Keyed by bracket id; `None` means withheld.
    pub by_age: BTreeMap<String, Option<CandidateCell>>,
    pub citation: String,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateSegment {
    pub id: String,
    pub description: String,
    pub start_sec: f64,
    pub end_sec: f64,
    pub expectations: Vec<CandidateExpectation>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateVideo {
    pub title: String,
    pub category: String,
    pub weight: f64,
    pub segments: Vec<CandidateSegment>,
}

/// Candidate baseline document (same schema family as `normative_baselines.json`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateBaselineSet {
    pub schema_version: String,
    pub status: String,
    pub provenance: String,
    pub age_brackets: Vec<AgeBracket>,
    pub videos: BTreeMap<String, CandidateVideo>,
}

impl CandidateBaselineSet {
    /// Pretty JSON with a trailing newline (the on-disk form of the candidate file).
    pub fn to_json_pretty(&self) -> String {
        let mut s = serde_json::to_string_pretty(self).expect("candidate serialises");
        s.push('\n');
        s
    }
}

/// Bracket id containing `age_months`, or `None` when outside every bracket.
pub fn bracket_id_for(age_months: i64, brackets: &[AgeBracket]) -> Option<String> {
    brackets
        .iter()
        .find(|b| b.min_months <= age_months && age_months <= b.max_months)
        .map(|b| b.id.clone())
}

/// Linear-interpolated percentile of an ascending slice (`pct` in 0..=1). Empty -> 0.0.
pub fn percentile(sorted: &[f64], pct: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let index = (sorted.len() - 1) as f64 * pct;
    let low = index.floor();
    let high = index.ceil();
    if low == high {
        return sorted[low as usize];
    }
    sorted[low as usize] + (sorted[high as usize] - sorted[low as usize]) * (index - low)
}

fn median(sorted: &[f64]) -> f64 {
    let n = sorted.len();
    if n % 2 == 1 {
        sorted[n / 2]
    } else {
        (sorted[n / 2 - 1] + sorted[n / 2]) / 2.0
    }
}

fn round4(x: f64) -> f64 {
    (x * 1e4).round() / 1e4
}

/// Parses the text of one session file into records: JSONL (bad lines skipped) or a single
/// JSON object (possibly pretty-printed) that carries `per_video`.
pub fn load_session_records(text: &str) -> Vec<Value> {
    let text = text.trim();
    if text.is_empty() {
        return Vec::new();
    }
    if text.starts_with('{') && text.contains("\"per_video\"") {
        if let Ok(v) = serde_json::from_str::<Value>(text) {
            return vec![v];
        }
    }
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .collect()
}

/// Adds the observed values of `records` into `buckets`.
///
/// Accepts vault lines (`{"type":"per_video_score","score":{..}}`) and share exports
/// (`{"per_video":{..}}`). Rows whose age is outside every bracket are skipped.
pub fn extract_observations(records: &[Value], params: &AggregateParams, buckets: &mut Buckets) {
    for record in records {
        let Some(obj) = record.as_object() else {
            continue;
        };
        let score = if obj.get("type").and_then(Value::as_str) == Some("per_video_score") {
            obj.get("score")
        } else {
            obj.get("per_video")
        };
        let Some(score) = score.and_then(Value::as_object) else {
            continue;
        };
        let age = score
            .get("ageMonths")
            .and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)))
            .unwrap_or(0);
        let Some(bracket) = bracket_id_for(age, &params.brackets) else {
            continue;
        };
        let videos = score.get("videos").and_then(Value::as_array);
        for video in videos.into_iter().flatten() {
            let stimulus = video
                .get("stimulusId")
                .and_then(Value::as_str)
                .unwrap_or("");
            let rows = video.get("rows").and_then(Value::as_array);
            for row in rows.into_iter().flatten() {
                if !params.include_imputed
                    && row.get("imputed").and_then(Value::as_bool).unwrap_or(false)
                {
                    continue;
                }
                let biomarker = row.get("biomarker").and_then(Value::as_str).unwrap_or("");
                let value = row.get("observed").and_then(Value::as_f64);
                if let (false, Some(v)) = (biomarker.is_empty(), value) {
                    buckets
                        .entry((stimulus.to_string(), biomarker.to_string(), bracket.clone()))
                        .or_default()
                        .push(v);
                }
            }
        }
    }
}

/// Builds the candidate document; cells below `min_n` or with zero spread are withheld.
pub fn build_candidate_baseline(
    buckets: &Buckets,
    params: &AggregateParams,
) -> CandidateBaselineSet {
    let mut grouped: BTreeMap<(&str, &str), BTreeMap<&str, &Vec<f64>>> = BTreeMap::new();
    for ((stimulus, biomarker, bracket), values) in buckets {
        grouped
            .entry((stimulus, biomarker))
            .or_default()
            .insert(bracket, values);
    }

    let mut videos: BTreeMap<String, CandidateVideo> = BTreeMap::new();
    for ((stimulus, biomarker), by_bracket) in grouped {
        let video = videos
            .entry(stimulus.to_string())
            .or_insert_with(|| CandidateVideo {
                title: stimulus.to_string(),
                category: "Candidate Norm".to_string(),
                weight: 0.0,
                segments: vec![CandidateSegment {
                    id: "candidate".to_string(),
                    description:
                        "Aggregated from collected sessions; segment boundaries pending review."
                            .to_string(),
                    start_sec: 0.0,
                    end_sec: 0.0,
                    expectations: Vec::new(),
                }],
            });
        let lower = LOWER_IS_BETTER.contains(&biomarker);
        let mut by_age = BTreeMap::new();
        for bracket in &params.brackets {
            let cell = by_bracket
                .get(bracket.id.as_str())
                .and_then(|values| cell_for(values, lower, params.min_n));
            by_age.insert(bracket.id.clone(), cell);
        }
        video.segments[0].expectations.push(CandidateExpectation {
            biomarker: biomarker.to_string(),
            direction: if lower {
                NormativeDirection::LowerBetter
            } else {
                NormativeDirection::HigherBetter
            },
            target_aoi: None,
            by_age,
            citation: format!(
                "Candidate norm from collected sessions (min n={} per cell); requires clinical review before promotion.",
                params.min_n
            ),
            source: "collected-candidate".to_string(),
        });
    }

    CandidateBaselineSet {
        schema_version: "1.0.0-candidate".to_string(),
        status: "candidate-collected".to_string(),
        provenance: "Aggregated by asd-cli norms aggregate from consented norm-collection sessions. NOT approved for on-device scoring until reviewed and promoted.".to_string(),
        age_brackets: params.brackets.clone(),
        videos,
    }
}

fn cell_for(values: &[f64], lower: bool, min_n: usize) -> Option<CandidateCell> {
    if values.len() < min_n || values.is_empty() {
        return None;
    }
    let mut ordered = values.to_vec();
    ordered.sort_by(|a, b| a.total_cmp(b));
    let spread = ordered[ordered.len() - 1] - ordered[0];
    if spread <= 0.0 {
        return None;
    }
    let mut cell = CandidateCell {
        expected: round4(median(&ordered)),
        n: values.len(),
        acceptable_low: None,
        acceptable_high: None,
    };
    if lower {
        cell.acceptable_high = Some(round4(percentile(&ordered, 0.90)));
    } else {
        cell.acceptable_low = Some(round4(percentile(&ordered, 0.10)));
    }
    Some(cell)
}

/// CORE-8 entry point: records -> candidate baseline.
pub fn aggregate(records: &[Value], params: &AggregateParams) -> CandidateBaselineSet {
    let mut buckets = Buckets::new();
    extract_observations(records, params, &mut buckets);
    build_candidate_baseline(&buckets, params)
}

/// Number of non-withheld cells (for the CLI summary line).
pub fn qualifying_cells(set: &CandidateBaselineSet) -> usize {
    set.videos
        .values()
        .flat_map(|v| &v.segments)
        .flat_map(|s| &s.expectations)
        .flat_map(|e| e.by_age.values())
        .filter(|c| c.is_some())
        .count()
}
