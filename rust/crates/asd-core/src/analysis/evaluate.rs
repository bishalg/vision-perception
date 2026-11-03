//! owner: WP-X.1
//!
//! RPX-2 / RPX-3: [`evaluate`] is the one function every caller (wasm preview, backend job,
//! CLI) uses to score one session with a profile and a baseline set.
//!
//! - `Level::Score` rebuilds metric sets from the session's own score rows
//!   ([`metrics::from_reports_with_warnings`]) and scores without frames: exactly the
//!   `asd-wasm` `rescoreSession` path without frames (same warnings), with the profile's
//!   `lowConfidenceValidRatio` instead of the constant.
//! - `Level::Frames` reprocesses the research frame log ([`reprocess`]) with the profile's gaze
//!   sections and scores the result with its frames (segment windows).
//! - Both then `score_session_with(.., profile.low_confidence_valid_ratio)` and
//!   `flag::assess(.., &profile.flag)`.
//! - `LevelRequest::Auto` is `Frames` when the session carries a research log, else `Score`.
//!   `Frames` asked for without a log evaluates at `Score` with the warning
//!   `level_unavailable: frames`.
//! - A session without `ageMonths` is `unscorable: no_age` at every level (metrics are still
//!   returned; no score, no flag).
//! - `param_not_applied: <id>`: the core adds the ones it knows by itself
//!   (`metrics.ivtUnit = deg`, not implemented before WP-G.13). Which registry knob a level can
//!   apply is registry data (`asd_contract::params`, `reprocess`), so the caller diffs the
//!   experiment profile against the published one with [`changed_params`], filters by the
//!   registry and appends the warnings (RUN-4 counts them).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::profile::{AnalysisProfile, IvtUnit};
use super::{Level, LevelRequest};
use crate::flag;
use crate::gaze::raw_log::RawLog;
use crate::gaze::reprocess::{reprocess, ReprocessOptions, TracePoint};
use crate::metrics::{self, Fixation};
use crate::scoring;
use crate::types::tracking::{TrackingQuality, TrackingVerdict};
use crate::types::{
    AttentionFlagAssessment, GazeFrame, NormativeBaselineSet, PerVideoMetricSet,
    PerVideoScoreReport, PerVideoSessionScore, StimulusCatalog,
};

/// The research frame log of a session and what reprocessing it needs.
#[derive(Debug, Clone, PartialEq)]
pub struct ResearchInput {
    pub log: RawLog,
    /// The catalog version the log header names (AOI fallback and roles).
    pub catalog: StimulusCatalog,
    pub options: ReprocessOptions,
}

/// One stored session, as the evaluation reads it. The caller decodes the session document
/// (any supported schema version) into this.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SessionInput {
    pub session_id: String,
    /// `summary.ageMonths` (the `sessions.age_months` column).
    pub age_months: Option<i64>,
    /// `rawPayload.perVideo` (the device's per-clip score reports).
    pub reports: Vec<PerVideoScoreReport>,
    /// `rawPayload.perVideo` was present.
    pub has_reports: bool,
    /// The session's `validFrameRatio` (score level).
    pub valid_frame_ratio: Option<f64>,
    /// The session's `trackingQuality` (score level).
    pub tracking: Option<TrackingQuality>,
    /// Written verbatim into `score.scoredAtUtc`.
    pub scored_at_utc: String,
    /// The research frame log, when the caller has it (RPX-3).
    pub research: Option<ResearchInput>,
}

/// Why a session has no score.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Unscorable {
    /// No `ageMonths`: no age bracket to score against.
    NoAge,
    /// The research frame log could not be replayed (the error is in `warnings`).
    ReprocessFailed,
}

/// Session quality from the valid ratio and the tracking record, with the profile's limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityStatus {
    Good,
    /// Valid ratio below `review.lowQualityValidRatio` (the backend's review rule).
    LowQuality,
    /// Tracking verdict `poor`/`unusable`, or estimated accuracy above the profile's `fair`
    /// band (`trackingQuality.verdict.accuracyBands.fairMaxDeg`).
    LowTrackingQuality,
    /// Valid ratio below `lowConfidenceValidRatio` (the backend stores the flag `unreliable`).
    Unreliable,
}

impl QualityStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            QualityStatus::Good => "good",
            QualityStatus::LowQuality => "low_quality",
            QualityStatus::LowTrackingQuality => "low_tracking_quality",
            QualityStatus::Unreliable => "unreliable",
        }
    }
}

/// The quality status of a session (the strongest rule wins: unreliable, low tracking, low
/// quality). Without a valid ratio only the tracking rule applies.
pub fn quality_status(
    valid_ratio: Option<f64>,
    tracking: Option<&TrackingQuality>,
    profile: &AnalysisProfile,
) -> QualityStatus {
    if valid_ratio.is_some_and(|r| r < profile.low_confidence_valid_ratio) {
        return QualityStatus::Unreliable;
    }
    let fair_max = profile.tracking_quality.verdict.accuracy_bands.fair_max_deg;
    let low_tracking = tracking.is_some_and(|t| {
        matches!(t.verdict, TrackingVerdict::Poor | TrackingVerdict::Unusable)
            || t.estimated_accuracy_deg.is_some_and(|a| a > fair_max)
    });
    if low_tracking {
        return QualityStatus::LowTrackingQuality;
    }
    if valid_ratio.is_some_and(|r| r < profile.review.low_quality_valid_ratio) {
        return QualityStatus::LowQuality;
    }
    QualityStatus::Good
}

