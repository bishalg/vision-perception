//! owner: WP-X.2
//!
//! Researcher toolkit: experiments, cohorts and the effective configuration
//! (`docs/spec/modules/asd-research-toolkit/technical.md` EXP-2, GOV-2, COH-1..3, PRV-1,
//! GOV-6):
//!
//! - `POST /v1/orgs/{id}/experiments` (`ExperimentWrite`), `GET /v1/orgs/{id}/experiments`
//!   (`AnalysisRead`), `GET /v1/experiments/{id}` (`AnalysisRead`), `PATCH
//!   /v1/experiments/{id}` (`ExperimentWrite`, `If-Match`), `POST
//!   /v1/experiments/{id}/archive` (`ExperimentWrite`), `POST /v1/experiments/{id}/promote`
//!   (`ExperimentPromote`);
//! - `POST /v1/orgs/{id}/cohorts` (`ExperimentWrite`), `GET /v1/orgs/{id}/cohorts`
//!   (`AnalysisRead`), `GET /v1/cohorts/{id}` (`AnalysisRead`), `PATCH /v1/cohorts/{id}`
//!   (`ExperimentWrite`), `POST /v1/orgs/{id}/cohorts/preview` (`AnalysisRead`);
//! - `GET /v1/orgs/{id}/config/effective` (`ConfigRead`).
//!
//! ## Experiments (EXP-1, EXP-2, GOV-2)
//!
//! An experiment is an org's named draft `ScoringProfile` (stored whole, schema 1.2.0, with
//! `profile_hash` = sha256 of its RFC 8785 form). It starts from the published profile (the
//! default profile, base version `default`, when none is published), another experiment of
//! the org, or a named `scoring_profile` document. `PATCH` needs `If-Match: "<row_version>"`
//! (`412 stale-version`), validates a new profile with the parameter registry
//! (`params::validate`, PRM-3: `400 config-invalid` with `field_problems: [{path,
//! message}]`) and audits the changed registry ids, never values. Only a `draft` experiment
//! changes; `archive` and `promote` end it. Promote files a **draft** config document
//! through `config_docs::lifecycle::create` with notes `experiment:<id> run:<run>` and the
//! caller as author (`409 experiment-no-run` without a run, `409 experiment-promoted` the
//! second time); approve and publish stay the vendor's four-eyes steps (GOV-3 then needs the
//! run's evidence).
//!
//! ## Cohorts (COH-1..3, PRV-1, AGE-3)
//!
//! A cohort is a saved [`CohortDefinition`] (`definition_hash` = sha256 of its RFC 8785
//! form). The preview counts, in one SQL statement, the org's real-capture sessions in the
//! definition's window, devices and frame requirement, classifies each as excluded
//! (`no_consent`, `withdrawn`, `unscorable`, `no_age`, `quality`, in that order) or in, and
//! places it in an age band of the profile (the published one, or `?experiment_id=`'s, so a
//! researcher can test other bands). Research consent and "not withdrawn" are always
//! applied. Counts below the org's `settings.research.small_cell_k` (default 12, 5..=50) are
//! suppressed in Rust after the query: a band (and `frames_n`) with `0 < n < k` is `null`
//! with `suppressed: true`; zero stays zero.
//!
//! ## Effective configuration (GOV-6, INV-2)
//!
//! The org's newest signed bundle's documents when the org has one, else the global published
//! documents; a kind with nothing published falls back to the bundled default (version
//! `bundled-<schema>`), the documented fallback of every number.
//!
//! Audits: `experiment.create|update|archive|promote` (plus `config.document.create` from the
//! lifecycle), `cohort.create|update|archive`; reads of these records are not clinical
//! content and are not audited (the frame log read is, `session.frames_read`).

use std::collections::{BTreeMap, BTreeSet};

use asd_contract::api_v1::ScoringStatus;
use asd_contract::api_v1::{ConfigDocument, ConfigDocumentKind};
use asd_contract::config::{ScoringProfile, SCORING_PROFILE_SCHEMA_VERSION};
use asd_contract::params::{self, DocumentKind};
use asd_contract::problem as slugs;
use asd_contract::profile_checks::ProfileProblem;
use asd_contract::research::{
    settings as research_keys, Cohort, CohortBandCount, CohortDefinition, CohortExclusions,
    CohortPage, CohortPreview, CreateCohortRequest, CreateExperimentRequest, EffectiveConfig,
    EffectiveDocument, EffectiveSource, Experiment, ExperimentPage, ExperimentStatus, LevelCounts,
    PromoteExperimentRequest, UpdateCohortRequest, UpdateExperimentRequest, DEFAULT_SMALL_CELL_K,
    MAX_DESCRIPTION_CHARS, MAX_NAME_CHARS, MAX_SMALL_CELL_K, MIN_SMALL_CELL_K,
};
use asd_core::types::AgeBracket;
use axum::extract::rejection::QueryRejection;
use axum::extract::{Path, Query, State};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{DateTime, NaiveDate, Utc};
use http::{header, HeaderMap, StatusCode};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use crate::app::AppState;
use crate::audit::AuditEvent;
use crate::auth::extractors::{ApiJson, RequestMeta, Require};
use crate::auth::rbac::perm;
use crate::config_docs::{kind_str, lifecycle, seed, ConfigApiError};
use crate::error::AppError;
use crate::problem::{InvalidParam, Problem};
use crate::read::cursor::{self, Cursor};
use crate::read::sessions::parse_bound;
use crate::routes::sessions::query_or_problem;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/orgs/{id}/experiments",
            post(create_experiment).get(list_experiments),
        )
        .route(
            "/experiments/{id}",
            get(get_experiment).patch(patch_experiment),
        )
        .route("/experiments/{id}/archive", post(archive_experiment))
        .route("/experiments/{id}/promote", post(promote_experiment))
        .route("/orgs/{id}/cohorts", post(create_cohort).get(list_cohorts))
        .route("/orgs/{id}/cohorts/preview", post(preview_cohort))
        .route("/cohorts/{id}", get(get_cohort).patch(patch_cohort))
        .route("/orgs/{id}/config/effective", get(effective_config))
}

// ---- errors ------------------------------------------------------------------------------

/// Errors of these routes: every [`AppError`] and config-lifecycle error, plus problems
/// with members `AppError` has no variant for (`field_problems`).
#[derive(Debug)]
pub enum ResearchError {
    App(AppError),
    Config(ConfigApiError),
    Problem(Box<Problem>),
}

impl From<AppError> for ResearchError {
    fn from(e: AppError) -> Self {
        Self::App(e)
    }
}

impl From<sqlx::Error> for ResearchError {
    fn from(e: sqlx::Error) -> Self {
        Self::App(AppError::Database(e))
    }
}

impl From<ConfigApiError> for ResearchError {
    fn from(e: ConfigApiError) -> Self {
        match e {
            ConfigApiError::App(e) => Self::App(e),
            other => Self::Config(other),
        }
    }
}

impl IntoResponse for ResearchError {
    fn into_response(self) -> Response {
        match self {
            Self::App(e) => e.into_response(),
            Self::Config(e) => e.into_response(),
            Self::Problem(p) => p.into_response(),
        }
    }
}

type RResult<T> = Result<T, ResearchError>;

