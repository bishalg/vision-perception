//! owner: WP-C.11
//!
//! Clinical notes (NOTE-1, NOTE-2):
//!
//! - `POST /v1/participants/{id}/notes` (`ClinicalNoteWrite`)
//! - `GET /v1/participants/{id}/notes` and `GET /v1/notes/{id}` (`ClinicalContentRead`)
//!
//! The body (1..=20 000 characters) is encrypted into `body_ct` with the AAD
//! `clinical_notes:body_ct:{org}:{note id}`; the id is generated first (trap 9). Notes are
//! append-only: there is no edit or delete route and a trigger refuses both. A correction
//! is a new note of kind `correction` whose `corrects_id` names a note of the same
//! participant; the corrected note reports `corrected_by` (the latest correction). The audit
//! row of a create names the note and its links, never the text.

use asd_contract::clinical::{
    ClinicalNote, ClinicalNoteKind, ClinicalNotePage, CreateClinicalNoteRequest, MAX_NOTE_CHARS,
};
use axum::extract::rejection::QueryRejection;
use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use http::StatusCode;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::app::AppState;
use crate::audit::AuditEvent;
use crate::auth::extractors::{ApiJson, RequestMeta, Require};
use crate::auth::rbac::perm;
use crate::crypto::{Aad, ClinicalCrypto};
use crate::error::{AppError, AppResult};
use crate::problem::InvalidParam;
use crate::read::cursor::{self, Cursor};
use crate::routes::participant_detail::{page_query, participant_org, PageQuery};
use crate::routes::sessions::path_uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/participants/{id}/notes", post(create).get(list))
        .route("/notes/{id}", get(one))
}

fn invalid(name: &str, reason: &str) -> AppError {
    AppError::Validation(vec![InvalidParam {
        name: name.into(),
        reason: reason.into(),
    }])
}

struct NoteRow {
    id: Uuid,
    org_id: Uuid,
    participant_id: Uuid,
    session_id: Option<String>,
    kind: String,
    body_ct: Vec<u8>,
    body_key_id: Uuid,
    corrects_id: Option<Uuid>,
    author_id: Uuid,
    author_name: Option<String>,
    created_at: DateTime<Utc>,
    corrected_by: Option<Uuid>,
}

impl NoteRow {
    async fn into_note(self, crypto: &ClinicalCrypto, org: Uuid) -> AppResult<ClinicalNote> {
        let aad = Aad::new("clinical_notes", "body_ct", org, self.id);
        let body = crypto
            .decrypt_string(org, &aad, self.body_key_id, &self.body_ct)
            .await?;
        Ok(ClinicalNote {
            id: self.id.to_string(),
            participant_id: self.participant_id.to_string(),
            session_id: self.session_id,
            kind: ClinicalNoteKind::parse(&self.kind)
                .ok_or_else(|| AppError::Internal("unknown note kind".into()))?,
            body: Some(body),
            corrects_id: self.corrects_id.map(|u| u.to_string()),
            corrected_by: self.corrected_by.map(|u| u.to_string()),
            author_id: self.author_id.to_string(),
            author_display_name: self.author_name,
            created_at: cursor::ts(self.created_at),
        })
    }
}

async fn note_by_id(db: &PgPool, id: Uuid) -> AppResult<Option<NoteRow>> {
    Ok(sqlx::query_as!(
        NoteRow,
        r#"SELECT n.id, n.org_id, n.participant_id, n.session_id, n.kind, n.body_ct, n.body_key_id,
                  n.corrects_id, n.author_id, u.display_name AS "author_name?", n.created_at,
                  (SELECT c.id FROM clinical_notes c WHERE c.corrects_id = n.id
                   ORDER BY c.created_at DESC, c.id DESC LIMIT 1) AS "corrected_by?"
           FROM clinical_notes n
           JOIN users u ON u.id = n.author_id
           WHERE n.id = $1"#,
        id
    )
    .fetch_optional(db)
    .await?)
}

