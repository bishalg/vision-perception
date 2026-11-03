//! owner: WP-X.3
//!
//! Analysis runs (`docs/spec/modules/asd-research-toolkit/technical.md` RUN-1, RUN-2, RUN-5,
//! RUN-6, RUN-8, RUN-9, SWP-2, GOV-4):
//!
//! - `POST /v1/orgs/{id}/analysis-runs` (`AnalysisRun`): validates the request (the
//!   experiment profile against the parameter registry, PRM-3; the cohort; the options),
//!   resolves the membership once to check the budget (`400 cohort-empty`,
//!   `cohort-too-large`, `sweep-too-large`, `analysis-level-unavailable`), checks the org's
//!   queue (`429 analysis-quota-exceeded`), snapshots the definition and every RUN-5 version,
//!   inserts the run, links it as the experiment's `last_run_id` and enqueues job kind
//!   `analysis_run` in one transaction; audit `analysis.run_enqueue`; `202 AnalysisRun`.
//! - `GET /v1/orgs/{id}/analysis-runs?kind&status&experiment_id&cursor&limit`
//!   (`AnalysisRead`): newest first, without `summary` (the detail has it).
//! - `GET /v1/analysis-runs/{id}` (`AnalysisRead`): every RUN-1 column but the blob key,
//!   the job's live `progress`, `membership_drift` of a finished re-run; audits
//!   `analysis.read` once per call before the body is built.
//! - `GET /v1/analysis-runs/{id}/sessions?cursor&limit` (`AnalysisRead` and `SessionRead`):
//!   rows of the sealed result blob; audits `analysis.sessions_read` once per page.
//! - `POST /v1/analysis-runs/{id}/cancel` (`AnalysisRun`): a queued run ends `cancelled` at
//!   once; a running one gets `jobs.cancel_requested_at` and stops before its next page with
//!   nothing stored; a finished one is `409 job-finished`. Audit `analysis.run_cancel`.
//! - `POST /v1/analysis-runs/{id}/rerun` (`AnalysisRun`): a new run from the stored
//!   definition (same profile snapshot, cohort, level, options and seed; the published
//!   versions of today), with the same checks; the job reports `membership_drift {added,
//!   removed}` against the old run's membership. Audit `analysis.run_enqueue` with
//!   `rerun_of`.
//!
//! A run of another org is a 404. The vendor (`superadmin`) reads runs but never runs one
//! (GOV-1). `param_sweep` and `age_profile` runs are phase B (WP-X.9): their sweep budget
//! (SWP-1, SWP-2) is checked here, then they are refused as not yet available.

use std::collections::BTreeSet;

use asd_contract::api_v1::{ConfigDocumentKind, JobProgress};
use asd_contract::config::ScoringProfile;
use asd_contract::params::{self, DocumentKind, ParamKind};
use asd_contract::problem as slugs;
use asd_contract::research::{
    AnalysisDefinition, AnalysisRun, AnalysisRunPage, AnalysisRunRequest, AnalysisSessionPage,
    AnalysisSessionRow, CohortDefinition, LevelRequest, MembershipDrift, RunKind, RunOptions,
    RunStatus, RunSummary, SweepSpec, MAX_SWEEP_AXES, MAX_SWEEP_CELLS, MAX_SWEEP_STEPS,
};
use asd_core::analysis::AnalysisProfile;
use asd_core::metrics::PerVideoMetricKey;
use axum::extract::rejection::QueryRejection;
use axum::extract::{Path, Query, State};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use http::StatusCode;
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::PgConnection;
use uuid::Uuid;

use crate::app::AppState;
use crate::audit::{record_read, AuditEvent};
use crate::auth::extractors::{ApiJson, RequestMeta, Require, UserPrincipal};
use crate::auth::rbac::{self, perm, Action};
use crate::config_docs::lifecycle::body_hash;
use crate::error::AppError;
use crate::jobs::analysis::{self as job, check_budget, core_level, open_result};
use crate::jobs::{self, KIND_ANALYSIS};
use crate::problem::{InvalidParam, Problem};
use crate::read::cursor::{self, Cursor};
use crate::routes::sessions::query_or_problem;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/orgs/{id}/analysis-runs", post(create_run).get(list_runs))
        .route("/analysis-runs/{id}", get(get_run))
        .route("/analysis-runs/{id}/sessions", get(run_sessions))
        .route("/analysis-runs/{id}/cancel", post(cancel_run))
        .route("/analysis-runs/{id}/rerun", post(rerun))
}

// ---- errors ------------------------------------------------------------------------------

/// Errors of these routes: every [`AppError`], plus problems `AppError` has no variant for
/// (`429 analysis-quota-exceeded`, `400 config-invalid` with `field_problems`).
#[derive(Debug)]
pub enum RunError {
    App(AppError),
    Problem(Box<Problem>),
}

