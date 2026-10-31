//! owner: WP-B.3
//!
//! Review queue (API-2, ING-3): `GET /v1/orgs/{id}/review` (filters `status`, `reason`,
//! cursor over `(created_at, id)`), `POST /v1/orgs/{id}/review` (manual enqueue, one per
//! session) and `PATCH /v1/review/{id}` (status, assignee, decision, labels). Items are
//! created automatically at ingest (`ingest::auto_review`). Reads need
//! `perm::ReviewRead`, writes `perm::ReviewWrite` (the `auth::rbac` matrix); each write adds
//! one audit row in the same transaction (AUTH-5).
//!
//! KEY-7 (WP-C.9): the optional manual-enqueue note is stored encrypted (`note_ct`,
//! `note_key_id`, AAD `review_items:note_ct:{org}:{id}`); the plaintext `note` column is
//! never written. A note without a configured `ASD_DATA_KEK` answers
//! `503 clinical-crypto-unavailable` and stores nothing. The note is still never returned.

use asd_contract::api_v1::{
    CreateReviewRequest, ReviewItem, ReviewPage, ReviewReason, ReviewSessionSummary, ReviewStatus,
    UpdateReviewRequest,
};
use axum::extract::rejection::{JsonRejection, QueryRejection};
use axum::extract::{Path, Query, State};
use axum::routing::{get, patch};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use http::StatusCode;
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::app::AppState;
use crate::audit::AuditEvent;
use crate::auth::extractors::{RequestMeta, Require};
use crate::auth::rbac::perm;
use crate::crypto::{Aad, ClinicalCrypto};
use crate::error::{AppError, AppResult};
use crate::ingest::auto_review::reason_str;
use crate::problem::InvalidParam;
use crate::read::cursor::{self, Cursor};
use crate::routes::sessions::{path_uuid, query_or_problem};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/orgs/{id}/review", get(list).post(create))
        .route("/review/{id}", patch(update))
}

fn invalid(name: &str, reason: &str) -> AppError {
    AppError::Validation(vec![InvalidParam {
        name: name.into(),
        reason: reason.into(),
    }])
}

pub(crate) fn json_or_problem<T>(body: Result<Json<T>, JsonRejection>) -> AppResult<T> {
    body.map(|Json(t)| t)
        .map_err(|e| invalid("body", &e.body_text()))
}

fn status_str(s: ReviewStatus) -> &'static str {
    match s {
        ReviewStatus::Open => "open",
        ReviewStatus::InReview => "in_review",
        ReviewStatus::Done => "done",
        ReviewStatus::Dismissed => "dismissed",
    }
}

fn parse<T: serde::de::DeserializeOwned>(s: String) -> AppResult<T> {
    serde_json::from_value(Value::String(s))
        .map_err(|_| AppError::Internal("stored review value is not in the contract".into()))
}

#[allow(clippy::too_many_arguments)]
fn item(
    id: Uuid,
    org_id: Uuid,
    session_id: String,
    reason: String,
    status: String,
    assigned_to: Option<Uuid>,
    decision: Option<Value>,
    decided_by: Option<Uuid>,
    decided_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
) -> AppResult<ReviewItem> {
    Ok(ReviewItem {
        id: id.to_string(),
        org_id: org_id.to_string(),
        session_id,
        reason: parse(reason)?,
        status: parse(status)?,
        assigned_to: assigned_to.map(|u| u.to_string()),
        decision,
        decided_by: decided_by.map(|u| u.to_string()),
        decided_at: decided_at.map(cursor::ts),
        created_at: cursor::ts(created_at),
        session: None,
    })
}

#[derive(Debug, Default, Deserialize)]
struct ReviewQuery {
    status: Option<String>,
    reason: Option<String>,
    cursor: Option<String>,
    limit: Option<String>,
}

