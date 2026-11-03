//! owner: WP-X.0
//!
//! Researcher toolkit DTOs (ADR-0012; `docs/spec/modules/asd-research-toolkit/technical.md`):
//! experiments (EXP-*), cohorts (COH-*), analysis runs and their summaries (RUN-*, SWP-*,
//! PRV-1), the effective configuration (GOV-6). The backend routes (`routes/research.rs`,
//! `routes/analysis_runs.rs`), the console (`src/lib/api/research.ts`) and the analysis job
//! share these shapes through `api-v1.json`.
//!
//! Conventions as `api_v1`: snake_case, UUID strings, RFC 3339 UTC timestamps, hashes as
//! lowercase hex sha256. Distributions carry `null` numbers when small-cell suppression
//! (PRV-1) removed them; a suppressed cell is never a zero.

use std::collections::BTreeMap;

use asd_core::types::tracking::TrackingVerdict;
use asd_core::types::GazeQualityVerdict;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::api_v1::{ConfigDocumentKind, JobProgress, ScoringStatus, SessionAttentionFlag};
use crate::config::ScoringProfile;
use crate::mirror;

// ---- limits (owner defaults, ADR-0012 open questions) ---------------------------------------

/// PRV-1: default small-cell threshold `k` (ADR-0012 Q2: 12, the norm min-n).
pub const DEFAULT_SMALL_CELL_K: u32 = 12;
/// PRV-1: allowed range of `organizations.settings.research.small_cell_k`.
pub const MIN_SMALL_CELL_K: u32 = 5;
pub const MAX_SMALL_CELL_K: u32 = 50;
/// `p10`/`p90` are reported only from this many observations (PRV-1).
pub const MIN_N_FOR_DECILES: u32 = 20;
/// EXP-1 / COH-2: name length in characters.
pub const MAX_NAME_CHARS: usize = 80;
/// EXP-1: description length in characters.
pub const MAX_DESCRIPTION_CHARS: usize = 500;
/// SWP-1: at most this many axes and steps per axis.
pub const MAX_SWEEP_AXES: usize = 2;
pub const MAX_SWEEP_STEPS: u32 = 24;
/// SWP-2: at most this many cells per sweep.
pub const MAX_SWEEP_CELLS: u32 = 625;
/// RFL-1/RFL-3: largest research frame log accepted, bytes (16 MiB).
pub const MAX_RESEARCH_LOG_BYTES: usize = 16 * 1024 * 1024;
/// RUN-3: largest sealed per-session result blob, bytes (64 MiB).
pub const MAX_RESULT_BYTES: usize = 64 * 1024 * 1024;

/// `organizations.settings` keys owned by the toolkit: `{"research": {"small_cell_k": 12}}`.
pub mod settings {
    pub const RESEARCH: &str = "research";
    pub const SMALL_CELL_K: &str = "small_cell_k";
}

// ---- experiments (EXP-1, EXP-2, GOV-2) -------------------------------------------------------

/// `experiments.status`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExperimentStatus {
    Draft,
    Archived,
    Promoted,
}

/// A named draft scoring profile of an org (EXP-1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Experiment {
    pub id: String,
    pub org_id: String,
    /// 1..80 characters, unique in the org.
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub status: ExperimentStatus,
    /// The published profile version the experiment started from.
    pub base_profile_version: String,
    /// The full draft profile (schema 1.2.0).
    pub profile: ScoringProfile,
    /// sha256 of the RFC 8785 form of `profile`, hex.
    pub profile_hash: String,
    /// Baselines version (published or draft) the experiment scores against; `null` = the
    /// published one.
    #[serde(default)]
    pub baselines_version: Option<String>,
    /// The draft config document created by promote (GOV-2).
    #[serde(default)]
    pub promoted_config_version: Option<String>,
    #[serde(default)]
    pub last_run_id: Option<String>,
    /// Optimistic concurrency: send `If-Match: "<row_version>"` with `PATCH`.
    pub row_version: i32,
    #[serde(default)]
    pub created_by: Option<String>,
    #[schemars(extend("format" = "date-time"))]
    pub created_at: String,
    #[schemars(extend("format" = "date-time"))]
    pub updated_at: String,
}