impl From<AppError> for RunError {
    fn from(e: AppError) -> Self {
        Self::App(e)
    }
}

impl From<sqlx::Error> for RunError {
    fn from(e: sqlx::Error) -> Self {
        Self::App(AppError::Database(e))
    }
}

impl IntoResponse for RunError {
    fn into_response(self) -> Response {
        match self {
            Self::App(e) => e.into_response(),
            Self::Problem(p) => p.into_response(),
        }
    }
}

type RResult<T> = Result<T, RunError>;

fn invalid(name: &str, reason: impl Into<String>) -> InvalidParam {
    InvalidParam {
        name: name.into(),
        reason: reason.into(),
    }
}

fn validation(errors: Vec<InvalidParam>) -> RunError {
    RunError::App(AppError::Validation(errors))
}

fn bad_request(slug: &'static str, detail: impl Into<String>) -> RunError {
    RunError::App(AppError::BadRequest {
        slug,
        detail: detail.into(),
    })
}

fn path_uuid(raw: &str) -> RResult<Uuid> {
    Uuid::parse_str(raw).map_err(|_| RunError::App(AppError::NotFound))
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

fn wire<T: serde::Serialize>(v: &T) -> String {
    match serde_json::to_value(v) {
        Ok(Value::String(s)) => s,
        _ => String::new(),
    }
}

fn decode<T: serde::de::DeserializeOwned>(what: &str, v: Value) -> RResult<T> {
    serde_json::from_value(v)
        .map_err(|e| RunError::App(AppError::Internal(format!("stored {what}: {e}"))))
}

// ---- rows ----------------------------------------------------------------------------------

/// An `analysis_runs` row with its job's progress and result drift.
#[derive(Debug, Clone)]
struct RunRow {
    id: Uuid,
    org_id: Uuid,
    kind: String,
    status: String,
    experiment_id: Option<Uuid>,
    cohort_id: Option<Uuid>,
    definition: Value,
    definition_hash: Vec<u8>,
    cohort_hash: Vec<u8>,
    membership_hash: Option<Vec<u8>>,
    profile_hash: Vec<u8>,
    published_profile_version: String,
    baselines_version: String,
    catalog_version: String,
    bundle_version: Option<String>,
    core_version: String,
    contract_version: String,
    pipeline_version: String,
    registry_version: String,
    seed: i64,
    level_requested: String,
    sessions_n: Option<i32>,
    sessions_frames_n: Option<i32>,
    sessions_score_n: Option<i32>,
    excluded: Option<Value>,
    summary: Option<Value>,
    result_blob_key: Option<String>,
    result_bytes: Option<i32>,
    progress: Option<Value>,
    job_id: Option<Uuid>,
    error: Option<String>,
    created_by: Option<Uuid>,
    created_at: DateTime<Utc>,
    started_at: Option<DateTime<Utc>>,
    finished_at: Option<DateTime<Utc>>,
    job_progress: Option<Value>,
    drift: Option<Value>,
}

impl RunRow {
    fn dto(&self, with_summary: bool) -> RResult<AnalysisRun> {
        let progress = self
            .job_progress
            .clone()
            .or_else(|| self.progress.clone())
            .and_then(|p| serde_json::from_value::<JobProgress>(p).ok());
        let n = |v: Option<i32>| v.and_then(|n| u32::try_from(n).ok());
        Ok(AnalysisRun {
            id: self.id.to_string(),
            org_id: self.org_id.to_string(),
            kind: decode("run kind", Value::from(self.kind.as_str()))?,
            status: decode("run status", Value::from(self.status.as_str()))?,
            experiment_id: self.experiment_id.map(|u| u.to_string()),
            cohort_id: self.cohort_id.map(|u| u.to_string()),
            definition: decode("run definition", self.definition.clone())?,
            definition_hash: hex(&self.definition_hash),
            cohort_hash: hex(&self.cohort_hash),
            membership_hash: self.membership_hash.as_deref().map(hex),
            profile_hash: hex(&self.profile_hash),
            published_profile_version: self.published_profile_version.clone(),
            baselines_version: self.baselines_version.clone(),
            catalog_version: self.catalog_version.clone(),
            bundle_version: self.bundle_version.clone(),
            core_version: self.core_version.clone(),
            contract_version: self.contract_version.clone(),
            pipeline_version: self.pipeline_version.clone(),
            registry_version: self.registry_version.clone(),
            seed: self.seed,
            level_requested: decode("run level", Value::from(self.level_requested.as_str()))?,
            sessions_n: n(self.sessions_n),
            sessions_frames_n: n(self.sessions_frames_n),
            sessions_score_n: n(self.sessions_score_n),
            excluded: match &self.excluded {
                Some(v) => Some(decode("run exclusions", v.clone())?),
                None => None,
            },
            summary: match (&self.summary, with_summary) {
                (Some(v), true) => Some(decode::<RunSummary>("run summary", v.clone())?),
                _ => None,
            },
            result_bytes: self.result_bytes.and_then(|b| u64::try_from(b).ok()),
            progress,
            job_id: self.job_id.map(|u| u.to_string()),
            error: self.error.clone(),
            membership_drift: self
                .drift
                .clone()
                .and_then(|d| serde_json::from_value::<MembershipDrift>(d).ok()),
            created_by: self.created_by.map(|u| u.to_string()),
            created_at: cursor::ts(self.created_at),
            started_at: self.started_at.map(cursor::ts),
            finished_at: self.finished_at.map(cursor::ts),
        })
    }
}

async fn load_run(conn: &mut PgConnection, id: Uuid) -> RResult<RunRow> {
    sqlx::query_as!(
        RunRow,
        r#"SELECT r.id, r.org_id, r.kind, r.status, r.experiment_id, r.cohort_id, r.definition,
                  r.definition_hash, r.cohort_hash, r.membership_hash, r.profile_hash,
                  r.published_profile_version, r.baselines_version, r.catalog_version,
                  r.bundle_version, r.core_version, r.contract_version, r.pipeline_version,
                  r.registry_version, r.seed, r.level_requested, r.sessions_n,
                  r.sessions_frames_n, r.sessions_score_n, r.excluded, r.summary,
                  r.result_blob_key, r.result_bytes, r.progress, r.job_id, r.error,
                  r.created_by, r.created_at, r.started_at, r.finished_at,
                  j.progress AS "job_progress?", j.result -> 'membership_drift' AS "drift?"
           FROM analysis_runs r LEFT JOIN jobs j ON j.id = r.job_id
           WHERE r.id = $1"#,
        id
    )
    .fetch_optional(&mut *conn)
    .await?
    .ok_or(RunError::App(AppError::NotFound))
}

// ---- create ----------------------------------------------------------------------------------

/// The experiment columns a run snapshots.
struct ExperimentRow {
    org_id: Uuid,
    profile: Value,
    baselines_version: Option<String>,
}

/// What a new run is built from (a request or a stored definition).
struct Plan {
    experiment_id: Option<Uuid>,
    cohort_id: Option<Uuid>,
    definition: AnalysisDefinition,
    /// The experiment profile body as stored (GOV-3: `profile_hash = body_hash(body)`).
    profile_body: Value,
    rerun_of: Option<Uuid>,
}

/// SWP-1 / SWP-2 shape and cell count of a sweep; `None` for no sweep (one cell).
fn sweep_cells(sweep: Option<&SweepSpec>) -> RResult<u64> {
    let Some(sweep) = sweep else { return Ok(1) };
    if sweep.axes.is_empty() || sweep.axes.len() > MAX_SWEEP_AXES {
        return Err(validation(vec![invalid(
            "sweep.axes",
            format!("must have 1 to {MAX_SWEEP_AXES} axes"),
        )]));
    }
    let mut cells: u64 = 1;
    for (i, axis) in sweep.axes.iter().enumerate() {
        let name = format!("sweep.axes[{i}]");
        let spec = params::spec(&axis.param_id).ok_or_else(|| {
            validation(vec![invalid(
                &format!("{name}.param_id"),
                "not a parameter registry id",
            )])
        })?;
        let (min, max) = match &spec.kind {
            ParamKind::Number { min, max, .. } => (*min, *max),
            ParamKind::Integer { min, max, .. } => (*min as f64, *max as f64),
            _ => {
                return Err(validation(vec![invalid(
                    &format!("{name}.param_id"),
                    "must be a number or integer parameter",
                )]))
            }
        };
        let finite = axis.from.is_finite() && axis.to.is_finite() && axis.step.is_finite();
        if !finite || axis.step <= 0.0 || axis.from > axis.to {
            return Err(validation(vec![invalid(
                &name,
                "needs from <= to and a positive step",
            )]));
        }
        if axis.from < min || axis.to > max {
            return Err(validation(vec![invalid(
                &name,
                format!("values must be inside [{min}, {max}]"),
            )]));
        }
        let steps = ((axis.to - axis.from) / axis.step + 1e-9).floor();
        if steps > f64::from(MAX_SWEEP_STEPS) {
            return Err(bad_request(
                slugs::SWEEP_TOO_LARGE,
                format!("{name} has {steps} steps; at most {MAX_SWEEP_STEPS} per axis"),
            ));
        }
        cells = cells.saturating_mul(steps as u64 + 1);
    }
    if cells > u64::from(MAX_SWEEP_CELLS) {
        return Err(bad_request(
            slugs::SWEEP_TOO_LARGE,
            format!("{cells} cells; a sweep has at most {MAX_SWEEP_CELLS}"),
        ));
    }
    Ok(cells)
}

fn check_options(options: &RunOptions) -> RResult<()> {
    let known: BTreeSet<&str> = PerVideoMetricKey::ALL.iter().map(|k| k.as_str()).collect();
    let errors: Vec<InvalidParam> = options
        .metrics
        .iter()
        .enumerate()
        .filter(|(_, m)| !known.contains(m.as_str()))
        .map(|(i, _)| invalid(&format!("options.metrics[{i}]"), "not a metric key"))
        .collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(validation(errors))
    }
}