async fn list(
    auth: Require<perm::ReviewRead>,
    State(state): State<AppState>,
    query: Result<Query<ReviewQuery>, QueryRejection>,
) -> AppResult<Json<ReviewPage>> {
    let org = auth.org();
    let q = query_or_problem(query)?;
    let status = match q.status.filter(|s| !s.is_empty()) {
        Some(s) => Some(status_str(
            serde_json::from_value::<ReviewStatus>(Value::String(s))
                .map_err(|_| invalid("status", "must be open, in_review, done or dismissed"))?,
        )),
        None => None,
    };
    let reason = match q.reason.filter(|s| !s.is_empty()) {
        Some(s) => Some(reason_str(
            serde_json::from_value::<ReviewReason>(Value::String(s))
                .map_err(|_| invalid("reason", "unknown review reason"))?,
        )),
        None => None,
    };
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

    let rows = sqlx::query!(
        r#"SELECT r.id, r.org_id, r.session_id, r.reason, r.status, r.assigned_to, r.decision,
                  r.decided_by, r.decided_at, r.created_at,
                  s.attention_flag AS "s_flag?", s.quality_verdict AS "s_verdict?",
                  s.participant_token AS "s_token?", s.started_at AS "s_started?"
           FROM review_items r
           LEFT JOIN sessions s ON s.id = r.session_id AND s.org_id = r.org_id
           WHERE r.org_id = $1
             AND ($2::text IS NULL OR r.status = $2)
             AND ($3::text IS NULL OR r.reason = $3)
             AND ($4::timestamptz IS NULL OR (r.created_at, r.id::text) < ($4, $5::text))
           ORDER BY r.created_at DESC, r.id::text DESC
           LIMIT $6"#,
        org,
        status,
        reason,
        after_at,
        after_id,
        limit + 1,
    )
    .fetch_all(&state.db)
    .await?;
    let total = sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!" FROM review_items
           WHERE org_id = $1 AND ($2::text IS NULL OR status = $2)
             AND ($3::text IS NULL OR reason = $3)"#,
        org,
        status,
        reason,
    )
    .fetch_one(&state.db)
    .await?;

    let limit = usize::try_from(limit).unwrap_or(usize::MAX);
    let has_more = rows.len() > limit;
    let mut items = Vec::new();
    let mut last = None;
    for r in rows.into_iter().take(limit) {
        last = Some(Cursor::new(r.created_at, r.id.to_string()));
        let session = match (r.s_flag, r.s_verdict, r.s_token, r.s_started) {
            (Some(flag), Some(quality_verdict), Some(participant_token), Some(started)) => {
                Some(ReviewSessionSummary {
                    attention_flag: parse(flag)?,
                    quality_verdict,
                    participant_token,
                    started_at: cursor::ts(started),
                })
            }
            _ => None,
        };
        let mut it = item(
            r.id,
            r.org_id,
            r.session_id,
            r.reason,
            r.status,
            r.assigned_to,
            r.decision,
            r.decided_by,
            r.decided_at,
            r.created_at,
        )?;
        it.session = session;
        items.push(it);
    }
    Ok(Json(ReviewPage {
        items,
        next_cursor: if has_more {
            last.map(|c| c.encode())
        } else {
            None
        },
        total: Some(u64::try_from(total).unwrap_or(0)),
    }))
}

/// Manual enqueue: 201 with the new item, 200 with the existing manual item.
async fn create(
    auth: Require<perm::ReviewWrite>,
    meta: RequestMeta,
    State(state): State<AppState>,
    body: Result<Json<CreateReviewRequest>, JsonRejection>,
) -> AppResult<(StatusCode, Json<ReviewItem>)> {
    let org = auth.org();
    let user = auth.principal;
    let req = json_or_problem(body)?;

    // The row id is part of the AAD, so it is chosen before the insert (trap 9).
    let id = Uuid::new_v4();
    let note = match req.note.as_deref().filter(|n| !n.trim().is_empty()) {
        Some(text) => {
            let crypto = ClinicalCrypto::from_state(&state)?;
            let aad = Aad::new("review_items", "note_ct", org, id);
            Some(crypto.encrypt_str(org, &aad, text).await?)
        }
        None => None,
    };
    let (note_ct, note_key_id) = match note {
        Some(sealed) => (Some(sealed.ciphertext), Some(sealed.key_id)),
        None => (None, None),
    };

    let mut tx = state.db.begin().await?;
    let exists = sqlx::query_scalar!(
        r#"SELECT 1 AS "one!: i32" FROM sessions WHERE id = $1 AND org_id = $2"#,
        req.session_id,
        org,
    )
    .fetch_optional(&mut *tx)
    .await?;
    if exists.is_none() {
        return Err(AppError::NotFound);
    }
    let created = sqlx::query!(
        r#"INSERT INTO review_items (id, org_id, session_id, reason, note_ct, note_key_id,
                                     created_by)
           VALUES ($1, $2, $3, 'manual', $4, $5, $6)
           ON CONFLICT (session_id, reason) DO NOTHING
           RETURNING id, org_id, session_id, reason, status, assigned_to, decision,
                     decided_by, decided_at, created_at"#,
        id,
        org,
        req.session_id,
        note_ct,
        note_key_id,
        user.user_id,
    )
    .fetch_optional(&mut *tx)
    .await?;
    let (status, r) = match created {
        Some(r) => {
            AuditEvent::user(&user, "review.create", "review_item", r.id)
                .org(org)
                .after(json!({"session_id": r.session_id, "reason": "manual"}))
                .meta(&meta)
                .record(&mut tx)
                .await?;
            (
                StatusCode::CREATED,
                item(
                    r.id,
                    r.org_id,
                    r.session_id,
                    r.reason,
                    r.status,
                    r.assigned_to,
                    r.decision,
                    r.decided_by,
                    r.decided_at,
                    r.created_at,
                )?,
            )
        }
        None => {
            let r = sqlx::query!(
                r#"SELECT id, org_id, session_id, reason, status, assigned_to, decision,
                          decided_by, decided_at, created_at
                   FROM review_items WHERE session_id = $1 AND reason = 'manual'"#,
                req.session_id,
            )
            .fetch_one(&mut *tx)
            .await?;
            (
                StatusCode::OK,
                item(
                    r.id,
                    r.org_id,
                    r.session_id,
                    r.reason,
                    r.status,
                    r.assigned_to,
                    r.decision,
                    r.decided_by,
                    r.decided_at,
                    r.created_at,
                )?,
            )
        }
    };
    tx.commit().await?;
    Ok((status, Json(r)))
}