/// `POST /v1/orgs/{id}/experiments` (`ExperimentWrite`). Starts from the published profile
/// unless `from_experiment_id` or `from_version` names another (at most one of them).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateExperimentRequest {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_experiment_id: Option<String>,
    /// A `scoring_profile` config document version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_version: Option<String>,
}

/// `PATCH /v1/experiments/{id}` (`ExperimentWrite`, `If-Match`). `profile` is a whole
/// `ScoringProfile` document, validated by the parameter registry (PRM-3; `400
/// config-invalid` with `field_problems`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateExperimentRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<ScoringProfile>")]
    pub profile: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baselines_version: Option<String>,
}

/// `POST /v1/experiments/{id}/promote` (`ExperimentPromote`, GOV-2): creates a **draft**
/// config document of `kind` (`scoring_profile` or `normative_baselines`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PromoteExperimentRequest {
    pub kind: ConfigDocumentKind,
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

// ---- cohorts (COH-1..3) ---------------------------------------------------------------------

/// Sex of the participant as a session may one day carry it (ADR-0012 Q8: session schema 1.9).
/// No session carries it yet, so a non-empty `CohortDefinition.sex` selects nothing extra and
/// is reported, never fabricated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Sex {
    Female,
    Male,
    Other,
    Unknown,
}

/// Which sessions a run reads (COH-1). Research consent, "not withdrawn" and
/// `capture_source = truedepth` are always applied and cannot be expressed here. Every list
/// empty = no restriction. `cohort_hash = sha256(JCS(definition))`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CohortDefinition {
    /// Band ids of the experiment profile's `ageBands`.
    #[serde(default)]
    pub age_bands: Vec<String>,
    /// Applied only once sessions carry sex (ADR-0012 Q8).
    #[serde(default)]
    pub sex: Vec<Sex>,
    /// Device ids ("site").
    #[serde(default)]
    pub device_ids: Vec<String>,
    /// `received_at` lower bound, `YYYY-MM-DD` (start of day, UTC).
    #[serde(default)]
    pub from: Option<String>,
    /// `received_at` upper bound, `YYYY-MM-DD` (end of day, UTC).
    #[serde(default)]
    pub to: Option<String>,
    #[serde(default)]
    #[schemars(with = "Vec<mirror::GazeQualityVerdict>")]
    pub quality_verdicts: Vec<GazeQualityVerdict>,
    #[serde(default)]
    #[schemars(with = "Vec<mirror::TrackingVerdict>")]
    pub tracking_verdicts: Vec<TrackingVerdict>,
    #[serde(default)]
    pub min_valid_ratio: Option<f64>,
    /// Empty = `scored` only.
    #[serde(default)]
    pub scoring_status: Vec<ScoringStatus>,
    /// Only sessions with a research frame log.
    #[serde(default)]
    pub require_frames: bool,
    /// Synthetic sessions too; honoured only with `ASD_ALLOW_SYNTHETIC`, flagged in results.
    #[serde(default)]
    pub include_synthetic: bool,
}

/// A saved cohort (COH-2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Cohort {
    pub id: String,
    pub org_id: String,
    pub name: String,
    pub definition: CohortDefinition,
    /// sha256 of the RFC 8785 form of `definition`, hex.
    pub definition_hash: String,
    #[serde(default)]
    pub created_by: Option<String>,
    #[schemars(extend("format" = "date-time"))]
    pub created_at: String,
    #[schemars(extend("format" = "date-time"))]
    pub updated_at: String,
    #[serde(default)]
    pub archived_at: Option<String>,
}

/// `POST /v1/orgs/{id}/cohorts` (`ExperimentWrite`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateCohortRequest {
    pub name: String,
    pub definition: CohortDefinition,
}

/// `PATCH /v1/cohorts/{id}` (`ExperimentWrite`); `archived: true` archives it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateCohortRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub definition: Option<CohortDefinition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archived: Option<bool>,
}

/// One age band of a cohort preview; counts are `null` when suppressed (PRV-1, AGE-3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CohortBandCount {
    pub band: String,
    pub n: Option<u32>,
    /// Sessions of the band with a research frame log.
    pub frames_n: Option<u32>,
    pub suppressed: bool,
}