/// `400 config-invalid` with `field_problems` (PRM-3).
fn profile_problems(body: &Value) -> RResult<()> {
    let problems = params::validate(DocumentKind::ScoringProfile, body);
    if problems.is_empty() {
        return Ok(());
    }
    let field_problems: Vec<Value> = problems
        .iter()
        .map(|p| json!({"path": p.path, "message": p.message}))
        .collect();
    Err(RunError::Problem(Box::new(
        Problem::new(StatusCode::BAD_REQUEST, slugs::CONFIG_INVALID)
            .with_detail("the experiment profile fails the parameter registry checks")
            .with_extension("field_problems", Value::Array(field_problems)),
    )))
}

async fn load_experiment(conn: &mut PgConnection, id: Uuid, org: Uuid) -> RResult<ExperimentRow> {
    sqlx::query_as!(
        ExperimentRow,
        "SELECT org_id, profile, baselines_version FROM experiments WHERE id = $1",
        id
    )
    .fetch_optional(&mut *conn)
    .await?
    .filter(|e| e.org_id == org)
    .ok_or_else(|| {
        validation(vec![invalid(
            "experiment_id",
            "no such experiment in this org",
        )])
    })
}

/// The plan of `POST /orgs/{id}/analysis-runs`.
async fn plan_request(
    conn: &mut PgConnection,
    org: Uuid,
    req: AnalysisRunRequest,
) -> RResult<Plan> {
    let experiment_id = Uuid::parse_str(&req.experiment_id)
        .map_err(|_| validation(vec![invalid("experiment_id", "must be an experiment id")]))?;
    let exp = load_experiment(conn, experiment_id, org).await?;
    profile_problems(&exp.profile)?;
    let profile: ScoringProfile = serde_json::from_value(exp.profile.clone())
        .map_err(|e| AppError::Internal(format!("stored experiment profile: {e}")))?;
    let (cohort_id, cohort) = match (&req.cohort_id, req.cohort) {
        (Some(raw), None) => {
            let id = Uuid::parse_str(raw)
                .map_err(|_| validation(vec![invalid("cohort_id", "must be a cohort id")]))?;
            let row = sqlx::query!(
                "SELECT org_id, definition, archived_at IS NOT NULL AS \"archived!\"
                 FROM cohorts WHERE id = $1",
                id
            )
            .fetch_optional(&mut *conn)
            .await?
            .filter(|c| c.org_id == org)
            .ok_or_else(|| validation(vec![invalid("cohort_id", "no such cohort in this org")]))?;
            if row.archived {
                return Err(validation(vec![invalid(
                    "cohort_id",
                    "the cohort is archived",
                )]));
            }
            (
                Some(id),
                decode::<CohortDefinition>("cohort", row.definition)?,
            )
        }
        (None, Some(def)) => (None, def),
        _ => {
            return Err(validation(vec![invalid(
                "cohort_id",
                "give exactly one of cohort_id and cohort",
            )]))
        }
    };
    check_options(&req.options)?;
    let seed = req.seed.unwrap_or_else(|| {
        let b = Uuid::new_v4().into_bytes();
        i64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]])
    });
    Ok(Plan {
        experiment_id: Some(experiment_id),
        cohort_id,
        definition: AnalysisDefinition {
            kind: req.kind,
            cohort,
            profile,
            baselines_version: exp.baselines_version,
            sweep: req.sweep,
            level: req.level,
            options: req.options,
            seed,
        },
        profile_body: exp.profile,
        rerun_of: None,
    })
}