fn invalid(name: &str, reason: impl Into<String>) -> InvalidParam {
    InvalidParam {
        name: name.into(),
        reason: reason.into(),
    }
}

fn validation(errors: Vec<InvalidParam>) -> ResearchError {
    ResearchError::App(AppError::Validation(errors))
}

/// `400 config-invalid` with `field_problems` (PRM-3, EXP-2).
fn config_invalid(problems: &[ProfileProblem]) -> ResearchError {
    let field_problems: Vec<Value> = problems
        .iter()
        .map(|p| json!({"path": p.path, "message": p.message}))
        .collect();
    ResearchError::Problem(Box::new(
        Problem::new(StatusCode::BAD_REQUEST, slugs::CONFIG_INVALID)
            .with_detail("the profile fails the parameter registry checks")
            .with_extension("field_problems", Value::Array(field_problems)),
    ))
}

fn conflict(slug: &'static str, detail: impl Into<String>) -> ResearchError {
    ResearchError::App(AppError::Conflict {
        slug,
        detail: detail.into(),
    })
}

/// A path UUID; anything else is an unknown resource (404).
fn path_uuid(raw: &str) -> RResult<Uuid> {
    Uuid::parse_str(raw).map_err(|_| ResearchError::App(AppError::NotFound))
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

/// Whether `e` is the unique violation of `constraint`.
fn is_unique(e: &sqlx::Error, constraint: &str) -> bool {
    matches!(e, sqlx::Error::Database(d) if d.constraint() == Some(constraint))
}

/// A name: trimmed, 1..=80 characters, no control characters (EXP-1, COH-2).
fn check_name(raw: &str, errors: &mut Vec<InvalidParam>) -> String {
    let name = raw.trim().to_owned();
    let n = name.chars().count();
    if n == 0 || n > MAX_NAME_CHARS || name.chars().any(char::is_control) {
        errors.push(invalid(
            "name",
            format!("must be 1 to {MAX_NAME_CHARS} characters without control characters"),
        ));
    }
    name
}

fn check_description(raw: Option<&str>, errors: &mut Vec<InvalidParam>) -> Option<String> {
    let d = raw.map(str::trim).filter(|d| !d.is_empty())?;
    if d.chars().count() > MAX_DESCRIPTION_CHARS {
        errors.push(invalid(
            "description",
            format!("must be at most {MAX_DESCRIPTION_CHARS} characters"),
        ));
    }
    Some(d.to_owned())
}

/// `If-Match: "<n>"` (also `W/"<n>"` or a bare `n`) as a row version.
fn if_match(headers: &HeaderMap) -> RResult<i32> {
    let bad = || {
        validation(vec![invalid(
            "If-Match",
            "required: the row_version as \"<n>\"",
        )])
    };
    let raw = headers
        .get(header::IF_MATCH)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(bad)?
        .trim();
    let raw = raw.strip_prefix("W/").unwrap_or(raw);
    let raw = raw
        .strip_prefix('"')
        .and_then(|r| r.strip_suffix('"'))
        .unwrap_or(raw);
    raw.parse::<i32>().ok().filter(|n| *n >= 1).ok_or_else(bad)
}

/// A decoded list cursor: `(updated_at, id)` of the last row of the previous page.
type PageAfter = (DateTime<Utc>, Uuid);

/// `?cursor=&limit=` plus the list's own filter.
#[derive(Debug, Default, Deserialize)]
struct ListQuery {
    status: Option<String>,
    archived: Option<String>,
    cursor: Option<String>,
    limit: Option<String>,
}

impl ListQuery {
    fn page(&self) -> RResult<(Option<PageAfter>, i64)> {
        let limit = match self.limit.as_deref().filter(|s| !s.is_empty()) {
            Some(s) => Some(
                s.parse::<i64>()
                    .map_err(|_| validation(vec![invalid("limit", "must be an integer")]))?,
            ),
            None => None,
        };
        let limit = cursor::limit(limit)?;
        let after = match self.cursor.as_deref().filter(|s| !s.is_empty()) {
            Some(raw) => {
                let c = Cursor::decode(raw)?;
                let id = Uuid::parse_str(&c.id).map_err(|_| {
                    validation(vec![invalid("cursor", "not a cursor from this API")])
                })?;
                Some((c.at, id))
            }
            None => None,
        };
        Ok((after, limit))
    }
}

// ---- org settings (PRV-1) -----------------------------------------------------------------

/// `organizations.settings.research.small_cell_k`; the default (12) when absent or outside
/// `5..=50` (the org route refuses such values; this guards rows written before it).
pub fn small_cell_k(settings: &Value) -> u32 {
    settings
        .get(research_keys::RESEARCH)
        .and_then(|r| r.get(research_keys::SMALL_CELL_K))
        .and_then(Value::as_u64)
        .and_then(|k| u32::try_from(k).ok())
        .filter(|k| (MIN_SMALL_CELL_K..=MAX_SMALL_CELL_K).contains(k))
        .unwrap_or(DEFAULT_SMALL_CELL_K)
}

/// The small-cell threshold of `org`.
pub async fn org_small_cell_k(db: &PgPool, org: Uuid) -> Result<u32, AppError> {
    let settings = sqlx::query_scalar!("SELECT settings FROM organizations WHERE id = $1", org)
        .fetch_optional(db)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(small_cell_k(&settings))
}

/// PRV-1 for one count: `None` when `0 < n < k` (zero stays zero).
pub fn suppress_count(n: u32, k: u32) -> Option<u32> {
    (n == 0 || n >= k).then_some(n)
}

// ---- experiments -----------------------------------------------------------------------------

/// An `experiments` row.
#[derive(Debug, Clone)]
struct ExperimentRow {
    id: Uuid,
    org_id: Uuid,
    name: String,
    description: Option<String>,
    status: String,
    base_profile_version: String,
    profile: Value,
    profile_hash: Vec<u8>,
    baselines_version: Option<String>,
    promoted_config_version: Option<String>,
    last_run_id: Option<Uuid>,
    row_version: i32,
    created_by: Uuid,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

fn experiment_status(s: &str) -> RResult<ExperimentStatus> {
    serde_json::from_value(Value::from(s))
        .map_err(|_| ResearchError::App(AppError::Internal("stored experiment status".into())))
}

impl ExperimentRow {
    fn status(&self) -> RResult<ExperimentStatus> {
        experiment_status(&self.status)
    }

    fn dto(&self) -> RResult<Experiment> {
        Ok(Experiment {
            id: self.id.to_string(),
            org_id: self.org_id.to_string(),
            name: self.name.clone(),
            description: self.description.clone(),
            status: self.status()?,
            base_profile_version: self.base_profile_version.clone(),
            profile: serde_json::from_value(self.profile.clone()).map_err(|e| {
                ResearchError::App(AppError::Internal(format!(
                    "stored experiment profile: {e}"
                )))
            })?,
            profile_hash: hex(&self.profile_hash),
            baselines_version: self.baselines_version.clone(),
            promoted_config_version: self.promoted_config_version.clone(),
            last_run_id: self.last_run_id.map(|u| u.to_string()),
            row_version: self.row_version,
            created_by: Some(self.created_by.to_string()),
            created_at: cursor::ts(self.created_at),
            updated_at: cursor::ts(self.updated_at),
        })
    }

    /// Only a draft experiment changes (`409 invalid-transition` otherwise).
    fn ensure_draft(&self, step: &str) -> RResult<()> {
        match self.status()? {
            ExperimentStatus::Draft => Ok(()),
            ExperimentStatus::Promoted if step == "promote" => Err(conflict(
                slugs::EXPERIMENT_PROMOTED,
                "this experiment was already promoted to a draft document",
            )),
            other => Err(conflict(
                slugs::INVALID_TRANSITION,
                format!("cannot {step} an experiment that is {other:?}").to_lowercase(),
            )),
        }
    }
}

async fn load_experiment(
    conn: &mut PgConnection,
    id: Uuid,
    for_update: bool,
) -> RResult<ExperimentRow> {
    let row = if for_update {
        sqlx::query_as!(
            ExperimentRow,
            "SELECT id, org_id, name, description, status, base_profile_version, profile,
                    profile_hash, baselines_version, promoted_config_version, last_run_id,
                    row_version, created_by, created_at, updated_at
             FROM experiments WHERE id = $1 FOR UPDATE",
            id
        )
        .fetch_optional(&mut *conn)
        .await?
    } else {
        sqlx::query_as!(
            ExperimentRow,
            "SELECT id, org_id, name, description, status, base_profile_version, profile,
                    profile_hash, baselines_version, promoted_config_version, last_run_id,
                    row_version, created_by, created_at, updated_at
             FROM experiments WHERE id = $1",
            id
        )
        .fetch_optional(&mut *conn)
        .await?
    };
    row.ok_or(ResearchError::App(AppError::NotFound))
}

/// A profile body as an experiment stores it: registry-valid (PRM-3), decoded, schema 1.2.0,
/// serialised whole (every member present, so the hash names one document).
fn normalise_profile(body: &Value) -> RResult<(ScoringProfile, Value)> {
    let problems = params::validate(DocumentKind::ScoringProfile, body);
    if !problems.is_empty() {
        return Err(config_invalid(&problems));
    }
    let mut profile: ScoringProfile = serde_json::from_value(body.clone()).map_err(|e| {
        config_invalid(&[ProfileProblem {
            path: String::new(),
            message: format!("the profile does not decode: {e}"),
        }])
    })?;
    profile.schema_version = SCORING_PROFILE_SCHEMA_VERSION.to_owned();
    let value = serde_json::to_value(&profile)
        .map_err(|e| ResearchError::App(AppError::Internal(format!("profile encode: {e}"))))?;
    Ok((profile, value))
}

/// Registry ids whose value differs between two profile bodies (audit `changed_params`).
fn changed_params(old: &Value, new: &Value) -> Vec<String> {
    params::registry()
        .iter()
        .filter(|s| params::get_path(old, &s.id) != params::get_path(new, &s.id))
        .map(|s| s.id.clone())
        .collect()
}

/// A `normative_baselines` document must exist for `baselines_version` (any status but
/// retired: drafts are what an experiment scores against, EXP-1).
async fn check_baselines_version(conn: &mut PgConnection, version: &str) -> RResult<()> {
    let ok = sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM config_documents
                          WHERE kind = 'normative_baselines' AND version = $1
                            AND status <> 'retired') AS "x!""#,
        version
    )
    .fetch_one(&mut *conn)
    .await?;
    if ok {
        Ok(())
    } else {
        Err(validation(vec![invalid(
            "baselines_version",
            "no normative_baselines document with this version (or it is retired)",
        )]))
    }
}