async fn create(
    auth: Require<perm::ClinicalNoteWrite>,
    crypto: ClinicalCrypto,
    State(state): State<AppState>,
    meta: RequestMeta,
    Path(raw_id): Path<String>,
    ApiJson(req): ApiJson<CreateClinicalNoteRequest>,
) -> AppResult<(StatusCode, Json<ClinicalNote>)> {
    let (participant, org) = participant_org(&state.db, &raw_id).await?;
    auth.ensure_org(org)?;

    if req.body.trim().is_empty() {
        return Err(invalid("body", "must not be empty"));
    }
    if req.body.chars().count() > MAX_NOTE_CHARS {
        return Err(invalid(
            "body",
            &format!("must be at most {MAX_NOTE_CHARS} characters"),
        ));
    }
    if let Some(session) = &req.session_id {
        let ok = sqlx::query_scalar!(
            r#"SELECT EXISTS (SELECT 1 FROM sessions
                 WHERE id = $1 AND participant_id = $2 AND org_id = $3) AS "ok!""#,
            session,
            participant,
            org
        )
        .fetch_one(&state.db)
        .await?;
        if !ok {
            return Err(invalid(
                "session_id",
                "is not a session of this participant",
            ));
        }
    }
    let corrects = match &req.corrects_id {
        Some(raw) => {
            let id = Uuid::parse_str(raw).map_err(|_| invalid("corrects_id", "must be a UUID"))?;
            let ok = sqlx::query_scalar!(
                r#"SELECT EXISTS (SELECT 1 FROM clinical_notes
                     WHERE id = $1 AND participant_id = $2 AND org_id = $3) AS "ok!""#,
                id,
                participant,
                org
            )
            .fetch_one(&state.db)
            .await?;
            if !ok {
                return Err(invalid("corrects_id", "is not a note of this participant"));
            }
            Some(id)
        }
        None => None,
    };
    let kind = if corrects.is_some() {
        ClinicalNoteKind::Correction
    } else {
        ClinicalNoteKind::General
    };

    let id = Uuid::new_v4();
    let aad = Aad::new("clinical_notes", "body_ct", org, id);
    let sealed = crypto.encrypt_str(org, &aad, &req.body).await?;

    let mut tx = state.db.begin().await?;
    sqlx::query!(
        r#"INSERT INTO clinical_notes
             (id, org_id, participant_id, session_id, kind, body_ct, body_key_id, corrects_id,
              author_id)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)"#,
        id,
        org,
        participant,
        req.session_id,
        kind.as_str(),
        sealed.ciphertext,
        sealed.key_id,
        corrects,
        auth.principal.user_id,
    )
    .execute(&mut *tx)
    .await?;
    AuditEvent::user(&auth.principal, "note.create", "clinical_note", id)
        .org(org)
        .after(json!({
            "participant_id": participant,
            "kind": kind.as_str(),
            "session_id": req.session_id,
            "corrects_id": corrects,
        }))
        .meta(&meta)
        .record(&mut tx)
        .await?;
    tx.commit().await?;

    let row = note_by_id(&state.db, id).await?.ok_or(AppError::NotFound)?;
    Ok((
        StatusCode::CREATED,
        Json(row.into_note(&crypto, org).await?),
    ))
}

async fn one(
    auth: Require<perm::ClinicalContentRead>,
    crypto: ClinicalCrypto,
    State(state): State<AppState>,
    Path(raw_id): Path<String>,
) -> AppResult<Json<ClinicalNote>> {
    let id = path_uuid(&raw_id)?;
    let row = note_by_id(&state.db, id).await?.ok_or(AppError::NotFound)?;
    auth.ensure_org(row.org_id)?;
    let org = row.org_id;
    Ok(Json(row.into_note(&crypto, org).await?))
}

async fn list(
    auth: Require<perm::ClinicalContentRead>,
    crypto: ClinicalCrypto,
    State(state): State<AppState>,
    Path(raw_id): Path<String>,
    query: Result<Query<PageQuery>, QueryRejection>,
) -> AppResult<Json<ClinicalNotePage>> {
    let (participant, org) = participant_org(&state.db, &raw_id).await?;
    auth.ensure_org(org)?;
    let (limit, after) = page_query(query)?.parse()?;
    let (after_at, after_id) = match after {
        Some(c) => {
            let id = Uuid::parse_str(&c.id)
                .map_err(|_| invalid("cursor", "not a cursor from this API"))?;
            (Some(c.at), Some(id))
        }
        None => (None, None),
    };
    let rows = sqlx::query_as!(
        NoteRow,
        r#"SELECT n.id, n.org_id, n.participant_id, n.session_id, n.kind, n.body_ct, n.body_key_id,
                  n.corrects_id, n.author_id, u.display_name AS "author_name?", n.created_at,
                  (SELECT c.id FROM clinical_notes c WHERE c.corrects_id = n.id
                   ORDER BY c.created_at DESC, c.id DESC LIMIT 1) AS "corrected_by?"
           FROM clinical_notes n
           JOIN users u ON u.id = n.author_id
           WHERE n.participant_id = $1 AND n.org_id = $2
             AND ($3::timestamptz IS NULL OR (n.created_at, n.id) < ($3, $4::uuid))
           ORDER BY n.created_at DESC, n.id DESC
           LIMIT $5"#,
        participant,
        org,
        after_at,
        after_id,
        limit + 1,
    )
    .fetch_all(&state.db)
    .await?;
    let total = sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!" FROM clinical_notes WHERE participant_id = $1 AND org_id = $2"#,
        participant,
        org
    )
    .fetch_one(&state.db)
    .await?;

    let limit = usize::try_from(limit).unwrap_or(usize::MAX);
    let has_more = rows.len() > limit;
    let mut last = None;
    let mut items = Vec::with_capacity(rows.len().min(limit));
    for row in rows.into_iter().take(limit) {
        last = Some(Cursor::new(row.created_at, row.id.to_string()));
        items.push(row.into_note(&crypto, org).await?);
    }
    Ok(Json(ClinicalNotePage {
        items,
        next_cursor: if has_more {
            last.map(|c| c.encode())
        } else {
            None
        },
        total: Some(u64::try_from(total).unwrap_or(0)),
    }))
}