/// Validates a plan against the data and the limits, then inserts the run and its job in
/// one transaction. Returns the new run's id.
async fn enqueue(
    state: &AppState,
    principal: &UserPrincipal,
    meta: &RequestMeta,
    org: Uuid,
    plan: Plan,
) -> RResult<Uuid> {
    let limits = &state.config.analysis;
    let def = &plan.definition;
    let cells = sweep_cells(def.sweep.as_ref())?;
    if def.kind != RunKind::CohortRescore {
        return Err(validation(vec![invalid(
            "kind",
            "param_sweep and age_profile runs are not available yet (phase B, WP-X.9)",
        )]));
    }
    if def.sweep.is_some() {
        return Err(validation(vec![invalid(
            "sweep",
            "only param_sweep runs take a sweep",
        )]));
    }
    let draft: AnalysisProfile = serde_json::from_value(plan.profile_body.clone())
        .map_err(|e| AppError::Internal(format!("experiment profile: {e}")))?;
    let known: BTreeSet<&str> = draft.age_bands.iter().map(|b| b.id.as_str()).collect();
    let bad_bands: Vec<InvalidParam> = def
        .cohort
        .age_bands
        .iter()
        .enumerate()
        .filter(|(_, b)| !known.contains(b.as_str()))
        .map(|(i, _)| {
            invalid(
                &format!("definition.age_bands[{i}]"),
                "not an age band of the experiment profile",
            )
        })
        .collect();
    if !bad_bands.is_empty() {
        return Err(validation(bad_bands));
    }

    let mut tx = state.db.begin().await?;
    // RUN-9: one enqueue per org at a time, so the queue count below is exact.
    sqlx::query!(
        "SELECT pg_advisory_xact_lock(hashtext('analysis_runs:' || $1::text))",
        org.to_string()
    )
    .execute(&mut *tx)
    .await?;

    let level = core_level(def.level);
    let members = job::membership(
        &mut tx,
        org,
        &def.cohort,
        &draft.age_bands,
        level,
        state.config.allow_synthetic,
    )
    .await?;
    if members.ids.is_empty() {
        return Err(bad_request(
            slugs::COHORT_EMPTY,
            "the cohort selects no research-consented session",
        ));
    }
    if def.level == LevelRequest::Frames && members.frames_n() == 0 {
        return Err(bad_request(
            slugs::ANALYSIS_LEVEL_UNAVAILABLE,
            "no session of the cohort has a research frame log; use level auto or score",
        ));
    }
    check_budget(
        limits,
        members.score_n(),
        members.frames_n(),
        cells,
        def.sweep.is_some(),
    )
    .map_err(|b| bad_request(b.slug, b.detail))?;

    let queued = sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!" FROM analysis_runs WHERE org_id = $1 AND status = 'queued'"#,
        org
    )
    .fetch_one(&mut *tx)
    .await?;
    if queued >= i64::from(limits.max_queued_per_org) {
        return Err(RunError::Problem(Box::new(
            Problem::new(
                StatusCode::TOO_MANY_REQUESTS,
                slugs::ANALYSIS_QUOTA_EXCEEDED,
            )
            .with_detail(format!(
                "the org already has {queued} queued analysis runs (at most {}); wait or \
                     cancel one",
                limits.max_queued_per_org
            )),
        )));
    }

    // RUN-5: every version before the job starts.
    let (published_version, _) =
        job::published_document(&mut tx, ConfigDocumentKind::ScoringProfile).await?;
    let baselines_version = match &def.baselines_version {
        Some(v) => {
            if job::document(&mut tx, ConfigDocumentKind::NormativeBaselines, v)
                .await?
                .is_none()
            {
                return Err(validation(vec![invalid(
                    "baselines_version",
                    "the experiment's baselines document no longer exists",
                )]));
            }
            v.clone()
        }
        None => {
            job::published_document(&mut tx, ConfigDocumentKind::NormativeBaselines)
                .await?
                .0
        }
    };
    let (catalog_version, _) =
        job::published_document(&mut tx, ConfigDocumentKind::StimulusCatalog).await?;
    let definition_json = serde_json::to_value(def)
        .map_err(|e| AppError::Internal(format!("run definition: {e}")))?;
    let cohort_json = serde_json::to_value(&def.cohort)
        .map_err(|e| AppError::Internal(format!("cohort definition: {e}")))?;
    let run_id = Uuid::new_v4();
    let mut params = json!({"run_id": run_id});
    if let Some(old) = plan.rerun_of {
        params["rerun_of"] = json!(old);
    }
    let job_row = jobs::enqueue(
        &mut tx,
        KIND_ANALYSIS,
        Some(org),
        &params,
        Some(principal.user_id),
    )
    .await?;
    let profile_hash = body_hash(&plan.profile_body);
    let definition_hash = body_hash(&definition_json);
    let cohort_hash = body_hash(&cohort_json);
    sqlx::query!(
        "INSERT INTO analysis_runs
           (id, org_id, kind, status, experiment_id, cohort_id, definition, definition_hash,
            cohort_hash, profile_hash, published_profile_version, baselines_version,
            catalog_version, core_version, contract_version, pipeline_version,
            registry_version, seed, level_requested, job_id, created_by)
         VALUES ($1, $2, $3, 'queued', $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15,
                 $16, $17, $18, $19, $20)",
        run_id,
        org,
        wire(&def.kind),
        plan.experiment_id,
        plan.cohort_id,
        definition_json,
        definition_hash,
        cohort_hash,
        profile_hash,
        published_version,
        baselines_version,
        catalog_version,
        CORE_VERSION,
        CONTRACT_VERSION,
        asd_core::gaze::PIPELINE_VERSION,
        params::registry_version(),
        def.seed,
        wire(&def.level),
        job_row.id,
        principal.user_id,
    )
    .execute(&mut *tx)
    .await?;
    if let Some(exp) = plan.experiment_id {
        // GOV-2: promote names the experiment's latest run.
        sqlx::query!(
            "UPDATE experiments SET last_run_id = $2 WHERE id = $1",
            exp,
            run_id
        )
        .execute(&mut *tx)
        .await?;
    }
    AuditEvent::user(principal, "analysis.run_enqueue", "analysis_run", run_id)
        .org(org)
        .after(json!({
            "kind": wire(&def.kind),
            "job_id": job_row.id,
            "experiment_id": plan.experiment_id,
            "cohort_id": plan.cohort_id,
            "rerun_of": plan.rerun_of,
            "level": wire(&def.level),
            "profile_hash": hex(&profile_hash),
            "definition_hash": hex(&definition_hash),
            "cohort_hash": hex(&cohort_hash),
            "sessions": members.members.len(),
        }))
        .meta(meta)
        .record(&mut tx)
        .await?;
    tx.commit().await?;
    Ok(run_id)
}