async fn create_experiment(
    auth: Require<perm::ExperimentWrite>,
    meta: RequestMeta,
    State(state): State<AppState>,
    ApiJson(req): ApiJson<CreateExperimentRequest>,
) -> RResult<(StatusCode, Json<Experiment>)> {
    let org = auth.org();
    let mut errors = Vec::new();
    let name = check_name(&req.name, &mut errors);
    let description = check_description(req.description.as_deref(), &mut errors);
    if req.from_experiment_id.is_some() && req.from_version.is_some() {
        errors.push(invalid(
            "from_version",
            "name at most one of from_experiment_id and from_version",
        ));
    }
    if !errors.is_empty() {
        return Err(validation(errors));
    }
    let mut tx = state.db.begin().await?;
    let (base_version, body, baselines_version) = match (&req.from_experiment_id, &req.from_version)
    {
        (Some(raw), _) => {
            let from = load_experiment(&mut tx, path_uuid(raw)?, false).await?;
            if from.org_id != org {
                return Err(AppError::NotFound.into());
            }
            (
                from.base_profile_version,
                from.profile,
                from.baselines_version,
            )
        }
        (None, Some(version)) => {
            let doc = lifecycle::get(&mut tx, ConfigDocumentKind::ScoringProfile, version).await?;
            (doc.version, doc.body, None)
        }
        (None, None) => match lifecycle::published_scoring_profile(&mut tx).await? {
            Some((version, p)) => (
                version,
                serde_json::to_value(p)
                    .map_err(|e| AppError::Internal(format!("profile encode: {e}")))?,
                None,
            ),
            // The documented fallback: today's constants.
            None => (
                "default".to_owned(),
                serde_json::to_value(ScoringProfile::default())
                    .map_err(|e| AppError::Internal(format!("profile encode: {e}")))?,
                None,
            ),
        },
    };
    let (_, profile) = normalise_profile(&body)?;
    let hash = lifecycle::body_hash(&profile);
    let row = sqlx::query_as!(
        ExperimentRow,
        "INSERT INTO experiments
           (org_id, name, description, base_profile_version, profile, profile_hash,
            baselines_version, created_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
         RETURNING id, org_id, name, description, status, base_profile_version, profile,
                   profile_hash, baselines_version, promoted_config_version, last_run_id,
                   row_version, created_by, created_at, updated_at",
        org,
        name,
        description,
        base_version,
        profile,
        hash,
        baselines_version,
        auth.principal.user_id,
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| {
        if is_unique(&e, "experiments_org_name_key") {
            conflict(
                slugs::EXPERIMENT_NAME_EXISTS,
                "an experiment of this org already has this name",
            )
        } else {
            e.into()
        }
    })?;
    AuditEvent::user(&auth.principal, "experiment.create", "experiment", row.id)
        .org(org)
        .after(json!({
            "base_profile_version": row.base_profile_version,
            "profile_hash": hex(&row.profile_hash),
            "from_experiment_id": req.from_experiment_id,
            "from_version": req.from_version,
        }))
        .meta(&meta)
        .record(&mut tx)
        .await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(row.dto()?)))
}