/// Sessions by the level a run would evaluate them at (RPX-3).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LevelCounts {
    pub frames: u32,
    pub score: u32,
}

/// Sessions left out of a cohort, by reason (COH-2).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CohortExclusions {
    pub no_consent: u32,
    pub withdrawn: u32,
    pub unscorable: u32,
    pub no_age: u32,
    pub quality: u32,
}

/// `POST /v1/orgs/{id}/cohorts/preview` (`AnalysisRead`; body: a [`CohortDefinition`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CohortPreview {
    /// `null` when the whole cohort is below `small_cell_k`.
    pub n: Option<u32>,
    pub by_age_band: Vec<CohortBandCount>,
    pub by_level: LevelCounts,
    pub excluded: CohortExclusions,
    /// The org's suppression threshold applied to this answer (PRV-1).
    pub small_cell_k: u32,
}

// ---- analysis runs (RUN-1..9, SWP-1..3) -------------------------------------------------------

/// `analysis_runs.kind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RunKind {
    CohortRescore,
    ParamSweep,
    AgeProfile,
}

/// `analysis_runs.status` (the job's lifecycle).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

/// Requested evaluation level (RPX-3): `auto` = frames where a research log exists.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LevelRequest {
    #[default]
    Auto,
    Score,
    Frames,
}

/// The level a session was actually evaluated at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    Score,
    Frames,
}

fn default_true() -> bool {
    true
}

/// RUN-2 options.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RunOptions {
    /// Metric keys to summarise; empty = the 15 metric keys and the composite.
    #[serde(default)]
    pub metrics: Vec<String>,
    /// Bootstrap 95 % CIs of means (default `true`).
    #[serde(default = "default_true")]
    pub bootstrap: bool,
}

impl Default for RunOptions {
    fn default() -> Self {
        Self {
            metrics: Vec::new(),
            bootstrap: true,
        }
    }
}

/// One sweep axis (SWP-1): a Number or Integer registry id, `from <= to`, at most
/// [`MAX_SWEEP_STEPS`] steps, values inside the registry range.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SweepAxis {
    pub param_id: String,
    pub from: f64,
    pub to: f64,
    pub step: f64,
}

/// A parameter sweep (SWP-1, phase B): 1..=2 axes over the experiment profile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SweepSpec {
    pub axes: Vec<SweepAxis>,
}

/// `POST /v1/orgs/{id}/analysis-runs` (`AnalysisRun`; RUN-2). Exactly one of `cohort_id`
/// and `cohort`. Answers `202 AnalysisRun`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AnalysisRunRequest {
    pub kind: RunKind,
    pub experiment_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cohort_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cohort: Option<CohortDefinition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sweep: Option<SweepSpec>,
    #[serde(default)]
    pub level: LevelRequest,
    /// Bootstrap seed; the server draws one when absent and stores it (RUN-5).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<i64>,
    #[serde(default)]
    pub options: RunOptions,
}

/// The snapshot a run is computed from (`analysis_runs.definition`, RUN-1);
/// `definition_hash = sha256(JCS(definition))`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AnalysisDefinition {
    pub kind: RunKind,
    pub cohort: CohortDefinition,
    /// The experiment profile at enqueue time.
    pub profile: ScoringProfile,
    #[serde(default)]
    pub baselines_version: Option<String>,
    #[serde(default)]
    pub sweep: Option<SweepSpec>,
    pub level: LevelRequest,
    pub options: RunOptions,
    pub seed: i64,
}

/// Histogram of a distribution: `counts[i]` values in `[edges[i], edges[i + 1])`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Hist {
    pub edges: Vec<f64>,
    pub counts: Vec<u32>,
}

/// A distribution summary (RUN-4). With `suppressed: true` every other field is `null`
/// (PRV-1); `p10`/`p90` are `null` below [`MIN_N_FOR_DECILES`].
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
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
    /// Bootstrap 95 % CI of the mean (seeded, RUN-5).
    pub ci95_mean: Option<[f64; 2]>,
    pub hist: Option<Hist>,
}

/// Published, draft and draft-minus-published distributions of one quantity.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DistComparison {
    pub published: Dist,
    pub draft: Dist,
    pub delta: Dist,
}