/// `core_version` of a run: the `asd-core` crate version (one workspace version).
const CORE_VERSION: &str = env!("CARGO_PKG_VERSION");
/// `contract_version` of a run: the `api_v1` contract at the workspace version.
const CONTRACT_VERSION: &str = concat!("api-v1/", env!("CARGO_PKG_VERSION"));

async fn create_run(
    auth: Require<perm::AnalysisRun>,
    meta: RequestMeta,
    State(state): State<AppState>,
    ApiJson(req): ApiJson<AnalysisRunRequest>,
) -> RResult<(StatusCode, Json<AnalysisRun>)> {
    let org = auth.org();
    let plan = {
        let mut conn = state.db.acquire().await?;
        plan_request(&mut conn, org, req).await?
    };
    let id = enqueue(&state, &auth.principal, &meta, org, plan).await?;
    let mut conn = state.db.acquire().await?;
    let run = load_run(&mut conn, id).await?;
    Ok((StatusCode::ACCEPTED, Json(run.dto(true)?)))
}

async fn rerun(
    auth: Require<perm::AnalysisRun>,
    meta: RequestMeta,
    State(state): State<AppState>,
    Path(raw): Path<String>,
) -> RResult<(StatusCode, Json<AnalysisRun>)> {
    let old_id = path_uuid(&raw)?;
    let mut conn = state.db.acquire().await?;
    let old = load_run(&mut conn, old_id).await?;
    auth.ensure_org(old.org_id)?;
    let definition: AnalysisDefinition = decode("run definition", old.definition.clone())?;
    let profile_body = serde_json::to_value(&definition.profile)
        .map_err(|e| AppError::Internal(format!("run profile: {e}")))?;
    drop(conn);
    let plan = Plan {
        experiment_id: old.experiment_id,
        cohort_id: old.cohort_id,
        definition,
        profile_body,
        rerun_of: Some(old_id),
    };
    let id = enqueue(&state, &auth.principal, &meta, old.org_id, plan).await?;
    let mut conn = state.db.acquire().await?;
    let run = load_run(&mut conn, id).await?;
    Ok((StatusCode::ACCEPTED, Json(run.dto(true)?)))
}