async fn list_experiments(
    auth: Require<perm::AnalysisRead>,
    State(state): State<AppState>,
    query: Result<Query<ListQuery>, QueryRejection>,
) -> RResult<Json<ExperimentPage>> {
    let org = auth.org();
    let q = query_or_problem(query)?;
    let status = match q.status.as_deref().filter(|s| !s.is_empty()) {
        Some(s) => {
            experiment_status(s).map_err(|_| {
                validation(vec![invalid(
                    "status",
                    "must be draft, archived or promoted",
                )])
            })?;
            Some(s.to_owned())
        }
        None => None,
    };
    let (after, limit) = q.page()?;
    let (after_at, after_id) = after.unzip();
    let mut rows = sqlx::query_as!(
        ExperimentRow,
        "SELECT id, org_id, name, description, status, base_profile_version, profile,
                profile_hash, baselines_version, promoted_config_version, last_run_id,
                row_version, created_by, created_at, updated_at
         FROM experiments
         WHERE org_id = $1
           AND ($2::text IS NULL OR status = $2)
           AND ($3::timestamptz IS NULL OR (updated_at, id) < ($3, $4::uuid))
         ORDER BY updated_at DESC, id DESC
         LIMIT $5",
        org,
        status,
        after_at,
        after_id,
        limit + 1,
    )
    .fetch_all(&state.db)
    .await?;
    let next_cursor = if rows.len() > usize::try_from(limit).unwrap_or(usize::MAX) {
        rows.pop();
        rows.last()
            .map(|r| Cursor::new(r.updated_at, r.id.to_string()).encode())
    } else {
        None
    };
    Ok(Json(ExperimentPage {
        items: rows
            .iter()
            .map(ExperimentRow::dto)
            .collect::<RResult<_>>()?,
        next_cursor,
    }))
}

async fn get_experiment(
    auth: Require<perm::AnalysisRead>,
    State(state): State<AppState>,
    Path(raw): Path<String>,
) -> RResult<Json<Experiment>> {
    let mut conn = state.db.acquire().await?;
    let row = load_experiment(&mut conn, path_uuid(&raw)?, false).await?;
    auth.ensure_org(row.org_id)?;
    Ok(Json(row.dto()?))
}

async fn patch_experiment(
    auth: Require<perm::ExperimentWrite>,
    meta: RequestMeta,
    State(state): State<AppState>,
    Path(raw): Path<String>,
    headers: HeaderMap,
    ApiJson(req): ApiJson<UpdateExperimentRequest>,
) -> RResult<Json<Experiment>> {
    let id = path_uuid(&raw)?;
    let expected = if_match(&headers)?;
    let mut errors = Vec::new();
    let name = req.name.as_deref().map(|n| check_name(n, &mut errors));
    let description = req
        .description
        .as_deref()
        .map(|d| check_description(Some(d), &mut errors));
    if !errors.is_empty() {
        return Err(validation(errors));
    }
    let new_profile = req.profile.as_ref().map(normalise_profile).transpose()?;

    let mut tx = state.db.begin().await?;
    let before = load_experiment(&mut tx, id, true).await?;
    auth.ensure_org(before.org_id)?;
    before.ensure_draft("update")?;
    if before.row_version != expected {
        return Err(ResearchError::App(AppError::PreconditionFailed {
            slug: slugs::STALE_VERSION,
            detail: format!(
                "the experiment is at row_version {}, not {expected}",
                before.row_version
            ),
        }));
    }
    let baselines_version = match req.baselines_version.as_deref().map(str::trim) {
        None => before.baselines_version.clone(),
        Some("") => None,
        Some(v) => {
            check_baselines_version(&mut tx, v).await?;
            Some(v.to_owned())
        }
    };
    let (profile, changed) = match &new_profile {
        Some((_, value)) => (value.clone(), changed_params(&before.profile, value)),
        None => (before.profile.clone(), Vec::new()),
    };
    let hash = lifecycle::body_hash(&profile);
    let row = sqlx::query_as!(
        ExperimentRow,
        "UPDATE experiments
         SET name = $2, description = $3, profile = $4, profile_hash = $5,
             baselines_version = $6, row_version = row_version + 1, updated_at = now()
         WHERE id = $1
         RETURNING id, org_id, name, description, status, base_profile_version, profile,
                   profile_hash, baselines_version, promoted_config_version, last_run_id,
                   row_version, created_by, created_at, updated_at",
        id,
        name.unwrap_or_else(|| before.name.clone()),
        description.unwrap_or_else(|| before.description.clone()),
        profile,
        hash,
        baselines_version,
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| {
        if is_unique(&e, "experiments_org_name_key") {
            conflict(
                slugs::EXPERIMENT_NAME_EXISTS,
                "an experiment of this org already has this name",
            )
        } else {
            e.into()
        }
    })?;
    AuditEvent::user(&auth.principal, "experiment.update", "experiment", id)
        .org(row.org_id)
        .before(json!({
            "row_version": before.row_version,
            "profile_hash": hex(&before.profile_hash),
        }))
        .after(json!({
            "row_version": row.row_version,
            "profile_hash": hex(&row.profile_hash),
            "changed_params": changed,
            "name_changed": row.name != before.name,
            "baselines_version": row.baselines_version,
        }))
        .meta(&meta)
        .record(&mut tx)
        .await?;
    tx.commit().await?;
    Ok(Json(row.dto()?))
}

async fn archive_experiment(
    auth: Require<perm::ExperimentWrite>,
    meta: RequestMeta,
    State(state): State<AppState>,
    Path(raw): Path<String>,
) -> RResult<Json<Experiment>> {
    let id = path_uuid(&raw)?;
    let mut tx = state.db.begin().await?;
    let before = load_experiment(&mut tx, id, true).await?;
    auth.ensure_org(before.org_id)?;
    before.ensure_draft("archive")?;
    let row = sqlx::query_as!(
        ExperimentRow,
        "UPDATE experiments
         SET status = 'archived', row_version = row_version + 1, updated_at = now()
         WHERE id = $1
         RETURNING id, org_id, name, description, status, base_profile_version, profile,
                   profile_hash, baselines_version, promoted_config_version, last_run_id,
                   row_version, created_by, created_at, updated_at",
        id
    )
    .fetch_one(&mut *tx)
    .await?;
    AuditEvent::user(&auth.principal, "experiment.archive", "experiment", id)
        .org(row.org_id)
        .before(json!({"status": before.status}))
        .after(json!({"status": row.status, "profile_hash": hex(&row.profile_hash)}))
        .meta(&meta)
        .record(&mut tx)
        .await?;
    tx.commit().await?;
    Ok(Json(row.dto()?))
}

/// The `notes` of a promoted draft (GOV-2): the evidence first, the caller's text after.
pub fn promote_notes(experiment: Uuid, run: Uuid, notes: Option<&str>) -> String {
    let head = format!("experiment:{experiment} run:{run}");
    match notes.map(str::trim).filter(|n| !n.is_empty()) {
        Some(n) => format!("{head}\n{n}"),
        None => head,
    }
}

