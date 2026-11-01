//! owner: WP-R.6
//!
//! The exported operations as plain Rust (no wasm-bindgen types), so native tests call
//! exactly what the wasm exports call. Each function decodes, delegates to asd-core /
//! asd-contract and encodes; the only logic of its own is reading a stored session
//! ([`crate::session`]).

use asd_contract::config::{
    ScoringProfile, CONFIG_BUNDLE_SCHEMA_ID, SCORING_PROFILE_SCHEMA_VERSION,
};
use asd_contract::{session_v1_6, session_v1_7};
use asd_core::types::{
    AttentionFlag, AttentionFlagAssessment, GazeFrame, NormativeBaselineSet, PerVideoMetricSet,
    PerVideoSessionScore,
};
use asd_core::{baselines, flag, metrics, scoring};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::error::BoundaryError;
use crate::session;

fn decode<T: DeserializeOwned>(json: &str, what: &str) -> Result<T, BoundaryError> {
    serde_json::from_str(json).map_err(|e| BoundaryError::invalid_json(what, e))
}

/// `ScoringProfile.flag` as the core's thresholds (the same mapping as asd-ffi).
fn thresholds(profile: &ScoringProfile) -> flag::FlagThresholds {
    let t = &profile.flag;
    flag::FlagThresholds {
        green_floor: t.green_floor,
        yellow_floor: t.yellow_floor,
        low_confidence_share: t.low_confidence_share,
        atypical_share_for_red: t.atypical_share_for_red,
    }
}

// --- version and defaults --------------------------------------------------------------

/// Versions of the core and of the contracts this build reads and writes.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContractInfo {
    /// Workspace version of the linked Rust core.
    pub core_version: &'static str,
    /// Session `schemaVersion` values [`rescore_session`] accepts (wire form).
    pub session_schema_versions: [&'static str; 2],
    pub session_schema_ids: [&'static str; 2],
    /// `schemaVersion` of [`default_scoring_profile`].
    pub scoring_profile_schema_version: &'static str,
    pub config_bundle_schema_id: &'static str,
    /// The core's low-confidence clip gate (`validSampleRatio` below it). The scorer uses
    /// this constant; `ScoringProfile.lowConfidenceValidRatio` does not change it.
    pub low_confidence_valid_ratio: f64,
}

pub fn contract_info() -> ContractInfo {
    ContractInfo {
        core_version: env!("CARGO_PKG_VERSION"),
        session_schema_versions: [session_v1_6::SCHEMA_VERSION, session_v1_7::SCHEMA_VERSION],
        session_schema_ids: [session_v1_6::SCHEMA_ID, session_v1_7::SCHEMA_ID],
        scoring_profile_schema_version: SCORING_PROFILE_SCHEMA_VERSION,
        config_bundle_schema_id: CONFIG_BUNDLE_SCHEMA_ID,
        low_confidence_valid_ratio: scoring::LOW_CONFIDENCE_VALID_RATIO,
    }
}

/// The bundled default `ScoringProfile` (the Swift constants, CON-5).
pub fn default_scoring_profile() -> ScoringProfile {
    ScoringProfile::default()
}

// --- validation ---------------------------------------------------------------------------

/// Outcome of a document check: `valid` is `problems.is_empty()`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Validation {
    pub valid: bool,
    pub problems: Vec<String>,
}

impl Validation {
    fn from(problems: Vec<String>) -> Self {
        Self {
            valid: problems.is_empty(),
            problems,
        }
    }
}

/// Shape check of a `ScoringProfile` (it decodes as the contract type). Range checks are
/// not in a shared crate yet, so they are not run here.
pub fn validate_scoring_profile(profile_json: &str) -> Validation {
    Validation::from(
        match decode::<ScoringProfile>(profile_json, "scoring profile") {
            Ok(_) => Vec::new(),
            Err(e) => vec![e.message().to_owned()],
        },
    )
}

/// `NormativeBaselineLibrary.validate` (asd-core `baselines::validate`): the set decodes
/// and every expectation has a range for every bracket; `required_ids_json` (a JSON array
/// of stimulus ids, optional) must each have a video profile.
pub fn validate_baselines(
    baselines_json: &str,
    required_ids_json: Option<&str>,
) -> Result<Validation, BoundaryError> {
    let required: Vec<String> = match required_ids_json {
        Some(json) => decode(json, "required ids")?,
        None => Vec::new(),
    };
    Ok(Validation::from(
        match decode::<NormativeBaselineSet>(baselines_json, "baseline set") {
            Ok(set) => baselines::validate(&set, &required),
            Err(e) => vec![e.message().to_owned()],
        },
    ))
}

// --- scoring primitives (same as asd-ffi) ----------------------------------------------

/// `PerVideoScoringEngine.score`: `[PerVideoMetricSet]` + `NormativeBaselineSet` + all
/// session `[GazeFrame]` -> `PerVideoSessionScore` (`scored_at_utc` written verbatim).
pub fn score_session(
    metric_sets_json: &str,
    baselines_json: &str,
    age_months: i64,
    frames_json: &str,
    scored_at_utc: &str,
) -> Result<PerVideoSessionScore, BoundaryError> {
    let metric_sets: Vec<PerVideoMetricSet> = decode(metric_sets_json, "metric sets")?;
    let baseline: NormativeBaselineSet = decode(baselines_json, "baseline set")?;
    let frames: Vec<GazeFrame> = decode(frames_json, "frames")?;
    Ok(scoring::score_session(
        &metric_sets,
        &baseline,
        age_months,
        &frames,
        scored_at_utc,
    ))
}

