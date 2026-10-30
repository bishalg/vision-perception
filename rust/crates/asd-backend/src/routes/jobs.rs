//! owner: WP-B.5
//!
//! Norm aggregation endpoints (API-2, 02 §4):
//!
//! - `POST /v1/jobs/norm-aggregate` `{min_n=12, include_imputed=false, source_filter}` queues
//!   a job and answers `202` with it; the worker (`asd-backend worker`) runs it;
//! - `GET /v1/jobs/{id}` the job (status, `result.norm_run_id` once it succeeded);
//! - `GET /v1/norm-runs` (cursor list, newest first) and `GET /v1/norm-runs/{id}` (with the
//!   candidate `cells`);
//! - `POST /v1/norm-runs/{id}/promote` writes the run as a **draft** `normative_baselines`
//!   config document. It is never published here.
//!
//! Queueing, listing and reading need `perm::JobRun`; promoting needs `perm::ConfigWrite`
//! (it creates a config document). A run or job of another org is a 404. Every mutation
//! adds one audit row in its own transaction (AUTH-5).

use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use http::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::app::AppState;
use crate::audit::AuditEvent;
use crate::auth::extractors::{ApiJson, RequestMeta, Require};
use crate::auth::rbac::perm;
use crate::error::{AppError, AppResult};
use crate::jobs::norm_aggregate::{self, NormParams};
use crate::jobs::{self, Job};
use crate::problem::InvalidParam;
use crate::read::cursor::{self, Cursor};
use crate::routes::sessions::{path_uuid, query_or_problem};

use asd_core::norms::{self, CandidateBaselineSet};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/jobs/norm-aggregate", post(enqueue_norm_aggregate))
        .route("/jobs/{id}", get(get_job))
        .route("/norm-runs", get(list_runs))
        .route("/norm-runs/{id}", get(get_run))
        .route("/norm-runs/{id}/promote", post(promote_run))
}

// ---- wire types ------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct JobView {
    pub id: String,
    pub kind: String,
    pub status: String,
    pub params: Value,
    pub result: Option<Value>,
    pub attempts: i32,
    pub org_id: Option<String>,
    pub created_by: Option<String>,
    pub created_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub error: Option<String>,
}