async fn promote_experiment(
    auth: Require<perm::ExperimentPromote>,
    meta: RequestMeta,
    State(state): State<AppState>,
    Path(raw): Path<String>,
    ApiJson(req): ApiJson<PromoteExperimentRequest>,
) -> RResult<(StatusCode, Json<ConfigDocument>)> {
    let id = path_uuid(&raw)?;
    let mut tx = state.db.begin().await?;
    let exp = load_experiment(&mut tx, id, true).await?;
    auth.ensure_org(exp.org_id)?;
    exp.ensure_draft("promote")?;
    let run = exp.last_run_id.ok_or_else(|| {
        conflict(
            slugs::EXPERIMENT_NO_RUN,
            "run the experiment on a cohort before proposing it (the run can be small)",
        )
    })?;
    let body = match req.kind {
        ConfigDocumentKind::ScoringProfile => exp.profile.clone(),
        ConfigDocumentKind::NormativeBaselines => {
            let version = exp.baselines_version.as_deref().ok_or_else(|| {
                validation(vec![invalid(
                    "kind",
                    "the experiment names no baselines_version to promote",
                )])
            })?;
            lifecycle::get(&mut tx, ConfigDocumentKind::NormativeBaselines, version)
                .await?
                .body
        }
        ConfigDocumentKind::StimulusCatalog => {
            return Err(validation(vec![invalid(
                "kind",
                "must be scoring_profile or normative_baselines",
            )]))
        }
    };
    let notes = promote_notes(id, run, req.notes.as_deref());
    let doc = lifecycle::create(
        &mut tx,
        &auth.principal,
        &meta,
        req.kind,
        &req.version,
        &body,
        Some(&notes),
    )
    .await?;
    sqlx::query!(
        "UPDATE experiments
         SET status = 'promoted', promoted_config_version = $2,
             row_version = row_version + 1, updated_at = now()
         WHERE id = $1",
        id,
        doc.version,
    )
    .execute(&mut *tx)
    .await?;
    AuditEvent::user(&auth.principal, "experiment.promote", "experiment", id)
        .org(exp.org_id)
        .after(json!({
            "kind": kind_str(req.kind),
            "version": doc.version,
            "config_document_id": doc.id,
            "last_run_id": run,
            "profile_hash": hex(&exp.profile_hash),
        }))
        .meta(&meta)
        .record(&mut tx)
        .await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(doc.to_dto()?)))
}

// ---- cohorts ---------------------------------------------------------------------------------

/// A `cohorts` row.
#[derive(Debug, Clone)]
struct CohortRow {
    id: Uuid,
    org_id: Uuid,
    name: String,
    definition: Value,
    definition_hash: Vec<u8>,
    created_by: Uuid,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    archived_at: Option<DateTime<Utc>>,
}

impl CohortRow {
    fn dto(&self) -> RResult<Cohort> {
        Ok(Cohort {
            id: self.id.to_string(),
            org_id: self.org_id.to_string(),
            name: self.name.clone(),
            definition: serde_json::from_value(self.definition.clone()).map_err(|e| {
                ResearchError::App(AppError::Internal(format!("stored cohort: {e}")))
            })?,
            definition_hash: hex(&self.definition_hash),
            created_by: Some(self.created_by.to_string()),
            created_at: cursor::ts(self.created_at),
            updated_at: cursor::ts(self.updated_at),
            archived_at: self.archived_at.map(cursor::ts),
        })
    }
}

async fn load_cohort(conn: &mut PgConnection, id: Uuid, for_update: bool) -> RResult<CohortRow> {
    let row = if for_update {
        sqlx::query_as!(
            CohortRow,
            "SELECT id, org_id, name, definition, definition_hash, created_by, created_at,
                    updated_at, archived_at
             FROM cohorts WHERE id = $1 FOR UPDATE",
            id
        )
        .fetch_optional(&mut *conn)
        .await?
    } else {
        sqlx::query_as!(
            CohortRow,
            "SELECT id, org_id, name, definition, definition_hash, created_by, created_at,
                    updated_at, archived_at
             FROM cohorts WHERE id = $1",
            id
        )
        .fetch_optional(&mut *conn)
        .await?
    };
    row.ok_or(ResearchError::App(AppError::NotFound))
}

/// The `received_at` window of a definition: `[from 00:00, to 23:59:59.999999]` UTC.
pub type Window = (Option<DateTime<Utc>>, Option<DateTime<Utc>>);

/// Shape checks of a definition that need no profile (COH-1). Returns the parsed window and
/// device ids.
pub fn check_definition(
    def: &CohortDefinition,
    allow_synthetic: bool,
) -> Result<(Window, Vec<Uuid>), Vec<InvalidParam>> {
    let mut errors = Vec::new();
    let day = |name: &str, raw: &Option<String>, upper: bool, errors: &mut Vec<InvalidParam>| {
        let raw = raw.as_deref()?;
        if NaiveDate::parse_from_str(raw, "%Y-%m-%d").is_err() {
            errors.push(invalid(name, "must be a date YYYY-MM-DD"));
            return None;
        }
        parse_bound(raw, upper)
    };
    let from = day("definition.from", &def.from, false, &mut errors);
    let to = day("definition.to", &def.to, true, &mut errors);
    if let (Some(f), Some(t)) = (from, to) {
        if f > t {
            errors.push(invalid(
                "definition.from",
                "must not be after definition.to",
            ));
        }
    }
    let mut devices = Vec::new();
    for (i, raw) in def.device_ids.iter().enumerate() {
        match Uuid::parse_str(raw) {
            Ok(id) => devices.push(id),
            Err(_) => errors.push(invalid(
                &format!("definition.device_ids[{i}]"),
                "must be a device id (UUID)",
            )),
        }
    }
    if let Some(r) = def.min_valid_ratio {
        if !(r.is_finite() && (0.0..=1.0).contains(&r)) {
            errors.push(invalid("definition.min_valid_ratio", "must be in [0, 1]"));
        }
    }
    let mut seen = BTreeSet::new();
    for (i, band) in def.age_bands.iter().enumerate() {
        if band.trim().is_empty() || band.chars().count() > 64 || !seen.insert(band) {
            errors.push(invalid(
                &format!("definition.age_bands[{i}]"),
                "must be a distinct age band id",
            ));
        }
    }
    if def.include_synthetic && !allow_synthetic {
        errors.push(invalid(
            "definition.include_synthetic",
            "this server does not accept synthetic data (ASD_ALLOW_SYNTHETIC is off)",
        ));
    }
    if errors.is_empty() {
        Ok(((from, to), devices))
    } else {
        Err(errors)
    }
}

/// The definition's band ids must name bands of `bands` (the profile the preview uses).
fn check_bands(def: &CohortDefinition, bands: &[AgeBracket]) -> RResult<()> {
    let known: BTreeSet<&str> = bands.iter().map(|b| b.id.as_str()).collect();
    let errors: Vec<InvalidParam> = def
        .age_bands
        .iter()
        .enumerate()
        .filter(|(_, b)| !known.contains(b.as_str()))
        .map(|(i, _)| {
            invalid(
                &format!("definition.age_bands[{i}]"),
                "not an age band of the profile",
            )
        })
        .collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(validation(errors))
    }
}

fn wire<T: serde::Serialize>(v: &T) -> String {
    match serde_json::to_value(v) {
        Ok(Value::String(s)) => s,
        _ => String::new(),
    }
}