async fn update(
    auth: Require<perm::ReviewWrite>,
    meta: RequestMeta,
    State(state): State<AppState>,
    Path(id): Path<String>,
    body: Result<Json<UpdateReviewRequest>, JsonRejection>,
) -> AppResult<Json<ReviewItem>> {
    let user = auth.principal;
    let id = path_uuid(&id)?;
    let mut tx = state.db.begin().await?;
    let current = sqlx::query!(
        "SELECT org_id, session_id FROM review_items WHERE id = $1 FOR UPDATE",
        id
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::NotFound)?;
    auth.ensure_org(current.org_id)?;
    let req = json_or_problem(body)?;

    let assigned_to = match &req.assigned_to {
        Some(raw) => {
            let uid = Uuid::parse_str(raw)
                .map_err(|_| invalid("assigned_to", "must be a user id of this org"))?;
            let member = sqlx::query_scalar!(
                r#"SELECT 1 AS "one!: i32" FROM users WHERE id = $1 AND org_id = $2"#,
                uid,
                current.org_id,
            )
            .fetch_optional(&mut *tx)
            .await?;
            if member.is_none() {
                return Err(invalid("assigned_to", "must be a user id of this org"));
            }
            Some(uid)
        }
        None => None,
    };
    let status = req.status.map(status_str);

    let r = sqlx::query!(
        r#"UPDATE review_items SET
               status      = COALESCE($2, status),
               assigned_to = COALESCE($3, assigned_to),
               decision    = COALESCE($4, decision),
               decided_by  = CASE WHEN $2 IN ('done', 'dismissed') THEN $5
                                  WHEN $2 IN ('open', 'in_review') THEN NULL
                                  ELSE decided_by END,
               decided_at  = CASE WHEN $2 IN ('done', 'dismissed') THEN now()
                                  WHEN $2 IN ('open', 'in_review') THEN NULL
                                  ELSE decided_at END
           WHERE id = $1
           RETURNING id, org_id, session_id, reason, status, assigned_to, decision,
                     decided_by, decided_at, created_at"#,
        id,
        status,
        assigned_to,
        req.decision,
        user.user_id,
    )
    .fetch_one(&mut *tx)
    .await?;

    for label in &req.labels {
        sqlx::query!(
            "INSERT INTO labels (session_id, clip_id, key, value, created_by) VALUES ($1, $2, $3, $4, $5)",
            current.session_id,
            label.clip_id,
            label.key,
            label.value,
            user.user_id,
        )
        .execute(&mut *tx)
        .await?;
    }
    AuditEvent::user(&user, "review.update", "review_item", id)
        .org(current.org_id)
        .after(json!({"status": r.status, "labels": req.labels.len()}))
        .meta(&meta)
        .record(&mut tx)
        .await?;
    let out = item(
        r.id,
        r.org_id,
        r.session_id,
        r.reason,
        r.status,
        r.assigned_to,
        r.decision,
        r.decided_by,
        r.decided_at,
        r.created_at,
    )?;
    tx.commit().await?;
    Ok(Json(out))
}