/// What [`evaluate`] returns.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Evaluation {
    pub level: Level,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unscorable: Option<Unscorable>,
    /// `None` when unscorable.
    pub score: Option<PerVideoSessionScore>,
    /// `None` when unscorable.
    pub flag: Option<AttentionFlagAssessment>,
    pub quality_status: QualityStatus,
    /// The valid ratio the quality status was judged on (session field, or reprocessed).
    pub valid_ratio: Option<f64>,
    /// The metric sets that were scored.
    pub metrics: Vec<PerVideoMetricSet>,
    pub tracking: Option<TrackingQuality>,
    /// Frames level: the I-VT fixations (scanpath).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub fixations: Vec<Fixation>,
    /// Frames level with `traceHz`: the down-sampled trace.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace: Option<Vec<TracePoint>>,
    pub warnings: Vec<String>,
}

/// RPX-2: scores `session` with `profile` and `baselines` at the requested level (module docs).
pub fn evaluate(
    session: &SessionInput,
    profile: &AnalysisProfile,
    baselines: &NormativeBaselineSet,
    level: LevelRequest,
) -> Evaluation {
    let mut warnings = Vec::new();
    let research = match level {
        LevelRequest::Score => None,
        LevelRequest::Auto => session.research.as_ref(),
        LevelRequest::Frames => {
            if session.research.is_none() {
                warnings.push("level_unavailable: frames".to_owned());
            }
            session.research.as_ref()
        }
    };

    let (level, metric_sets, frames, valid_ratio, tracking, fixations, trace) = match research {
        Some(r) => match reprocess(&r.log, &r.catalog, &profile.gaze_processing(), &r.options) {
            Ok(rep) => {
                warnings.extend(rep.warnings);
                (
                    Level::Frames,
                    rep.metric_sets,
                    rep.frames,
                    Some(rep.valid_ratio),
                    Some(rep.tracking),
                    rep.fixations,
                    rep.trace,
                )
            }
            Err(e) => {
                warnings.push(format!("reprocess_failed: {e}"));
                return Evaluation {
                    level: Level::Frames,
                    unscorable: Some(Unscorable::ReprocessFailed),
                    score: None,
                    flag: None,
                    quality_status: quality_status(None, None, profile),
                    valid_ratio: None,
                    metrics: Vec::new(),
                    tracking: None,
                    fixations: Vec::new(),
                    trace: None,
                    warnings,
                };
            }
        },
        None => {
            if !session.has_reports {
                warnings.push(
                    "the session carries no rawPayload.perVideo; pass frames to re-score it"
                        .to_owned(),
                );
            }
            if profile.metrics.ivt_unit == IvtUnit::Deg {
                warnings.push("param_not_applied: metrics.ivtUnit".to_owned());
            }
            let (sets, rebuilt) = metrics::from_reports_with_warnings(&session.reports);
            warnings.extend(rebuilt);
            (
                Level::Score,
                sets,
                Vec::<GazeFrame>::new(),
                session.valid_frame_ratio,
                session.tracking.clone(),
                Vec::new(),
                None,
            )
        }
    };

    let quality = quality_status(valid_ratio, tracking.as_ref(), profile);
    let (unscorable, score, flag) = match session.age_months {
        None => (Some(Unscorable::NoAge), None, None),
        Some(age) => {
            let score = scoring::score_session_with(
                &metric_sets,
                baselines,
                age,
                &frames,
                &session.scored_at_utc,
                profile.low_confidence_valid_ratio,
            );
            if level == Level::Score {
                for (set, video) in metric_sets.iter().zip(&score.videos) {
                    if set.observed_duration_sec == 0.0 && video.rows.iter().any(|r| r.imputed) {
                        warnings.push(format!(
                            "{}: an imputed latency uses the 0.5 s floor because the clip \
                             duration is not in the session; pass frames for the device value",
                            video.stimulus_id
                        ));
                    }
                }
            }
            let flag = flag::assess(&score, &profile.flag);
            (None, Some(score), Some(flag))
        }
    };

    Evaluation {
        level,
        unscorable,
        score,
        flag,
        quality_status: quality,
        valid_ratio,
        metrics: metric_sets,
        tracking,
        fixations,
        trace,
        warnings,
    }
}

/// Registry-style ids (camelCase dotted paths, an array is one leaf: PRM-2) of every leaf
/// whose value differs between `profile` and `reference`, sorted. The caller keeps the ids
/// whose registry `reprocess` the evaluated level cannot apply (RPX-3).
pub fn changed_params(profile: &AnalysisProfile, reference: &AnalysisProfile) -> Vec<String> {
    let (Ok(a), Ok(b)) = (
        serde_json::to_value(profile),
        serde_json::to_value(reference),
    ) else {
        return Vec::new();
    };
    let mut la = BTreeMap::new();
    let mut lb = BTreeMap::new();
    leaves(&a, String::new(), &mut la);
    leaves(&b, String::new(), &mut lb);
    let mut ids: Vec<String> = la
        .iter()
        .filter(|(k, v)| lb.get(*k) != Some(*v))
        .map(|(k, _)| k.clone())
        .collect();
    ids.extend(lb.keys().filter(|k| !la.contains_key(*k)).cloned());
    ids.sort();
    ids.dedup();
    ids
}

fn leaves(v: &Value, path: String, out: &mut BTreeMap<String, Value>) {
    match v {
        Value::Object(map) => {
            for (k, child) in map {
                let p = if path.is_empty() {
                    k.clone()
                } else {
                    format!("{path}.{k}")
                };
                leaves(child, p, out);
            }
        }
        other => {
            out.insert(path, other.clone());
        }
    }
}
