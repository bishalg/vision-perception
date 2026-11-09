//! owner: WP-R.6
//!
//! The exported operations as plain Rust (no wasm-bindgen types), so native tests call
//! exactly what the wasm exports call. Each function decodes, delegates to asd-core /
//! asd-contract and encodes; the only logic of its own is reading a stored session
//! ([`crate::session`]).
//!
//! WP-X.4 (RPX-4): the re-scores go through `asd_core::analysis::evaluate`, the one function
//! the backend job and the CLI use too, so a profile's `metrics` and
//! `lowConfidenceValidRatio` are honoured (before, the scorer's 0.60 constant was used
//! whatever the profile said); [`reprocess_session`] runs the same function at `frames` level
//! on a research frame log; [`param_registry`] serves `param-registry-v1.json`.

use asd_contract::catalog_checks;
use asd_contract::config::{
    ScoringProfile, CONFIG_BUNDLE_SCHEMA_ID, SCORING_PROFILE_SCHEMA_VERSION,
};
use asd_contract::profile_checks::{self, ProfileProblem};
use asd_contract::{params, session_v1_6, session_v1_7, session_v1_8};
use asd_core::analysis::{
    self, AnalysisProfile, Evaluation, IvtUnit, LevelRequest, ResearchInput, SessionInput,
};
use asd_core::display::{self, BandStatus, BandTarget, DisplayMetric};
use asd_core::gaze::raw_log::{self, RawLogError};
use asd_core::gaze::reprocess::ReprocessOptions;
use asd_core::metrics::AoiRole;
use asd_core::types::tracking::CalModelKind;
use asd_core::types::GazeQualityVerdict;
use asd_core::types::{
    AttentionFlag, AttentionFlagAssessment, GazeFrame, NormativeBaselineSet, PerVideoMetricSet,
    PerVideoSessionScore, StimulusCatalog,
};
use asd_core::{baselines, flag, metrics, quality, scoring};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::BoundaryError;
use crate::session;

pub(crate) fn decode<T: DeserializeOwned>(json: &str, what: &str) -> Result<T, BoundaryError> {
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
    pub session_schema_versions: [&'static str; 3],
    pub session_schema_ids: [&'static str; 3],
    /// `schemaVersion` of [`default_scoring_profile`].
    pub scoring_profile_schema_version: &'static str,
    pub config_bundle_schema_id: &'static str,
    /// The default low-confidence clip gate (`validSampleRatio` below it). Since WP-X.4 the
    /// re-scores use the profile's `lowConfidenceValidRatio`; this is its default.
    pub low_confidence_valid_ratio: f64,
}

pub fn contract_info() -> ContractInfo {
    ContractInfo {
        core_version: env!("CARGO_PKG_VERSION"),
        session_schema_versions: [
            session_v1_6::SCHEMA_VERSION,
            session_v1_7::SCHEMA_VERSION,
            session_v1_8::SCHEMA_VERSION,
        ],
        session_schema_ids: [
            session_v1_6::SCHEMA_ID,
            session_v1_7::SCHEMA_ID,
            session_v1_8::SCHEMA_ID,
        ],
        scoring_profile_schema_version: SCORING_PROFILE_SCHEMA_VERSION,
        config_bundle_schema_id: CONFIG_BUNDLE_SCHEMA_ID,
        low_confidence_valid_ratio: scoring::LOW_CONFIDENCE_VALID_RATIO,
    }
}

/// The bundled default `ScoringProfile` (the Swift constants, CON-5).
pub fn default_scoring_profile() -> ScoringProfile {
    ScoringProfile::default()
}

/// The parameter registry (PRM-1, RPX-4): exactly `param-registry-v1.json`
/// (`{$id, registryVersion, params}`).
pub fn param_registry() -> Value {
    params::registry_json()
}

// --- validation ---------------------------------------------------------------------------

/// Outcome of a document check: `valid` is `problems.is_empty()`. `fieldProblems` repeats
/// the problems that name a document member (`path`, camelCase dotted) for an editor to show
/// next to the field; the rest (shape errors, baseline coverage lines) are only in
/// `problems`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Validation {
    pub valid: bool,
    pub problems: Vec<String>,
    pub field_problems: Vec<ProfileProblem>,
}

impl Validation {
    fn from(problems: Vec<String>) -> Self {
        Self {
            valid: problems.is_empty(),
            problems,
            field_problems: Vec::new(),
        }
    }
}

/// A `ScoringProfile` check: the shape (it decodes as the contract type), then the range
/// checks the backend runs on create and approve (asd-contract `profile_checks`).
pub fn validate_scoring_profile(profile_json: &str) -> Validation {
    match decode::<ScoringProfile>(profile_json, "scoring profile") {
        Ok(profile) => {
            let field_problems = profile_checks::scoring_profile_problems(&profile);
            Validation {
                valid: field_problems.is_empty(),
                problems: field_problems.iter().map(|p| p.message.clone()).collect(),
                field_problems,
            }
        }
        Err(e) => Validation::from(vec![e.message().to_owned()]),
    }
}

/// A `StimulusCatalog` check: the shape (it decodes), then the value rules the backend runs
/// on create and approve (asd-contract `catalog_checks`), with the JSON path of each problem.
pub fn validate_catalog(catalog_json: &str) -> Validation {
    match decode::<StimulusCatalog>(catalog_json, "stimulus catalog") {
        Ok(catalog) => {
            let field_problems = catalog_checks::catalog_problems(&catalog);
            Validation {
                valid: field_problems.is_empty(),
                problems: field_problems.iter().map(|p| p.message.clone()).collect(),
                field_problems,
            }
        }
        Err(e) => Validation::from(vec![e.message().to_owned()]),
    }
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

/// I18N-4 (WP-L.0): the case id (`matches`, `no_scored_clips`, ...) of a sent flag headline,
/// so the console renders `flag_assessment.<case>.*` in the user's locale; `None` for a
/// headline this core does not know (shown as sent).
pub fn flag_case(headline: &str) -> Option<&'static str> {
    flag::flag_case_of_headline(headline).map(flag::FlagCase::id)
}

// --- display bands and session verdict (DSP-3, DSP-4, WP-X.0c) ----------------------------

/// The band of one value: `status` (`ok`, `monitor`, `outside`, `not_observed`) and the `ok`
/// target (`comparator` `at_least` | `at_most` | `below` | `between`, `low`, `high`, `unit`).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplayBandResult {
    pub status: BandStatus,
    #[serde(flatten)]
    pub target: BandTarget,
}