async fn create_cohort(
    auth: Require<perm::ExperimentWrite>,
    meta: RequestMeta,
    State(state): State<AppState>,
    ApiJson(req): ApiJson<CreateCohortRequest>,
) -> RResult<(StatusCode, Json<Cohort>)> {
    let org = auth.org();
    let mut errors = Vec::new();
    let name = check_name(&req.name, &mut errors);
    if let Err(e) = check_definition(&req.definition, state.config.allow_synthetic) {
        errors.extend(e);
    }
    if !errors.is_empty() {
        return Err(validation(errors));
    }
    let definition = serde_json::to_value(&req.definition)
        .map_err(|e| AppError::Internal(format!("cohort encode: {e}")))?;
    let hash = lifecycle::body_hash(&definition);
    let mut tx = state.db.begin().await?;
    let row = sqlx::query_as!(
        CohortRow,
        "INSERT INTO cohorts (org_id, name, definition, definition_hash, created_by)
         VALUES ($1, $2, $3, $4, $5)
         RETURNING id, org_id, name, definition, definition_hash, created_by, created_at,
                   updated_at, archived_at",
        org,
        name,
        definition,
        hash,
        auth.principal.user_id,
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| {
        if is_unique(&e, "cohorts_org_name_key") {
            conflict(
                slugs::COHORT_NAME_EXISTS,
                "a cohort of this org already has this name",
            )
        } else {
            e.into()
        }
    })?;
    AuditEvent::user(&auth.principal, "cohort.create", "cohort", row.id)
        .org(org)
        .after(json!({"definition_hash": hex(&row.definition_hash)}))
        .meta(&meta)
        .record(&mut tx)
        .await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(row.dto()?)))
}

async fn list_cohorts(
    auth: Require<perm::AnalysisRead>,
    State(state): State<AppState>,
    query: Result<Query<ListQuery>, QueryRejection>,
) -> RResult<Json<CohortPage>> {
    let org = auth.org();
    let q = query_or_problem(query)?;
    let archived = match q.archived.as_deref().filter(|s| !s.is_empty()) {
        None | Some("false") => false,
        Some("true") => true,
        Some(_) => {
            return Err(validation(vec![invalid(
                "archived",
                "must be true or false",
            )]))
        }
    };
    let (after, limit) = q.page()?;
    let (after_at, after_id) = after.unzip();
    let mut rows = sqlx::query_as!(
        CohortRow,
        "SELECT id, org_id, name, definition, definition_hash, created_by, created_at,
                updated_at, archived_at
         FROM cohorts
         WHERE org_id = $1
           AND (archived_at IS NOT NULL) = $2
           AND ($3::timestamptz IS NULL OR (updated_at, id) < ($3, $4::uuid))
         ORDER BY updated_at DESC, id DESC
         LIMIT $5",
        org,
        archived,
        after_at,
        after_id,
        limit + 1,
    )
    .fetch_all(&state.db)
    .await?;
    let next_cursor = if rows.len() > usize::try_from(limit).unwrap_or(usize::MAX) {
        rows.pop();
        rows.last()
            .map(|r| Cursor::new(r.updated_at, r.id.to_string()).encode())
    } else {
        None
    };
    Ok(Json(CohortPage {
        items: rows.iter().map(CohortRow::dto).collect::<RResult<_>>()?,
        next_cursor,
    }))
}

async fn get_cohort(
    auth: Require<perm::AnalysisRead>,
    State(state): State<AppState>,
    Path(raw): Path<String>,
) -> RResult<Json<Cohort>> {
    let mut conn = state.db.acquire().await?;
    let row = load_cohort(&mut conn, path_uuid(&raw)?, false).await?;
    auth.ensure_org(row.org_id)?;
    Ok(Json(row.dto()?))
}

async fn patch_cohort(
    auth: Require<perm::ExperimentWrite>,
    meta: RequestMeta,
    State(state): State<AppState>,
    Path(raw): Path<String>,
    ApiJson(req): ApiJson<UpdateCohortRequest>,
) -> RResult<Json<Cohort>> {
    let id = path_uuid(&raw)?;
    let mut errors = Vec::new();
    let name = req.name.as_deref().map(|n| check_name(n, &mut errors));
    if let Some(def) = &req.definition {
        if let Err(e) = check_definition(def, state.config.allow_synthetic) {
            errors.extend(e);
        }
    }
    if !errors.is_empty() {
        return Err(validation(errors));
    }
    let mut tx = state.db.begin().await?;
    let before = load_cohort(&mut tx, id, true).await?;
    auth.ensure_org(before.org_id)?;
    let definition = match &req.definition {
        Some(d) => serde_json::to_value(d)
            .map_err(|e| AppError::Internal(format!("cohort encode: {e}")))?,
        None => before.definition.clone(),
    };
    let hash = lifecycle::body_hash(&definition);
    let archiving = req.archived == Some(true) && before.archived_at.is_none();
    let row = sqlx::query_as!(
        CohortRow,
        "UPDATE cohorts
         SET name = $2, definition = $3, definition_hash = $4, updated_at = now(),
             archived_at = CASE WHEN $5::bool IS NULL THEN archived_at
                                WHEN $5 THEN coalesce(archived_at, now())
                                ELSE NULL END
         WHERE id = $1
         RETURNING id, org_id, name, definition, definition_hash, created_by, created_at,
                   updated_at, archived_at",
        id,
        name.unwrap_or_else(|| before.name.clone()),
        definition,
        hash,
        req.archived,
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| {
        if is_unique(&e, "cohorts_org_name_key") {
            conflict(
                slugs::COHORT_NAME_EXISTS,
                "a cohort of this org already has this name",
            )
        } else {
            e.into()
        }
    })?;
    let action = if archiving {
        "cohort.archive"
    } else {
        "cohort.update"
    };
    AuditEvent::user(&auth.principal, action, "cohort", id)
        .org(row.org_id)
        .before(json!({
            "definition_hash": hex(&before.definition_hash),
            "archived": before.archived_at.is_some(),
        }))
        .after(json!({
            "definition_hash": hex(&row.definition_hash),
            "archived": row.archived_at.is_some(),
            "name_changed": row.name != before.name,
        }))
        .meta(&meta)
        .record(&mut tx)
        .await?;
    tx.commit().await?;
    Ok(Json(row.dto()?))
}

/// `?experiment_id=` of the preview: band edges from that experiment's profile (COH-3).
#[derive(Debug, Default, Deserialize)]
struct PreviewQuery {
    experiment_id: Option<String>,
}

/// One row of the preview count: exclusion reason (or none), band, frames, sessions.
#[derive(Debug, Clone, PartialEq)]
pub struct PreviewCount {
    pub reason: Option<String>,
    pub band: Option<String>,
    pub has_frames: bool,
    pub n: u32,
}

/// Assembles the preview from the grouped counts (PRV-1 applied with `k`). Pure, so the
/// suppression rules are tested without a database.
pub fn assemble_preview(
    def: &CohortDefinition,
    bands: &[AgeBracket],
    counts: &[PreviewCount],
    k: u32,
) -> CohortPreview {
    let selected: BTreeSet<&str> = def.age_bands.iter().map(String::as_str).collect();
    let in_selection = |band: &Option<String>| {
        selected.is_empty() || band.as_deref().is_none_or(|b| selected.contains(b))
    };
    let mut per_band: BTreeMap<&str, (u32, u32)> = BTreeMap::new();
    let mut excluded = CohortExclusions::default();
    let mut by_level = LevelCounts::default();
    for c in counts.iter().filter(|c| in_selection(&c.band)) {
        match (c.reason.as_deref(), c.band.as_deref()) {
            (None, Some(band)) => {
                let e = per_band.entry(band).or_default();
                e.0 += c.n;
                if c.has_frames {
                    e.1 += c.n;
                    by_level.frames += c.n;
                } else {
                    by_level.score += c.n;
                }
            }
            (Some("no_consent"), _) => excluded.no_consent += c.n,
            (Some("withdrawn"), _) => excluded.withdrawn += c.n,
            (Some("unscorable"), _) => excluded.unscorable += c.n,
            (Some("quality"), _) => excluded.quality += c.n,
            // `no_age` and a session in no band of the profile.
            _ => excluded.no_age += c.n,
        }
    }
    let by_age_band = bands
        .iter()
        .filter(|b| selected.is_empty() || selected.contains(b.id.as_str()))
        .map(|b| {
            let (n, frames_n) = per_band.get(b.id.as_str()).copied().unwrap_or_default();
            let shown = suppress_count(n, k);
            CohortBandCount {
                band: b.id.clone(),
                n: shown,
                frames_n: shown.and_then(|_| suppress_count(frames_n, k)),
                suppressed: shown.is_none(),
            }
        })
        .collect();
    let total: u32 = per_band.values().map(|(n, _)| n).sum();
    CohortPreview {
        n: suppress_count(total, k),
        by_age_band,
        by_level,
        excluded,
        small_cell_k: k,
    }
}