// ---- reads ---------------------------------------------------------------------------------

#[derive(Debug, Default, Deserialize)]
struct ListQuery {
    kind: Option<String>,
    status: Option<String>,
    experiment_id: Option<String>,
    cursor: Option<String>,
    limit: Option<String>,
}

fn parse_limit(raw: Option<&str>) -> RResult<i64> {
    let limit = match raw.filter(|s| !s.is_empty()) {
        Some(s) => Some(
            s.parse::<i64>()
                .map_err(|_| validation(vec![invalid("limit", "must be an integer")]))?,
        ),
        None => None,
    };
    Ok(cursor::limit(limit)?)
}

async fn list_runs(
    auth: Require<perm::AnalysisRead>,
    State(state): State<AppState>,
    query: Result<Query<ListQuery>, QueryRejection>,
) -> RResult<Json<AnalysisRunPage>> {
    let org = auth.org();
    let q = query_or_problem(query)?;
    let limit = parse_limit(q.limit.as_deref())?;
    let kind = match q.kind.as_deref().filter(|s| !s.is_empty()) {
        Some(k) => Some(
            serde_json::from_value::<RunKind>(Value::from(k))
                .map(|k| wire(&k))
                .map_err(|_| validation(vec![invalid("kind", "not a run kind")]))?,
        ),
        None => None,
    };
    let status = match q.status.as_deref().filter(|s| !s.is_empty()) {
        Some(s) => Some(
            serde_json::from_value::<RunStatus>(Value::from(s))
                .map(|s| wire(&s))
                .map_err(|_| validation(vec![invalid("status", "not a run status")]))?,
        ),
        None => None,
    };
    let experiment = match q.experiment_id.as_deref().filter(|s| !s.is_empty()) {
        Some(e) => Some(
            Uuid::parse_str(e)
                .map_err(|_| validation(vec![invalid("experiment_id", "must be a UUID")]))?,
        ),
        None => None,
    };
    let (after_at, after_id) = match q.cursor.as_deref().filter(|s| !s.is_empty()) {
        Some(raw) => {
            let c = Cursor::decode(raw)?;
            let id = Uuid::parse_str(&c.id)
                .map_err(|_| validation(vec![invalid("cursor", "not a cursor from this API")]))?;
            (Some(c.at), Some(id))
        }
        None => (None, None),
    };
    let rows = sqlx::query_as!(
        RunRow,
        r#"SELECT r.id, r.org_id, r.kind, r.status, r.experiment_id, r.cohort_id, r.definition,
                  r.definition_hash, r.cohort_hash, r.membership_hash, r.profile_hash,
                  r.published_profile_version, r.baselines_version, r.catalog_version,
                  r.bundle_version, r.core_version, r.contract_version, r.pipeline_version,
                  r.registry_version, r.seed, r.level_requested, r.sessions_n,
                  r.sessions_frames_n, r.sessions_score_n, r.excluded, NULL::jsonb AS summary,
                  r.result_blob_key, r.result_bytes, r.progress, r.job_id, r.error,
                  r.created_by, r.created_at, r.started_at, r.finished_at,
                  j.progress AS "job_progress?", j.result -> 'membership_drift' AS "drift?"
           FROM analysis_runs r LEFT JOIN jobs j ON j.id = r.job_id
           WHERE r.org_id = $1
             AND ($2::text IS NULL OR r.kind = $2)
             AND ($3::text IS NULL OR r.status = $3)
             AND ($4::uuid IS NULL OR r.experiment_id = $4)
             AND ($5::timestamptz IS NULL OR (r.created_at, r.id) < ($5, $6::uuid))
           ORDER BY r.created_at DESC, r.id DESC
           LIMIT $7"#,
        org,
        kind,
        status,
        experiment,
        after_at,
        after_id,
        limit + 1,
    )
    .fetch_all(&state.db)
    .await?;
    let limit = usize::try_from(limit).unwrap_or(usize::MAX);
    let has_more = rows.len() > limit;
    let mut items = Vec::new();
    let mut last = None;
    for r in rows.into_iter().take(limit) {
        last = Some(Cursor::new(r.created_at, r.id.to_string()));
        items.push(r.dto(false)?);
    }
    Ok(Json(AnalysisRunPage {
        items,
        next_cursor: if has_more {
            last.map(|c| c.encode())
        } else {
            None
        },
    }))
}