/// `AttentionFlagAlgorithm.assess`: `PerVideoSessionScore` + `ScoringProfile` (only `flag`
/// is read) -> `AttentionFlagAssessment`.
pub fn assess_flag(
    score_json: &str,
    profile_json: &str,
) -> Result<AttentionFlagAssessment, BoundaryError> {
    let score: PerVideoSessionScore = decode(score_json, "session score")?;
    let profile: ScoringProfile = decode(profile_json, "scoring profile")?;
    Ok(flag::assess(&score, &thresholds(&profile)))
}

/// `PerVideoMetricsExtractor.extract(from:)`: `[GazeFrame]` -> `[PerVideoMetricSet]`.
pub fn extract_metrics(frames_json: &str) -> Result<Vec<PerVideoMetricSet>, BoundaryError> {
    let frames: Vec<GazeFrame> = decode(frames_json, "frames")?;
    Ok(metrics::extract(&frames))
}

// --- sandbox re-score ------------------------------------------------------------------------

/// Options of [`rescore_session`]. Unknown keys are refused so a typo is not ignored.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RescoreOptions {
    /// The child's age at capture; the session does not carry it.
    pub age_months: i64,
    /// Written verbatim into `score.scoredAtUtc`; empty when absent.
    #[serde(default)]
    pub scored_at_utc: String,
}

/// Where the re-score's metric sets came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum MetricSource {
    /// `metrics::extract` over the session frames: the device path, segment windows included.
    Frames,
    /// Rebuilt from `rawPayload.perVideo[].rows[]` (see `warnings` for what that loses).
    SessionReport,
}

/// What the session itself recorded, for a side-by-side diff.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceResult {
    /// `rawPayload.perVideoComposite`.
    pub composite_score: Option<f64>,
    /// The request's `attentionFlag` (`null` for a bare record or a server-derived flag).
    pub attention_flag: Option<AttentionFlag>,
}

/// Result of a sandbox re-score.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RescoreResult {
    pub session_id: String,
    /// Wire form (`1.6`, `1.7`).
    pub schema_version: String,
    pub metric_source: MetricSource,
    /// `scoring::score_session` with the given baselines.
    pub score: PerVideoSessionScore,
    /// `flag::assess` of `score` with the given profile's thresholds.
    pub flag: AttentionFlagAssessment,
    pub device: DeviceResult,
    pub warnings: Vec<String>,
}

/// Re-scores a stored session with a (draft) scoring profile and baseline set, the way the
/// device scores it: `scoring::score_session` then `flag::assess` with the profile's flag
/// thresholds. With `frames_json` (the session's `[GazeFrame]`), the metric sets come from
/// `metrics::extract` exactly as on the device; without, they are rebuilt from the
/// session's score rows ([`session::metric_sets_from_reports`]).
pub fn rescore_session(
    session_json: &str,
    profile_json: &str,
    baselines_json: &str,
    options_json: &str,
    frames_json: Option<&str>,
) -> Result<RescoreResult, BoundaryError> {
    let view = session::decode(session_json)?;
    let profile: ScoringProfile = decode(profile_json, "scoring profile")?;
    let baseline: NormativeBaselineSet = decode(baselines_json, "baseline set")?;
    let options: RescoreOptions = decode(options_json, "options")?;
    if options.age_months < 0 {
        return Err(BoundaryError::InvalidArgument(
            "options.ageMonths must not be negative".into(),
        ));
    }

    let mut warnings = Vec::new();
    let (metric_source, score) = match frames_json {
        Some(json) => {
            let frames: Vec<GazeFrame> = decode(json, "frames")?;
            if frames.is_empty() {
                warnings.push("frames is empty, so no clip is scored".to_owned());
            }
            let metric_sets = metrics::extract(&frames);
            let score = scoring::score_session(
                &metric_sets,
                &baseline,
                options.age_months,
                &frames,
                &options.scored_at_utc,
            );
            (MetricSource::Frames, score)
        }
        None => {
            if !view.has_reports {
                warnings.push(
                    "the session carries no rawPayload.perVideo; pass frames to re-score it"
                        .to_owned(),
                );
            }
            let (metric_sets, rebuilt) = session::metric_sets_from_reports(&view.reports);
            warnings.extend(rebuilt);
            let score = scoring::score_session(
                &metric_sets,
                &baseline,
                options.age_months,
                &[],
                &options.scored_at_utc,
            );
            for (set, video) in metric_sets.iter().zip(&score.videos) {
                if set.observed_duration_sec == 0.0 && video.rows.iter().any(|r| r.imputed) {
                    warnings.push(format!(
                        "{}: an imputed latency uses the 0.5 s floor because the clip \
                         duration is not in the session; pass frames for the device value",
                        video.stimulus_id
                    ));
                }
            }
            (MetricSource::SessionReport, score)
        }
    };
    let flag = flag::assess(&score, &thresholds(&profile));

    Ok(RescoreResult {
        session_id: view.session_id,
        schema_version: view.schema_version,
        metric_source,
        score,
        flag,
        device: DeviceResult {
            composite_score: view.device_composite,
            attention_flag: view.device_flag,
        },
        warnings,
    })
}