/// The scoring statuses a definition admits (empty = `scored`).
fn statuses(def: &CohortDefinition) -> Vec<String> {
    if def.scoring_status.is_empty() {
        vec![wire(&ScoringStatus::Scored)]
    } else {
        def.scoring_status.iter().map(wire).collect()
    }
}

async fn preview_cohort(
    auth: Require<perm::AnalysisRead>,
    State(state): State<AppState>,
    query: Result<Query<PreviewQuery>, QueryRejection>,
    ApiJson(def): ApiJson<CohortDefinition>,
) -> RResult<Json<CohortPreview>> {
    let org = auth.org();
    let q = query_or_problem(query)?;
    let ((from, to), devices) =
        check_definition(&def, state.config.allow_synthetic).map_err(validation)?;
    let mut conn = state.db.acquire().await?;
    let profile = match q.experiment_id.as_deref().filter(|s| !s.is_empty()) {
        Some(raw) => {
            let exp = load_experiment(&mut conn, path_uuid(raw)?, false).await?;
            if exp.org_id != org {
                return Err(AppError::NotFound.into());
            }
            serde_json::from_value::<ScoringProfile>(exp.profile)
                .map_err(|e| AppError::Internal(format!("stored experiment profile: {e}")))?
        }
        None => lifecycle::effective_scoring_profile(&mut conn).await?,
    };
    check_bands(&def, &profile.age_bands)?;
    drop(conn);
    let k = org_small_cell_k(&state.db, org).await?;
    let band_ids: Vec<String> = profile.age_bands.iter().map(|b| b.id.clone()).collect();
    let band_min: Vec<i32> = profile
        .age_bands
        .iter()
        .map(|b| i32::try_from(b.min_months).unwrap_or(i32::MAX))
        .collect();
    let band_max: Vec<i32> = profile
        .age_bands
        .iter()
        .map(|b| i32::try_from(b.max_months).unwrap_or(i32::MAX))
        .collect();
    let quality: Vec<String> = def.quality_verdicts.iter().map(wire).collect();
    let tracking: Vec<String> = def.tracking_verdicts.iter().map(wire).collect();
    let rows = sqlx::query!(
        r#"WITH base AS (
               SELECT s.age_months,
                      COALESCE(s.summary -> 'normCollection' -> 'consented' = 'true'::jsonb,
                               false) AS consented,
                      p.withdrawn_at IS NOT NULL AS withdrawn,
                      COALESCE(s.scoring_status, 'scored') AS scoring_status,
                      s.quality_verdict, s.tracking_verdict, s.valid_frame_ratio,
                      s.research_frames_blob_key IS NOT NULL AS has_frames
               FROM sessions s JOIN participants p ON p.id = s.participant_id
               WHERE s.org_id = $1
                 AND ($2::timestamptz IS NULL OR s.received_at >= $2)
                 AND ($3::timestamptz IS NULL OR s.received_at <= $3)
                 AND (cardinality($4::uuid[]) = 0 OR s.device_id = ANY($4))
                 AND (s.capture_source = 'truedepth'
                      OR ($5::bool AND s.capture_source = 'synthetic'))
                 AND (NOT $6::bool OR s.research_frames_blob_key IS NOT NULL))
           SELECT CASE
                    WHEN NOT b.consented THEN 'no_consent'
                    WHEN b.withdrawn THEN 'withdrawn'
                    WHEN NOT (b.scoring_status = ANY($7::text[])) THEN 'unscorable'
                    WHEN b.age_months IS NULL OR band.id IS NULL THEN 'no_age'
                    WHEN (cardinality($8::text[]) > 0
                          AND NOT (b.quality_verdict = ANY($8::text[])))
                      OR (cardinality($9::text[]) > 0
                          AND (b.tracking_verdict IS NULL
                               OR NOT (b.tracking_verdict = ANY($9::text[]))))
                      OR ($10::float8 IS NOT NULL AND b.valid_frame_ratio < $10)
                    THEN 'quality'
                  END AS reason,
                  band.id AS band,
                  b.has_frames AS "has_frames!",
                  count(*) AS "n!"
           FROM base b
           LEFT JOIN LATERAL (
               SELECT t.id
               FROM unnest($11::text[], $12::int[], $13::int[]) WITH ORDINALITY
                    AS t(id, lo, hi, ord)
               WHERE b.age_months BETWEEN t.lo AND t.hi
               ORDER BY t.ord
               LIMIT 1) band ON true
           GROUP BY 1, 2, 3"#,
        org,
        from,
        to,
        &devices,
        def.include_synthetic,
        def.require_frames,
        &statuses(&def),
        &quality,
        &tracking,
        def.min_valid_ratio,
        &band_ids,
        &band_min,
        &band_max,
    )
    .fetch_all(&state.db)
    .await?;
    let counts: Vec<PreviewCount> = rows
        .into_iter()
        .map(|r| PreviewCount {
            reason: r.reason,
            band: r.band,
            has_frames: r.has_frames,
            n: u32::try_from(r.n).unwrap_or(u32::MAX),
        })
        .collect();
    Ok(Json(assemble_preview(&def, &profile.age_bands, &counts, k)))
}

// ---- effective configuration (GOV-6) ------------------------------------------------------

/// The bundled default of a kind (the documented fallback, version `bundled-<schema>`).
fn bundled_default(kind: ConfigDocumentKind) -> RResult<EffectiveDocument> {
    let (_, version, body) = seed::seed_documents()?
        .into_iter()
        .find(|(k, _, _)| *k == kind)
        .ok_or_else(|| AppError::Internal("no bundled default for a config kind".into()))?;
    Ok(EffectiveDocument { version, body })
}