async fn get_run(
    auth: Require<perm::AnalysisRead>,
    meta: RequestMeta,
    State(state): State<AppState>,
    Path(raw): Path<String>,
) -> RResult<Json<AnalysisRun>> {
    let id = path_uuid(&raw)?;
    let mut conn = state.db.acquire().await?;
    let run = load_run(&mut conn, id).await?;
    drop(conn);
    auth.ensure_org(run.org_id)?;
    // One audit row per run view (GOV-4): the page covers many participants.
    record_read(
        &state.db,
        &auth.principal,
        &meta,
        run.org_id,
        "analysis.read",
        "analysis_run",
        &id.to_string(),
        Some(json!({"status": run.status})),
    )
    .await?;
    Ok(Json(run.dto(true)?))
}

#[derive(Debug, Default, Deserialize)]
struct PageQuery {
    cursor: Option<String>,
    limit: Option<String>,
}

async fn run_sessions(
    auth: Require<perm::AnalysisRead>,
    meta: RequestMeta,
    State(state): State<AppState>,
    Path(raw): Path<String>,
    query: Result<Query<PageQuery>, QueryRejection>,
) -> RResult<Json<AnalysisSessionPage>> {
    let id = path_uuid(&raw)?;
    let q = query_or_problem(query)?;
    let limit = parse_limit(q.limit.as_deref())?;
    let mut conn = state.db.acquire().await?;
    let run = load_run(&mut conn, id).await?;
    drop(conn);
    // PRV-2: the rows are per session, so the caller must also read sessions of the org.
    rbac::authorize(&auth.principal, Action::SessionRead, Some(run.org_id))?;
    let offset = match q.cursor.as_deref().filter(|s| !s.is_empty()) {
        Some(c) => {
            let c = Cursor::decode(c)?;
            c.id.parse::<usize>()
                .ok()
                .filter(|_| c.at == run.created_at)
                .ok_or_else(|| validation(vec![invalid("cursor", "not a cursor of this run")]))?
        }
        None => 0,
    };
    let Some(key) = run.result_blob_key.as_deref() else {
        return Err(RunError::App(AppError::Conflict {
            slug: slugs::INVALID_TRANSITION,
            detail: format!("the run is {}; it has no per-session rows", run.status),
        }));
    };
    let (Some(keyring), blobs) = (state.keyring.as_deref(), &state.blobs) else {
        return Err(RunError::App(AppError::Unavailable));
    };
    let limit = usize::try_from(limit).unwrap_or(usize::MAX);
    record_read(
        &state.db,
        &auth.principal,
        &meta,
        run.org_id,
        "analysis.sessions_read",
        "analysis_run",
        &id.to_string(),
        Some(json!({"offset": offset, "limit": limit})),
    )
    .await?;
    let stored = blobs
        .get(key)
        .await
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::Internal("analysis result blob missing".into()))?;
    let bytes = open_result(keyring, run.org_id, id, &stored.bytes).await?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|_| AppError::Internal("analysis result is not UTF-8".into()))?;
    let mut lines = text.lines().filter(|l| !l.is_empty()).skip(offset);
    let mut items = Vec::new();
    for line in lines.by_ref().take(limit) {
        let row: AnalysisSessionRow = serde_json::from_str(line)
            .map_err(|e| AppError::Internal(format!("analysis result row: {e}")))?;
        items.push(row);
    }
    let next_cursor = lines
        .next()
        .is_some()
        .then(|| Cursor::new(run.created_at, (offset + items.len()).to_string()).encode());
    Ok(Json(AnalysisSessionPage { items, next_cursor }))
}