/// DSP-3: `asd_core::display::band` of `value` (`None`: not observed) for `metric`
/// (`socialPreferencePct`, ..., `validFramePct`) under a profile's `display` and
/// `sessionQuality` sections (absent: the defaults).
pub fn display_band(
    metric: &str,
    value: Option<f64>,
    profile_json: &str,
) -> Result<DisplayBandResult, BoundaryError> {
    let metric = DisplayMetric::parse(metric).ok_or_else(|| {
        BoundaryError::InvalidArgument(format!("unknown display metric `{metric}`"))
    })?;
    let profile: ScoringProfile = decode(profile_json, "scoring profile")?;
    let (status, target) = display::band(
        metric,
        value,
        &(&profile.display).into(),
        &(&profile.session_quality).into(),
    );
    Ok(DisplayBandResult { status, target })
}

/// DSP-4: the session `qualityVerdict` (`optimal`, `acceptable`, `degraded`) of a valid-frame
/// ratio under a profile's `sessionQuality` (`asd_core::quality::session_verdict`).
pub fn session_quality_verdict(
    valid_ratio: f64,
    interrupted: bool,
    profile_json: &str,
) -> Result<GazeQualityVerdict, BoundaryError> {
    let profile: ScoringProfile = decode(profile_json, "scoring profile")?;
    Ok(quality::session_verdict(
        valid_ratio,
        interrupted,
        &(&profile.session_quality).into(),
    ))
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
    /// Wire form (`1.6`, `1.7`, `1.8`).
    pub schema_version: String,
    pub metric_source: MetricSource,
    /// `scoring::score_session` with the given baselines.
    pub score: PerVideoSessionScore,
    /// `flag::assess` of `score` with the given profile's thresholds.
    pub flag: AttentionFlagAssessment,
    pub device: DeviceResult,
    pub warnings: Vec<String>,
}

