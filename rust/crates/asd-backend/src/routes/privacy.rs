//! owner: WP-B.9
//!
//! Erasure and retention routes (user plane, `ParticipantErase`: owner and admin, and the
//! superadmin, which is not a clinical-content action):
//!
//! - `GET /v1/orgs/{id}/erasure-requests` ([`ErasureRequestPage`], newest first, cursor);
//! - `POST /v1/orgs/{id}/participants/{participant_id}/erasure`: files an erasure request
//!   for the participant (as the device route does: consent withdrawn now, the `erasure`
//!   job queued), `202` with the [`ErasureRequest`], or `200` with the open one
//!   (`duplicate: true`). Another org's participant is a 404;
//! - `POST /v1/orgs/{id}/retention-sweep` ([`RetentionSweepRequest`], `dry_run` defaults to
//!   true): queues a `retention_sweep` job for the org, `202` with [`RetentionSweepJob`];
//!   the counts are the job's `result` (`GET /v1/jobs/{id}`, [`RetentionSweepResult`]).
//!
//! Every mutation writes one audit row in its transaction (AUTH-5).
//!
//! [`RetentionSweepResult`]: asd_contract::privacy::RetentionSweepResult

use asd_contract::privacy::{
    ErasureOutcome, ErasureRequest, ErasureRequestPage, ErasureStatus, RetentionSweepJob,
    RetentionSweepRequest,
};
use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use http::StatusCode;
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::app::AppState;
use crate::audit::AuditEvent;
use crate::auth::extractors::{ApiJson, RequestMeta, Require};
use crate::auth::rbac::perm;
use crate::error::{AppError, AppResult};
use crate::jobs::{self, erasure, retention, KIND_RETENTION_SWEEP};
use crate::problem::InvalidParam;
use crate::read::cursor::{self, Cursor};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/orgs/{id}/erasure-requests", get(list))
        .route(
            "/orgs/{id}/participants/{participant_id}/erasure",
            post(request),
        )
        .route("/orgs/{id}/retention-sweep", post(sweep))
}

/// One `erasure_requests` row joined with its participant's token.
struct Row {
    id: Uuid,
    org_id: Uuid,
    participant_id: Uuid,
    token: String,
    status: String,
    scope: String,
    requested_by_type: String,
    requested_at: DateTime<Utc>,
    executed_at: Option<DateTime<Utc>>,
    completed_at: Option<DateTime<Utc>>,
    hold_reason: Option<String>,
    outcome: Option<Value>,
}

impl Row {
    fn dto(self, duplicate: bool) -> AppResult<ErasureRequest> {
        Ok(ErasureRequest {
            id: self.id.to_string(),
            org_id: self.org_id.to_string(),
            participant_id: self.participant_id.to_string(),
            participant_token: self.token,
            status: ErasureStatus::parse(&self.status).ok_or_else(|| {
                AppError::Internal("stored erasure status is not in the contract".into())
            })?,
            scope: self.scope,
            requested_by_type: self.requested_by_type,
            requested_at: cursor::ts(self.requested_at),
            executed_at: self.executed_at.map(cursor::ts),
            completed_at: self.completed_at.map(cursor::ts),
            hold_reason: self.hold_reason,
            outcome: self
                .outcome
                .and_then(|o| serde_json::from_value::<ErasureOutcome>(o).ok()),
            duplicate,
        })
    }
}

#[derive(Debug, Default, Deserialize)]
struct ListQuery {
    cursor: Option<String>,
    limit: Option<String>,
}

fn invalid(name: &str, reason: &str) -> AppError {
    AppError::Validation(vec![InvalidParam {
        name: name.into(),
        reason: reason.into(),
    }])
}

async fn list(
    auth: Require<perm::ParticipantErase>,
    State(state): State<AppState>,
    Query(q): Query<ListQuery>,
) -> AppResult<Json<ErasureRequestPage>> {
    let org = auth.org();
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
        Some(c) => (
            Some(c.at),
            Some(Uuid::parse_str(&c.id).map_err(|_| invalid("cursor", "is not valid"))?),
        ),
        None => (None, None),
    };
    let rows = sqlx::query_as!(
        Row,
        r#"SELECT r.id, r.org_id, r.participant_id, p.token, r.status, r.scope,
                  r.requested_by_type, r.requested_at, r.executed_at, r.completed_at,
                  r.hold_reason, r.outcome
           FROM erasure_requests r JOIN participants p ON p.id = r.participant_id
           WHERE r.org_id = $1
             AND ($2::timestamptz IS NULL OR (r.requested_at, r.id) < ($2, $3::uuid))
           ORDER BY r.requested_at DESC, r.id DESC
           LIMIT $4"#,
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
        last = Some(Cursor::new(r.requested_at, r.id.to_string()));
        items.push(r.dto(false)?);
    }
    Ok(Json(ErasureRequestPage {
        items,
        next_cursor: if has_more {
            last.map(|c| c.encode())
        } else {
            None
        },
    }))
}