/// Flag counts; `null` when suppressed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FlagCounts {
    pub green: Option<u32>,
    pub yellow: Option<u32>,
    pub red: Option<u32>,
    pub unreliable: Option<u32>,
}

/// Flag shares in `[0, 1]`; the whole object is `null` for a suppressed band.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FlagRates {
    pub green: f64,
    pub yellow: f64,
    pub red: f64,
    pub unreliable: f64,
}

/// One cell of a transition matrix: `[from, to, n]`; `n` is `null` when `0 < n < k`.
pub type Transition = (String, String, Option<u32>);

/// RUN-4 `flags`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FlagSummary {
    pub published: FlagCounts,
    pub draft: FlagCounts,
    pub transitions: Vec<Transition>,
}

/// RUN-4 `quality`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct QualitySummary {
    pub transitions: Vec<Transition>,
}

/// RUN-4 `counts`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RunCounts {
    pub sessions: u32,
    pub by_level: LevelCounts,
    /// Reason -> sessions left out (`no_frame_log`, `no_age`, ...).
    pub excluded: BTreeMap<String, u32>,
    /// Registry id -> sessions where the knob could not be applied at their level (RPX-3).
    pub params_not_applied: BTreeMap<String, u32>,
    /// Transition cells suppressed by PRV-1.
    #[serde(default)]
    pub suppressed_cells: u32,
}

/// Flag rates of one band, published and draft.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FlagRateComparison {
    pub published: Option<FlagRates>,
    pub draft: Option<FlagRates>,
}

/// RUN-4 per-band (and `overall`) block.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BandSummary {
    /// Band id, or `overall`.
    pub band: String,
    pub n: Option<u32>,
    #[serde(default)]
    pub suppressed: bool,
    /// Metric key -> distributions.
    pub metrics: BTreeMap<String, DistComparison>,
    pub composite: DistComparison,
    pub flag_rate: FlagRateComparison,
}

/// Mean and median of one quantity in a sweep cell.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MeanMedian {
    pub mean: Option<f64>,
    pub median: Option<f64>,
}

/// One grid cell of a sweep (SWP-3).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SweepCell {
    /// One value per axis.
    pub values: Vec<f64>,
    pub n: Option<u32>,
    pub flag_rate: Option<FlagRates>,
    /// Quality status -> share.
    pub quality_rate: BTreeMap<String, Option<f64>>,
    pub metrics: BTreeMap<String, MeanMedian>,
    pub composite: MeanMedian,
    /// Sessions whose flag differs from the published profile's.
    pub changed_vs_published: Option<u32>,
}

/// SWP-3 `summary.sweep`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SweepSummary {
    pub axes: Vec<SweepAxis>,
    pub cells: Vec<SweepCell>,
}

/// `analysis_runs.summary` (RUN-4, ≤ 1 MiB), PRV-1 applied before it is stored.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RunSummary {
    pub counts: RunCounts,
    pub flags: FlagSummary,
    pub quality: QualitySummary,
    pub by_age_band: Vec<BandSummary>,
    pub overall: BandSummary,
    /// `param_sweep` runs only (SWP-3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sweep: Option<SweepSummary>,
    /// `age_profile` runs only (AGE-1, phase B: shape owned by WP-X.9).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub age_profile: Option<Value>,
    /// The small-cell threshold the summary was suppressed with.
    pub small_cell_k: u32,
    /// Synthetic sessions were included (`include_synthetic`, COH-1).
    #[serde(default)]
    pub includes_synthetic: bool,
}

/// Sessions added and removed between a run and its re-run (RUN-2, RUN-6).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MembershipDrift {
    pub added: u32,
    pub removed: u32,
}