/// Re-scores a stored session with a (draft) scoring profile and baseline set. Without
/// `frames_json` this is `analysis::evaluate` at `score` level: the metric sets are rebuilt
/// from the session's score rows (`metrics::from_reports`), scored with the profile's
/// `lowConfidenceValidRatio` and flagged with its `flag` thresholds. With `frames_json` (the
/// session's `[GazeFrame]`), the metric sets come from `metrics::extract_with` and the
/// profile's `metrics` section, then the same scoring and flag (the device path).
pub fn rescore_session(
    session_json: &str,
    profile_json: &str,
    baselines_json: &str,
    options_json: &str,
    frames_json: Option<&str>,
) -> Result<RescoreResult, BoundaryError> {
    let view = session::decode(session_json)?;
    let profile = decode_profile(profile_json)?;
    let baseline: NormativeBaselineSet = decode(baselines_json, "baseline set")?;
    let options: RescoreOptions = decode(options_json, "options")?;
    check_age(options.age_months)?;
    rescore_view(
        view,
        &profile,
        &baseline,
        options.age_months,
        &options.scored_at_utc,
        frames_json,
        Vec::new(),
    )
}

/// Options of [`rescore_api_session`]. Unknown keys are refused so a typo is not ignored.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApiRescoreOptions {
    /// The child's age at capture; the session does not carry it.
    pub age_months: i64,
    /// Written verbatim into `score.scoredAtUtc`; empty when absent.
    #[serde(default)]
    pub scored_at_utc: String,
    /// Valid-sample ratio for every clip (the API has no per-clip ratio); default: the
    /// session's `valid_frame_ratio`.
    #[serde(default)]
    pub valid_sample_ratio: Option<f64>,
}

/// Sandbox re-score of a session as the console reads it from `/v1`: the `SessionDetail`
/// (`GET /v1/sessions/{id}`) and its `[ClipScore]` (`GET /v1/sessions/{id}/clips`, the
/// `items`). The clips' metric rows are the device's score rows, so this is the
/// [`rescore_session`] path without frames ([`session::decode_api`] says what the API
/// form loses; each loss is a warning).
pub fn rescore_api_session(
    session_json: &str,
    clips_json: &str,
    profile_json: &str,
    baselines_json: &str,
    options_json: &str,
) -> Result<RescoreResult, BoundaryError> {
    let options: ApiRescoreOptions = decode(options_json, "options")?;
    check_age(options.age_months)?;
    if let Some(r) = options.valid_sample_ratio {
        if !(0.0..=1.0).contains(&r) {
            return Err(BoundaryError::InvalidArgument(
                "options.validSampleRatio must be in [0, 1]".into(),
            ));
        }
    }
    let (view, warnings) =
        session::decode_api(session_json, clips_json, options.valid_sample_ratio)?;
    let profile = decode_profile(profile_json)?;
    let baseline: NormativeBaselineSet = decode(baselines_json, "baseline set")?;
    rescore_view(
        view,
        &profile,
        &baseline,
        options.age_months,
        &options.scored_at_utc,
        None,
        warnings,
    )
}

fn check_age(age_months: i64) -> Result<(), BoundaryError> {
    if age_months < 0 {
        return Err(BoundaryError::InvalidArgument(
            "options.ageMonths must not be negative".into(),
        ));
    }
    Ok(())
}

/// A scoring profile: the contract shape first (so a malformed draft is refused exactly as
/// before), then the core's view of it (`AnalysisProfile`, the members the core reads).
pub(crate) fn decode_profile(profile_json: &str) -> Result<AnalysisProfile, BoundaryError> {
    decode::<ScoringProfile>(profile_json, "scoring profile")?;
    decode(profile_json, "scoring profile")
}