async fn load(conn: &mut sqlx::PgConnection, id: Uuid) -> AppResult<Row> {
    Ok(sqlx::query_as!(
        Row,
        r#"SELECT r.id, r.org_id, r.participant_id, p.token, r.status, r.scope,
                  r.requested_by_type, r.requested_at, r.executed_at, r.completed_at,
                  r.hold_reason, r.outcome
           FROM erasure_requests r JOIN participants p ON p.id = r.participant_id
           WHERE r.id = $1"#,
        id
    )
    .fetch_one(&mut *conn)
    .await?)
}

async fn request(
    auth: Require<perm::ParticipantErase>,
    State(state): State<AppState>,
    meta: RequestMeta,
    Path((_, raw)): Path<(String, String)>,
) -> AppResult<(StatusCode, Json<ErasureRequest>)> {
    let org = auth.org();
    let participant = Uuid::parse_str(&raw).map_err(|_| AppError::NotFound)?;
    let mut tx = state.db.begin().await?;
    // The row lock serialises concurrent requests for one participant (as the device route).
    sqlx::query_scalar!(
        "SELECT id FROM participants WHERE id = $1 AND org_id = $2 FOR UPDATE",
        participant,
        org,
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::NotFound)?;
    let open = sqlx::query_scalar!(
        "SELECT id FROM erasure_requests WHERE participant_id = $1 AND completed_at IS NULL",
        participant,
    )
    .fetch_optional(&mut *tx)
    .await?;
    if let Some(open) = open {
        let row = load(&mut tx, open).await?;
        tx.rollback().await?;
        return Ok((StatusCode::OK, Json(row.dto(true)?)));
    }
    let id = sqlx::query_scalar!(
        "INSERT INTO erasure_requests (org_id, participant_id, requested_by_type, requested_by)
         VALUES ($1, $2, 'user', $3) RETURNING id",
        org,
        participant,
        auth.principal.user_id,
    )
    .fetch_one(&mut *tx)
    .await?;
    sqlx::query!(
        "UPDATE participants SET withdrawn_at = coalesce(withdrawn_at, now()) WHERE id = $1",
        participant,
    )
    .execute(&mut *tx)
    .await?;
    let job = erasure::enqueue(&mut tx, org, id, Some(auth.principal.user_id)).await?;
    AuditEvent::user(
        &auth.principal,
        "participant.erasure_request",
        "participant",
        participant,
    )
    .org(org)
    .after(json!({"erasure_request_id": id, "scope": "all", "job_id": job}))
    .meta(&meta)
    .record(&mut tx)
    .await?;
    let row = load(&mut tx, id).await?;
    tx.commit().await?;
    Ok((StatusCode::ACCEPTED, Json(row.dto(false)?)))
}

async fn sweep(
    auth: Require<perm::ParticipantErase>,
    State(state): State<AppState>,
    meta: RequestMeta,
    ApiJson(req): ApiJson<RetentionSweepRequest>,
) -> AppResult<(StatusCode, Json<RetentionSweepJob>)> {
    let org = auth.org();
    let mut tx = state.db.begin().await?;
    let job = jobs::enqueue(
        &mut tx,
        KIND_RETENTION_SWEEP,
        Some(org),
        &retention::params(req.dry_run, false),
        Some(auth.principal.user_id),
    )
    .await?;
    AuditEvent::user(
        &auth.principal,
        "retention.sweep_request",
        "organization",
        org,
    )
    .org(org)
    .after(json!({"job_id": job.id, "dry_run": req.dry_run}))
    .meta(&meta)
    .record(&mut tx)
    .await?;
    tx.commit().await?;
    Ok((
        StatusCode::ACCEPTED,
        Json(RetentionSweepJob {
            job_id: job.id.to_string(),
            org_id: org.to_string(),
            dry_run: req.dry_run,
            status: job.status,
            created_at: cursor::ts(job.created_at),
        }),
    ))
}