/// `GET /v1/analysis-runs/{id}` (`AnalysisRead`): every RUN-1 column but the result blob
/// key. The reproducibility fields (RUN-5) are set before the job starts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AnalysisRun {
    pub id: String,
    pub org_id: String,
    pub kind: RunKind,
    pub status: RunStatus,
    #[serde(default)]
    pub experiment_id: Option<String>,
    #[serde(default)]
    pub cohort_id: Option<String>,
    pub definition: AnalysisDefinition,
    pub definition_hash: String,
    pub cohort_hash: String,
    #[serde(default)]
    pub membership_hash: Option<String>,
    pub profile_hash: String,
    pub published_profile_version: String,
    pub baselines_version: String,
    pub catalog_version: String,
    #[serde(default)]
    pub bundle_version: Option<String>,
    pub core_version: String,
    pub contract_version: String,
    pub pipeline_version: String,
    pub registry_version: String,
    pub seed: i64,
    pub level_requested: LevelRequest,
    #[serde(default)]
    pub sessions_n: Option<u32>,
    #[serde(default)]
    pub sessions_frames_n: Option<u32>,
    #[serde(default)]
    pub sessions_score_n: Option<u32>,
    #[serde(default)]
    pub excluded: Option<BTreeMap<String, u32>>,
    #[serde(default)]
    pub summary: Option<RunSummary>,
    #[serde(default)]
    pub result_bytes: Option<u64>,
    #[serde(default)]
    pub progress: Option<JobProgress>,
    #[serde(default)]
    pub job_id: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
    /// Set on a re-run once it finished (RUN-2).
    #[serde(default)]
    pub membership_drift: Option<MembershipDrift>,
    #[serde(default)]
    pub created_by: Option<String>,
    #[schemars(extend("format" = "date-time"))]
    pub created_at: String,
    #[serde(default)]
    pub started_at: Option<String>,
    #[serde(default)]
    pub finished_at: Option<String>,
}

/// One side (published or draft) of a per-session result row (RUN-3).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SessionSide {
    pub composite: Option<f64>,
    pub flag: Option<SessionAttentionFlag>,
    pub quality_status: Option<String>,
    /// Metric key -> value.
    pub metrics: BTreeMap<String, Option<f64>>,
}

/// Draft minus published of a per-session row.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SessionDelta {
    pub composite: Option<f64>,
    pub metrics: BTreeMap<String, Option<f64>>,
}

/// `GET /v1/analysis-runs/{id}/sessions` item (RUN-3, PRV-2): no token, MRN, name or device
/// model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AnalysisSessionRow {
    pub session_id: String,
    pub level: Level,
    pub age_months: Option<i32>,
    pub age_band: Option<String>,
    pub device_id: Option<String>,
    pub published: SessionSide,
    pub draft: SessionSide,
    pub delta: SessionDelta,
    #[serde(default)]
    pub warnings: Vec<String>,
}

macro_rules! page {
    ($(#[$doc:meta])* $name:ident, $item:ty) => {
        $(#[$doc])*
        /// Cursor-paginated list envelope: `cursor` is opaque, `limit <= 100`.
        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
        pub struct $name {
            pub items: Vec<$item>,
            pub next_cursor: Option<String>,
        }
    };
}

page!(
    /// `GET /v1/orgs/{id}/experiments`.
    ExperimentPage,
    Experiment
);
page!(
    /// `GET /v1/orgs/{id}/cohorts`.
    CohortPage,
    Cohort
);
page!(
    /// `GET /v1/orgs/{id}/analysis-runs`.
    AnalysisRunPage,
    AnalysisRun
);
page!(
    /// `GET /v1/analysis-runs/{id}/sessions`.
    AnalysisSessionPage,
    AnalysisSessionRow
);

// ---- effective configuration (GOV-6) ----------------------------------------------------------

/// One published document of the effective configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EffectiveDocument {
    pub version: String,
    /// The `ScoringProfile`, `NormativeBaselineSet` or `StimulusCatalog` body.
    pub body: Value,
}

/// Where the effective configuration came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EffectiveSource {
    /// The org's own signed bundle.
    OrgBundle,
    /// The global published documents.
    Global,
}

/// `GET /v1/orgs/{id}/config/effective` (`ConfigRead`, GOV-6): what the org's devices and
/// the console render from (INV-2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EffectiveConfig {
    pub source: EffectiveSource,
    #[serde(default)]
    pub bundle_version: Option<String>,
    pub scoring_profile: EffectiveDocument,
    pub normative_baselines: EffectiveDocument,
    pub stimulus_catalog: EffectiveDocument,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_cell_defaults_are_the_owner_defaults() {
        assert_eq!(DEFAULT_SMALL_CELL_K, 12);
        assert!((MIN_SMALL_CELL_K..=MAX_SMALL_CELL_K).contains(&DEFAULT_SMALL_CELL_K));
    }
}