fn rescore_view(
    view: session::SessionView,
    profile: &AnalysisProfile,
    baseline: &NormativeBaselineSet,
    age_months: i64,
    scored_at_utc: &str,
    frames_json: Option<&str>,
    mut warnings: Vec<String>,
) -> Result<RescoreResult, BoundaryError> {
    let (metric_source, score, flag) = match frames_json {
        Some(json) => {
            let frames: Vec<GazeFrame> = decode(json, "frames")?;
            if frames.is_empty() {
                warnings.push("frames is empty, so no clip is scored".to_owned());
            }
            if profile.metrics.ivt_unit == IvtUnit::Deg {
                warnings.push("param_not_applied: metrics.ivtUnit".to_owned());
            }
            let metric_sets =
                metrics::extract_with(&frames, &AoiRole::builtin(), &profile.metrics, &profile.aoi);
            let score = scoring::score_session_with(
                &metric_sets,
                baseline,
                age_months,
                &frames,
                scored_at_utc,
                profile.low_confidence_valid_ratio,
            );
            let flag = flag::assess(&score, &profile.flag);
            (MetricSource::Frames, score, flag)
        }
        None => {
            let input = SessionInput {
                session_id: view.session_id.clone(),
                age_months: Some(age_months),
                reports: view.reports,
                has_reports: view.has_reports,
                scored_at_utc: scored_at_utc.to_owned(),
                ..SessionInput::default()
            };
            let evaluation = analysis::evaluate(&input, profile, baseline, LevelRequest::Score);
            warnings.extend(evaluation.warnings);
            let (Some(score), Some(flag)) = (evaluation.score, evaluation.flag) else {
                // Unreachable: the age is always given, so the evaluation always scores.
                return Err(BoundaryError::InvalidArgument(
                    "options.ageMonths is required".into(),
                ));
            };
            (MetricSource::SessionReport, score, flag)
        }
    };

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

// --- frame-level reprocessing (RPX-4) --------------------------------------------------------

/// Options of [`reprocess_session`]. Unknown keys are refused so a typo is not ignored.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReprocessSessionOptions {
    /// The child's age at capture.
    pub age_months: i64,
    /// Written verbatim into `score.scoredAtUtc`; empty when absent.
    #[serde(default)]
    pub scored_at_utc: String,
    /// Forces the calibration model (as `gaze replay --model`).
    #[serde(default)]
    pub model: Option<CalModelKind>,
    /// Operator answer at positioning (CNF-2).
    #[serde(default)]
    pub glasses: Option<bool>,
    /// Rate of the down-sampled plotting trace; no trace when absent or 0.
    #[serde(default)]
    pub trace_hz: Option<u32>,
}

/// RPX-4: re-processes a session's research frame log (`asd-raw-tracking/1` JSON lines) with
/// a catalog, a (draft) scoring profile and a baseline set: `analysis::evaluate` at `frames`
/// level, the same call `asd-cli research reprocess` makes. The result is the core
/// `Evaluation` (`level`, `score`, `flag`, `qualityStatus`, `validRatio`, `metrics`,
/// `tracking`, `fixations`, `trace` with `traceHz`, `warnings`). A log that parses but
/// cannot be replayed is `unscorable: reprocess_failed` (the error is in `warnings`);
/// `catalog_json` may be `null` when the log header carries every clip's AOIs.
pub fn reprocess_session(
    frame_log_jsonl: &str,
    catalog_json: &str,
    profile_json: &str,
    baselines_json: &str,
    options_json: &str,
) -> Result<Evaluation, BoundaryError> {
    let options: ReprocessSessionOptions = decode(options_json, "options")?;
    check_age(options.age_months)?;
    let log = raw_log::parse(frame_log_jsonl).map_err(|e| match e {
        RawLogError::Json { .. } => BoundaryError::invalid_json("frame log", e),
        other => BoundaryError::InvalidArgument(format!("frame log: {other}")),
    })?;
    let catalog: Option<StimulusCatalog> = decode(catalog_json, "stimulus catalog")?;
    let profile = decode_profile(profile_json)?;
    let baseline: NormativeBaselineSet = decode(baselines_json, "baseline set")?;
    let input = SessionInput {
        session_id: log.header.session_id.clone().unwrap_or_default(),
        age_months: Some(options.age_months),
        scored_at_utc: options.scored_at_utc,
        research: Some(ResearchInput {
            log,
            catalog: catalog.unwrap_or_else(empty_catalog),
            options: ReprocessOptions {
                model: options.model,
                glasses: options.glasses,
                trace_hz: options.trace_hz,
            },
        }),
        ..SessionInput::default()
    };
    Ok(analysis::evaluate(
        &input,
        &profile,
        &baseline,
        LevelRequest::Frames,
    ))
}

/// No catalog: every clip's AOIs must come from the log header.
fn empty_catalog() -> StimulusCatalog {
    StimulusCatalog {
        schema_version: String::new(),
        comment: None,
        languages: Vec::new(),
        fallback_language: String::new(),
        age_packs: Vec::new(),
        clips: Vec::new(),
    }
}