impl From<Job> for JobView {
    fn from(j: Job) -> Self {
        Self {
            id: j.id.to_string(),
            kind: j.kind,
            status: j.status,
            params: j.params,
            result: j.result,
            attempts: j.attempts,
            org_id: j.org_id.map(|u| u.to_string()),
            created_by: j.created_by.map(|u| u.to_string()),
            created_at: cursor::ts(j.created_at),
            started_at: j.started_at.map(cursor::ts),
            finished_at: j.finished_at.map(cursor::ts),
            error: j.error,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct NormRunSummary {
    pub id: String,
    pub job_id: Option<String>,
    pub org_id: Option<String>,
    pub baselines_source_id: Option<String>,
    pub params: Value,
    pub n_total: i32,
    pub qualifying_cells: usize,
    pub total_cells: usize,
    pub created_by: Option<String>,
    pub created_at: String,
    pub promoted_to_config_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NormRunDetail {
    #[serde(flatten)]
    pub summary: NormRunSummary,
    /// The `CandidateBaselineSet` (cells withheld by CORE-8 are `null`).
    pub cells: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct NormRunPage {
    pub items: Vec<NormRunSummary>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PromoteResponse {
    pub norm_run_id: String,
    pub config_document_id: String,
    pub kind: &'static str,
    pub version: String,
    pub status: &'static str,
}

fn invalid(name: &str, reason: &str) -> AppError {
    AppError::Validation(vec![InvalidParam {
        name: name.into(),
        reason: reason.into(),
    }])
}

// ---- enqueue ---------------------------------------------------------------------------

async fn enqueue_norm_aggregate(
    auth: Require<perm::JobRun>,
    meta: RequestMeta,
    State(state): State<AppState>,
    ApiJson(mut params): ApiJson<NormParams>,
) -> AppResult<(StatusCode, Json<JobView>)> {
    let user = auth.principal;
    // Org users aggregate their own org only; naming another org is a 404 (AUTH-4).
    if let Some(own) = user.org_id {
        match params.source_filter.org_id {
            Some(requested) if requested != own => return Err(AppError::NotFound),
            _ => params.source_filter.org_id = Some(own),
        }
    } else if let Some(requested) = params.source_filter.org_id {
        let exists = sqlx::query_scalar!(
            r#"SELECT 1 AS "one!: i32" FROM organizations WHERE id = $1"#,
            requested
        )
        .fetch_optional(&state.db)
        .await?;
        if exists.is_none() {
            return Err(AppError::NotFound);
        }
    }
    params.validate().map_err(AppError::Validation)?;
    if params.source_filter.include_synthetic && !state.config.allow_synthetic {
        return Err(invalid(
            "source_filter.include_synthetic",
            "this server does not accept synthetic data (ASD_ALLOW_SYNTHETIC is off)",
        ));
    }
    let org = params.source_filter.org_id;
    let stored = serde_json::to_value(&params)
        .map_err(|_| AppError::Internal("norm params do not serialise".into()))?;

    let mut tx = state.db.begin().await?;
    let job = jobs::enqueue(
        &mut tx,
        jobs::KIND_NORM_AGGREGATE,
        org,
        &stored,
        Some(user.user_id),
    )
    .await?;
    let mut audit = AuditEvent::user(&user, "job.norm_aggregate.enqueue", "job", job.id)
        .after(json!({"kind": job.kind, "params": stored}))
        .meta(&meta);
    if let Some(org) = org {
        audit = audit.org(org);
    }
    audit.record(&mut tx).await?;
    tx.commit().await?;
    Ok((StatusCode::ACCEPTED, Json(job.into())))
}

async fn get_job(
    auth: Require<perm::JobRun>,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<JobView>> {
    let id = path_uuid(&id)?;
    let mut conn = state.db.acquire().await?;
    let job = jobs::get(&mut conn, id).await?.ok_or(AppError::NotFound)?;
    if !norm_aggregate::run_visible(&auth.principal, job.org_id) {
        return Err(AppError::NotFound);
    }
    Ok(Json(job.into()))
}

// ---- runs ------------------------------------------------------------------------------

/// `(qualifying, total)` cell counts, computed by `asd-core` from the stored candidate.
fn cell_counts(cells: &Value) -> (usize, usize) {
    serde_json::from_value::<CandidateBaselineSet>(cells.clone())
        .map(|set| {
            (
                norms::qualifying_cells(&set),
                norm_aggregate::total_cells(&set),
            )
        })
        .unwrap_or((0, 0))
}

#[allow(clippy::too_many_arguments)]
fn summary(
    id: Uuid,
    job_id: Option<Uuid>,
    org_id: Option<Uuid>,
    baselines_source_id: Option<Uuid>,
    params: Value,
    cells: &Value,
    n_total: i32,
    created_by: Option<Uuid>,
    created_at: DateTime<Utc>,
    promoted: Option<Uuid>,
) -> NormRunSummary {
    let (qualifying_cells, total_cells) = cell_counts(cells);
    NormRunSummary {
        id: id.to_string(),
        job_id: job_id.map(|u| u.to_string()),
        org_id: org_id.map(|u| u.to_string()),
        baselines_source_id: baselines_source_id.map(|u| u.to_string()),
        params,
        n_total,
        qualifying_cells,
        total_cells,
        created_by: created_by.map(|u| u.to_string()),
        created_at: cursor::ts(created_at),
        promoted_to_config_id: promoted.map(|u| u.to_string()),
    }
}

#[derive(Debug, Default, Deserialize)]
struct RunQuery {
    cursor: Option<String>,
    limit: Option<String>,
}

async fn list_runs(
    auth: Require<perm::JobRun>,
    State(state): State<AppState>,
    query: Result<Query<RunQuery>, axum::extract::rejection::QueryRejection>,
) -> AppResult<Json<NormRunPage>> {
    let q = query_or_problem(query)?;
    let limit = match q.limit.filter(|s| !s.is_empty()) {
        Some(raw) => cursor::limit(Some(
            raw.parse()
                .map_err(|_| invalid("limit", "must be an integer"))?,
        ))?,
        None => cursor::limit(None)?,
    };
    let after = q
        .cursor
        .filter(|s| !s.is_empty())
        .map(|c| Cursor::decode(&c))
        .transpose()?;
    let (after_at, after_id) = match after {
        Some(c) => (Some(c.at), Some(c.id)),
        None => (None, None),
    };
    // A superadmin sees every run; everyone else only their org's.
    let scoped = !auth.principal.is_superadmin();
    let org = auth.principal.org_id;

    let rows = sqlx::query!(
        r#"SELECT id, job_id, org_id, baselines_source_id, params, cells, n_total, created_by,
                  created_at, promoted_to_config_id
           FROM norm_runs
           WHERE ($1::bool = false OR org_id = $2)
             AND ($3::timestamptz IS NULL OR (created_at, id::text) < ($3, $4::text))
           ORDER BY created_at DESC, id::text DESC
           LIMIT $5"#,
        scoped,
        org,
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
        items.push(summary(
            r.id,
            r.job_id,
            r.org_id,
            r.baselines_source_id,
            r.params,
            &r.cells,
            r.n_total,
            r.created_by,
            r.created_at,
            r.promoted_to_config_id,
        ));
    }
    Ok(Json(NormRunPage {
        items,
        next_cursor: if has_more {
            last.map(|c| c.encode())
        } else {
            None
        },
    }))
}

async fn get_run(
    auth: Require<perm::JobRun>,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<NormRunDetail>> {
    let id = path_uuid(&id)?;
    let r = sqlx::query!(
        r#"SELECT id, job_id, org_id, baselines_source_id, params, cells, n_total, created_by,
                  created_at, promoted_to_config_id
           FROM norm_runs WHERE id = $1"#,
        id,
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;
    if !norm_aggregate::run_visible(&auth.principal, r.org_id) {
        return Err(AppError::NotFound);
    }
    let summary = summary(
        r.id,
        r.job_id,
        r.org_id,
        r.baselines_source_id,
        r.params,
        &r.cells,
        r.n_total,
        r.created_by,
        r.created_at,
        r.promoted_to_config_id,
    );
    Ok(Json(NormRunDetail {
        summary,
        cells: r.cells,
    }))
}

async fn promote_run(
    auth: Require<perm::ConfigWrite>,
    meta: RequestMeta,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<(StatusCode, Json<PromoteResponse>)> {
    let id = path_uuid(&id)?;
    let mut tx = state.db.begin().await?;
    let promoted = norm_aggregate::promote(&mut tx, id, &auth.principal, &meta).await?;
    tx.commit().await?;
    Ok((
        StatusCode::CREATED,
        Json(PromoteResponse {
            norm_run_id: id.to_string(),
            config_document_id: promoted.config_document_id.to_string(),
            kind: "normative_baselines",
            version: promoted.version,
            status: "draft",
        }),
    ))
}