// ---- cancel ------------------------------------------------------------------------------------

async fn cancel_run(
    auth: Require<perm::AnalysisRun>,
    meta: RequestMeta,
    State(state): State<AppState>,
    Path(raw): Path<String>,
) -> RResult<(StatusCode, Json<AnalysisRun>)> {
    let id = path_uuid(&raw)?;
    let mut tx = state.db.begin().await?;
    let row = sqlx::query!(
        "SELECT org_id, status, job_id FROM analysis_runs WHERE id = $1 FOR UPDATE",
        id
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(RunError::App(AppError::NotFound))?;
    auth.ensure_org(row.org_id)?;
    let outcome = match (row.status.as_str(), row.job_id) {
        ("queued" | "running", Some(job_id)) => {
            let at_once = cancel_job(&mut tx, job_id).await?;
            if at_once {
                sqlx::query!(
                    "UPDATE analysis_runs SET status = 'cancelled', finished_at = now()
                     WHERE id = $1",
                    id
                )
                .execute(&mut *tx)
                .await?;
                "cancelled"
            } else {
                "cancel_requested"
            }
        }
        _ => {
            return Err(RunError::App(AppError::Conflict {
                slug: slugs::JOB_FINISHED,
                detail: format!("the run is {}; there is nothing to cancel", row.status),
            }))
        }
    };
    AuditEvent::user(&auth.principal, "analysis.run_cancel", "analysis_run", id)
        .org(row.org_id)
        .before(json!({"status": row.status}))
        .after(json!({"outcome": outcome, "job_id": row.job_id}))
        .meta(&meta)
        .record(&mut tx)
        .await?;
    tx.commit().await?;
    let mut conn = state.db.acquire().await?;
    let run = load_run(&mut conn, id).await?;
    Ok((StatusCode::ACCEPTED, Json(run.dto(true)?)))
}

/// RUN-8: a queued job ends `cancelled` at once (`true`); a running one gets
/// `cancel_requested_at` and stops at its next check (`false`).
pub async fn cancel_job(conn: &mut PgConnection, job_id: Uuid) -> Result<bool, AppError> {
    let at_once = sqlx::query!(
        "UPDATE jobs SET status = 'cancelled', cancel_requested_at = now(), finished_at = now(),
                         locked_by = NULL
         WHERE id = $1 AND status = 'queued'",
        job_id
    )
    .execute(&mut *conn)
    .await?
    .rows_affected()
        == 1;
    if !at_once {
        sqlx::query!(
            "UPDATE jobs SET cancel_requested_at = coalesce(cancel_requested_at, now())
             WHERE id = $1 AND status = 'running'",
            job_id
        )
        .execute(&mut *conn)
        .await?;
    }
    Ok(at_once)
}