async fn effective_config(
    auth: Require<perm::ConfigRead>,
    State(state): State<AppState>,
) -> RResult<Json<EffectiveConfig>> {
    let org = auth.org();
    let bundle = sqlx::query!(
        "SELECT b.bundle_version,
                c.version AS catalog_version, c.body AS catalog_body,
                bl.version AS baselines_version, bl.body AS baselines_body,
                sp.version AS profile_version, sp.body AS profile_body
         FROM config_bundles b
         JOIN config_documents c ON c.id = b.catalog_id
         JOIN config_documents bl ON bl.id = b.baselines_id
         JOIN config_documents sp ON sp.id = b.scoring_id
         WHERE b.org_id = $1
         ORDER BY b.published_at DESC
         LIMIT 1",
        org
    )
    .fetch_optional(&state.db)
    .await?;
    if let Some(b) = bundle {
        return Ok(Json(EffectiveConfig {
            source: EffectiveSource::OrgBundle,
            bundle_version: Some(b.bundle_version),
            scoring_profile: EffectiveDocument {
                version: b.profile_version,
                body: b.profile_body,
            },
            normative_baselines: EffectiveDocument {
                version: b.baselines_version,
                body: b.baselines_body,
            },
            stimulus_catalog: EffectiveDocument {
                version: b.catalog_version,
                body: b.catalog_body,
            },
        }));
    }
    let published = sqlx::query!(
        "SELECT DISTINCT ON (kind) kind, version, body
         FROM config_documents
         WHERE status = 'published'
         ORDER BY kind, published_at DESC"
    )
    .fetch_all(&state.db)
    .await?;
    let doc = |kind: ConfigDocumentKind| -> RResult<EffectiveDocument> {
        match published.iter().find(|r| r.kind == kind_str(kind)) {
            Some(r) => Ok(EffectiveDocument {
                version: r.version.clone(),
                body: r.body.clone(),
            }),
            None => bundled_default(kind),
        }
    };
    Ok(Json(EffectiveConfig {
        source: EffectiveSource::Global,
        bundle_version: None,
        scoring_profile: doc(ConfigDocumentKind::ScoringProfile)?,
        normative_baselines: doc(ConfigDocumentKind::NormativeBaselines)?,
        stimulus_catalog: doc(ConfigDocumentKind::StimulusCatalog)?,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bands() -> Vec<AgeBracket> {
        asd_core::norms::default_age_brackets()
    }

    fn count(reason: Option<&str>, band: Option<&str>, frames: bool, n: u32) -> PreviewCount {
        PreviewCount {
            reason: reason.map(str::to_owned),
            band: band.map(str::to_owned),
            has_frames: frames,
            n,
        }
    }

    #[test]
    fn small_cell_k_defaults_and_bounds() {
        assert_eq!(small_cell_k(&json!({})), 12);
        assert_eq!(small_cell_k(&json!({"research": {"small_cell_k": 5}})), 5);
        assert_eq!(small_cell_k(&json!({"research": {"small_cell_k": 50}})), 50);
        assert_eq!(small_cell_k(&json!({"research": {"small_cell_k": 4}})), 12);
        assert_eq!(
            small_cell_k(&json!({"research": {"small_cell_k": "7"}})),
            12
        );
        assert_eq!(suppress_count(0, 12), Some(0));
        assert_eq!(suppress_count(11, 12), None);
        assert_eq!(suppress_count(12, 12), Some(12));
    }

    #[test]
    fn preview_counts_bands_levels_and_exclusions() {
        let b = bands();
        let (b0, b2) = (b[0].id.as_str(), b[2].id.as_str());
        let counts = [
            count(None, Some(b0), true, 3),
            count(None, Some(b0), false, 4),
            count(None, Some(b2), true, 12),
            count(None, Some(b2), false, 8),
            count(Some("no_consent"), Some(b2), false, 5),
            count(Some("withdrawn"), Some(b0), false, 1),
            count(Some("no_age"), None, false, 2),
            count(Some("quality"), Some(b2), true, 3),
            count(Some("unscorable"), Some(b0), false, 1),
        ];
        let p = assemble_preview(&CohortDefinition::default(), &b, &counts, 12);
        assert_eq!(p.n, Some(27));
        assert_eq!(
            p.by_level,
            LevelCounts {
                frames: 15,
                score: 12
            }
        );
        assert_eq!(
            p.excluded,
            CohortExclusions {
                no_consent: 5,
                withdrawn: 1,
                unscorable: 1,
                no_age: 2,
                quality: 3
            }
        );
        assert_eq!(p.by_age_band.len(), 5);
        // 7 < 12: suppressed, never shown as a number or a zero.
        assert_eq!(p.by_age_band[0].n, None);
        assert_eq!(p.by_age_band[0].frames_n, None);
        assert!(p.by_age_band[0].suppressed);
        assert_eq!(p.by_age_band[1].n, Some(0));
        assert!(!p.by_age_band[1].suppressed);
        assert_eq!(p.by_age_band[2].n, Some(20));
        assert_eq!(p.by_age_band[2].frames_n, Some(12));
        // k = 5: the 7-session band shows; its 3 frame sessions do not.
        let p = assemble_preview(&CohortDefinition::default(), &b, &counts, 5);
        assert_eq!(p.by_age_band[0].n, Some(7));
        assert_eq!(p.by_age_band[0].frames_n, None);
        assert_eq!(p.small_cell_k, 5);
    }

    #[test]
    fn preview_band_selection_drops_other_bands() {
        let b = bands();
        let def = CohortDefinition {
            age_bands: vec![b[2].id.clone()],
            ..Default::default()
        };
        let counts = [
            count(None, Some(&b[0].id), false, 30),
            count(None, Some(&b[2].id), false, 13),
            count(Some("no_consent"), Some(&b[0].id), false, 9),
            count(Some("no_age"), None, false, 1),
        ];
        let p = assemble_preview(&def, &b, &counts, 12);
        assert_eq!(p.n, Some(13));
        assert_eq!(p.by_age_band.len(), 1);
        assert_eq!(p.excluded.no_consent, 0);
        assert_eq!(p.excluded.no_age, 1);
    }

    #[test]
    fn definitions_are_checked() {
        let ok = CohortDefinition {
            from: Some("2026-01-01".into()),
            to: Some("2026-12-31".into()),
            ..Default::default()
        };
        assert!(check_definition(&ok, false).is_ok());
        let bad = CohortDefinition {
            from: Some("2026-12-31".into()),
            to: Some("2026-01-01".into()),
            device_ids: vec!["nope".into()],
            min_valid_ratio: Some(1.5),
            age_bands: vec!["a".into(), "a".into()],
            include_synthetic: true,
            ..Default::default()
        };
        let names: Vec<String> = check_definition(&bad, false)
            .unwrap_err()
            .into_iter()
            .map(|p| p.name)
            .collect();
        assert_eq!(
            names,
            [
                "definition.from",
                "definition.device_ids[0]",
                "definition.min_valid_ratio",
                "definition.age_bands[1]",
                "definition.include_synthetic"
            ]
        );
    }

    #[test]
    fn promote_notes_lead_with_the_evidence() {
        let (e, r) = (Uuid::nil(), Uuid::max());
        assert_eq!(promote_notes(e, r, None), format!("experiment:{e} run:{r}"));
        assert_eq!(
            promote_notes(e, r, Some(" tighter I-VT ")),
            format!("experiment:{e} run:{r}\ntighter I-VT")
        );
    }

    #[test]
    fn changed_params_name_registry_ids_only() {
        let a = serde_json::to_value(ScoringProfile::default()).unwrap();
        let mut b = a.clone();
        b["metrics"]["ivtVelocityUnitsPerSec"] = json!(2.0);
        b["flag"]["greenFloor"] = json!(0.7);
        assert_eq!(
            changed_params(&a, &b),
            ["flag.greenFloor", "metrics.ivtVelocityUnitsPerSec"]
        );
        assert!(changed_params(&a, &a).is_empty());
    }
}
