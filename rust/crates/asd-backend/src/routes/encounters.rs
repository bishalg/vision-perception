//! owner: WP-B.19
//!
//! Encounters (REC-3, REC-4): visits, calls and their follow-ups.
//!
//! - `POST /v1/participants/{id}/encounters` (`EncounterWrite`); a `summary` also needs
//!   `ClinicalNoteWrite`, else `400 encounter-summary-forbidden`.
//! - `GET /v1/participants/{id}/encounters` and `GET /v1/encounters/{id}`
//!   (`ParticipantRead`): the summary only with `ClinicalContentRead`, otherwise
//!   `has_summary` alone. Returning a summary writes one `encounter.read` row first (HIP-1).
//! - `PATCH /v1/encounters/{id}` (`EncounterWrite`, `If-Match: "<row_version>"`, `412
//!   stale-version`). A `closed` encounter accepts only `follow_up_done_at` and a reopen
//!   (`status: open`) by `owner` or `clinician`; anything else is `409 encounter-closed`.
//!   For the optional text fields an empty string clears the value.
//! - `POST /v1/encounters/{id}/sessions {session_id}` and `DELETE
//!   /v1/encounters/{id}/sessions/{session_id}` (`EncounterWrite`) link and unlink a session
//!   of the same participant (`sessions.encounter_id`).
//!
//! The summary is encrypted into `summary_ct` with the AAD `encounters:summary_ct:{org}:{id}`
//! (the id is generated first, trap 3). Every write appends one audit row with ids and
//! `has_summary`, never the text (AUTH-5). `kind`, `reason` and `summary` are staff input;
//! nothing here derives them from a flag or score (REC-10).

use asd_contract::api_v1::Role;
use asd_contract::emr::{
    CreateEncounterRequest, Encounter, EncounterKind, EncounterPage, EncounterStatus,
    LinkSessionRequest, UpdateEncounterRequest, MAX_ENCOUNTER_REASON_CHARS,
    MAX_ENCOUNTER_SUMMARY_CHARS,
};
use asd_contract::problem::{
    CONFLICT, ENCOUNTER_CLOSED, ENCOUNTER_SUMMARY_FORBIDDEN, STALE_VERSION,
};
use axum::extract::rejection::QueryRejection;
use axum::extract::{Path, Query, State};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use http::{header, HeaderMap, HeaderValue, StatusCode};
use serde_json::json;
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use crate::app::AppState;
use crate::audit::AuditEvent;
use crate::auth::extractors::{ApiJson, RequestMeta, Require, UserPrincipal};
use crate::auth::rbac::{self, perm, Action};
use crate::crypto::{Aad, ClinicalCrypto};
use crate::error::{AppError, AppResult};
use crate::problem::InvalidParam;
use crate::read::cursor::{self, Cursor};
use crate::routes::participant_detail::{page_query, participant_org, PageQuery};
use crate::routes::sessions::path_uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/participants/{id}/encounters", post(create).get(list))
        .route("/encounters/{id}", get(one).patch(update))
        .route("/encounters/{id}/sessions", post(link))
        .route("/encounters/{id}/sessions/{session_id}", delete(unlink))
}

fn invalid(name: &str, reason: &str) -> AppError {
    AppError::Validation(vec![InvalidParam {
        name: name.into(),
        reason: reason.into(),
    }])
}

fn summary_forbidden() -> AppError {
    AppError::BadRequest {
        slug: ENCOUNTER_SUMMARY_FORBIDDEN,
        detail: "writing an encounter summary needs the clinical note permission".into(),
    }
}

fn closed() -> AppError {
    AppError::Conflict {
        slug: ENCOUNTER_CLOSED,
        detail: "the encounter is closed; only follow_up_done_at or a reopen is accepted".into(),
    }
}

/// The AAD of an encounter summary (KEY-3).
fn summary_aad(org: Uuid, id: Uuid) -> Aad {
    Aad::new("encounters", "summary_ct", org, id)
}

/// `If-Match: "<n>"` (also `W/"<n>"` or a bare `n`) as a row version.
fn if_match(headers: &HeaderMap) -> AppResult<i32> {
    let bad = || {
        invalid(
            "If-Match",
            "required: the encounter's row_version as \"<n>\"",
        )
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

// ---- field validation ---------------------------------------------------------------------

fn timestamp(name: &str, raw: &str) -> AppResult<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(raw.trim())
        .map(|d| d.with_timezone(&Utc))
        .map_err(|_| invalid(name, "must be an RFC 3339 timestamp"))
}

/// An optional timestamp where `""` means "clear" (PATCH) or "absent" (create).
fn opt_timestamp(name: &str, raw: Option<&str>) -> AppResult<Option<DateTime<Utc>>> {
    match raw.map(str::trim).filter(|t| !t.is_empty()) {
        None => Ok(None),
        Some(t) => timestamp(name, t).map(Some),
    }
}

/// `reason`: trimmed, ≤ 200 characters, no control characters; empty is none.
fn reason(raw: Option<&str>) -> AppResult<Option<String>> {
    let Some(t) = raw.map(str::trim).filter(|t| !t.is_empty()) else {
        return Ok(None);
    };
    if t.chars().count() > MAX_ENCOUNTER_REASON_CHARS {
        return Err(invalid(
            "reason",
            &format!("must be at most {MAX_ENCOUNTER_REASON_CHARS} characters"),
        ));
    }
    if t.chars().any(char::is_control) {
        return Err(invalid("reason", "must not contain control characters"));
    }
    Ok(Some(t.to_owned()))
}

/// `summary`: ≤ 8 000 characters; blank is none.
fn summary(raw: Option<&str>) -> AppResult<Option<String>> {
    let Some(s) = raw.filter(|s| !s.trim().is_empty()) else {
        return Ok(None);
    };
    if s.chars().count() > MAX_ENCOUNTER_SUMMARY_CHARS {
        return Err(invalid(
            "summary",
            &format!("must be at most {MAX_ENCOUNTER_SUMMARY_CHARS} characters"),
        ));
    }
    Ok(Some(s.to_owned()))
}

/// `clinician_id`: a user of `org`; empty is none.
async fn clinician(db: &PgPool, org: Uuid, raw: Option<&str>) -> AppResult<Option<Uuid>> {
    let Some(t) = raw.map(str::trim).filter(|t| !t.is_empty()) else {
        return Ok(None);
    };
    let id = Uuid::parse_str(t).map_err(|_| invalid("clinician_id", "must be a UUID"))?;
    let ok = sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM users WHERE id = $1 AND org_id = $2) AS "ok!""#,
        id,
        org
    )
    .fetch_one(db)
    .await?;
    if ok {
        Ok(Some(id))
    } else {
        Err(invalid(
            "clinician_id",
            "is not a user of this organization",
        ))
    }
}

/// `409 participant-withdrawn` for a participant who withdrew consent (no new data).
async fn ensure_not_withdrawn(conn: &mut PgConnection, participant: Uuid) -> AppResult<()> {
    let withdrawn = sqlx::query_scalar!(
        r#"SELECT withdrawn_at IS NOT NULL AS "w!" FROM participants WHERE id = $1 FOR SHARE"#,
        participant
    )
    .fetch_one(&mut *conn)
    .await?;
    if withdrawn {
        Err(crate::ingest::participant_withdrawn())
    } else {
        Ok(())
    }
}

/// Links `session_ids` (of `participant` in `org`) to `encounter`; a session linked to
/// another encounter is `409 conflict`, an unknown one `400`. Already linked is a no-op.
async fn link_sessions(
    conn: &mut PgConnection,
    org: Uuid,
    participant: Uuid,
    encounter: Uuid,
    session_ids: &[String],
) -> AppResult<()> {
    for session in session_ids {
        let current = sqlx::query!(
            "SELECT encounter_id FROM sessions
             WHERE id = $1 AND participant_id = $2 AND org_id = $3 FOR UPDATE",
            session,
            participant,
            org
        )
        .fetch_optional(&mut *conn)
        .await?
        .ok_or_else(|| invalid("session_id", "is not a session of this participant"))?;
        match current.encounter_id {
            Some(e) if e == encounter => {}
            Some(_) => {
                return Err(AppError::Conflict {
                    slug: CONFLICT,
                    detail: "the session is linked to another encounter; unlink it first".into(),
                })
            }
            None => {
                sqlx::query!(
                    "UPDATE sessions SET encounter_id = $2 WHERE id = $1",
                    session,
                    encounter
                )
                .execute(&mut *conn)
                .await?;
            }
        }
    }
    Ok(())
}

// ---- the read model ---------------------------------------------------------------------

pub(crate) struct EncounterRow {
    pub id: Uuid,
    pub org_id: Uuid,
    pub participant_id: Uuid,
    pub kind: String,
    pub occurred_at: DateTime<Utc>,
    pub clinician_id: Option<Uuid>,
    pub clinician_name: Option<String>,
    pub reason: Option<String>,
    pub summary_ct: Option<Vec<u8>>,
    pub summary_key_id: Option<Uuid>,
    pub status: String,
    pub follow_up_due_at: Option<DateTime<Utc>>,
    pub follow_up_done_at: Option<DateTime<Utc>>,
    pub row_version: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
    pub session_ids: Vec<String>,
}

impl EncounterRow {
    fn has_summary(&self) -> bool {
        self.summary_ct.is_some()
    }

    /// The DTO; the summary is decrypted only when `crypto` is given (the caller may read
    /// it, RBAC-4) and one exists.
    async fn into_dto(self, crypto: Option<&ClinicalCrypto>) -> AppResult<Encounter> {
        let has_summary = self.has_summary();
        let summary = match (crypto, &self.summary_ct, self.summary_key_id) {
            (Some(c), Some(ct), Some(key)) => Some(
                c.decrypt_string(self.org_id, &summary_aad(self.org_id, self.id), key, ct)
                    .await?,
            ),
            _ => None,
        };
        Ok(Encounter {
            id: self.id.to_string(),
            participant_id: self.participant_id.to_string(),
            kind: EncounterKind::parse(&self.kind)
                .ok_or_else(|| AppError::Internal("unknown encounter kind".into()))?,
            occurred_at: cursor::ts(self.occurred_at),
            clinician_id: self.clinician_id.map(|u| u.to_string()),
            clinician_display_name: self.clinician_name,
            reason: self.reason,
            summary,
            has_summary,
            status: EncounterStatus::parse(&self.status)
                .ok_or_else(|| AppError::Internal("unknown encounter status".into()))?,
            follow_up_due_at: self.follow_up_due_at.map(cursor::ts),
            follow_up_done_at: self.follow_up_done_at.map(cursor::ts),
            session_ids: self.session_ids,
            row_version: self.row_version,
            created_at: cursor::ts(self.created_at),
            updated_at: self.updated_at.map(cursor::ts),
        })
    }
}

pub(crate) async fn encounter_by_id(db: &PgPool, id: Uuid) -> AppResult<Option<EncounterRow>> {
    Ok(sqlx::query_as!(
        EncounterRow,
        r#"SELECT e.id, e.org_id, e.participant_id, e.kind, e.occurred_at, e.clinician_id,
                  u.display_name AS "clinician_name?", e.reason, e.summary_ct, e.summary_key_id,
                  e.status, e.follow_up_due_at, e.follow_up_done_at, e.row_version, e.created_at,
                  e.updated_at,
                  ARRAY(SELECT s.id FROM sessions s WHERE s.encounter_id = e.id
                        ORDER BY s.started_at, s.id) AS "session_ids!"
           FROM encounters e LEFT JOIN users u ON u.id = e.clinician_id
           WHERE e.id = $1"#,
        id
    )
    .fetch_optional(db)
    .await?)
}

/// The keyring when `principal` may read summaries (`ClinicalContentRead`) and one of
/// `rows` has a summary; `None` otherwise (summaries are then omitted).
fn summary_reader(
    state: &AppState,
    principal: &UserPrincipal,
    rows: &[&EncounterRow],
) -> AppResult<Option<ClinicalCrypto>> {
    if !rbac::allows(principal.role, Action::ClinicalContentRead)
        || !rows.iter().any(|r| r.has_summary())
    {
        return Ok(None);
    }
    ClinicalCrypto::from_state(state).map(Some)
}

/// Loads encounter `id` and answers it with its `ETag`. A summary in the answer is audited
/// as `encounter.read` first, unless `wrote_summary` (the caller just sent that text).
async fn reply(
    state: &AppState,
    principal: &UserPrincipal,
    meta: &RequestMeta,
    id: Uuid,
    status: StatusCode,
    wrote_summary: bool,
) -> AppResult<Response> {
    let row = encounter_by_id(&state.db, id)
        .await?
        .ok_or(AppError::NotFound)?;
    let crypto = summary_reader(state, principal, &[&row])?;
    if crypto.is_some() && !wrote_summary {
        crate::audit::record_read(
            &state.db,
            principal,
            meta,
            row.org_id,
            "encounter.read",
            "encounter",
            &id.to_string(),
            None,
        )
        .await?;
    }
    let dto = row.into_dto(crypto.as_ref()).await?;
    let etag = HeaderValue::from_str(&format!("\"{}\"", dto.row_version))
        .expect("digits are a valid header value");
    Ok((status, [(header::ETAG, etag)], Json(dto)).into_response())
}

/// The encounter `raw_id` of the caller's org (another org's or unknown: 404).
async fn load_for(
    state: &AppState,
    principal: &UserPrincipal,
    raw_id: &str,
) -> AppResult<EncounterRow> {
    let id = path_uuid(raw_id)?;
    let row = encounter_by_id(&state.db, id)
        .await?
        .ok_or(AppError::NotFound)?;
    principal.ensure_org(row.org_id)?;
    Ok(row)
}

// ---- handlers -----------------------------------------------------------------------------

async fn create(
    auth: Require<perm::EncounterWrite>,
    State(state): State<AppState>,
    meta: RequestMeta,
    Path(raw_id): Path<String>,
    ApiJson(req): ApiJson<CreateEncounterRequest>,
) -> AppResult<Response> {
    let (participant, org) = participant_org(&state.db, &raw_id).await?;
    auth.ensure_org(org)?;
    let summary = summary(req.summary.as_deref())?;
    if summary.is_some() && !rbac::allows(auth.principal.role, Action::ClinicalNoteWrite) {
        return Err(summary_forbidden());
    }
    let occurred_at = timestamp("occurred_at", &req.occurred_at)?;
    let reason = reason(req.reason.as_deref())?;
    let follow_up_due_at = opt_timestamp("follow_up_due_at", req.follow_up_due_at.as_deref())?;
    let clinician_id = clinician(&state.db, org, req.clinician_id.as_deref()).await?;

    // Trap 3: the id is part of the AAD, so it exists before anything is encrypted.
    let id = Uuid::new_v4();
    let sealed = match &summary {
        Some(text) => {
            let crypto = ClinicalCrypto::from_state(&state)?;
            Some(crypto.encrypt_str(org, &summary_aad(org, id), text).await?)
        }
        None => None,
    };

    let mut tx = state.db.begin().await?;
    ensure_not_withdrawn(&mut tx, participant).await?;
    sqlx::query!(
        "INSERT INTO encounters
             (id, org_id, participant_id, kind, occurred_at, clinician_id, reason, summary_ct,
              summary_key_id, follow_up_due_at, created_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
        id,
        org,
        participant,
        req.kind.as_str(),
        occurred_at,
        clinician_id,
        reason,
        sealed.as_ref().map(|s| s.ciphertext.clone()),
        sealed.as_ref().map(|s| s.key_id),
        follow_up_due_at,
        auth.principal.user_id,
    )
    .execute(&mut *tx)
    .await?;
    link_sessions(&mut tx, org, participant, id, &req.session_ids).await?;
    AuditEvent::user(&auth.principal, "encounter.create", "encounter", id)
        .org(org)
        .after(json!({
            "participant_id": participant,
            "kind": req.kind.as_str(),
            "has_summary": sealed.is_some(),
            "has_reason": reason.is_some(),
            "clinician_id": clinician_id,
            "follow_up_due_at": follow_up_due_at.map(cursor::ts),
            "session_ids": req.session_ids,
        }))
        .meta(&meta)
        .record(&mut tx)
        .await?;
    tx.commit().await?;

    reply(
        &state,
        &auth.principal,
        &meta,
        id,
        StatusCode::CREATED,
        sealed.is_some(),
    )
    .await
}

async fn one(
    auth: Require<perm::ParticipantRead>,
    State(state): State<AppState>,
    meta: RequestMeta,
    Path(raw_id): Path<String>,
) -> AppResult<Response> {
    let row = load_for(&state, &auth.principal, &raw_id).await?;
    reply(
        &state,
        &auth.principal,
        &meta,
        row.id,
        StatusCode::OK,
        false,
    )
    .await
}

async fn list(
    auth: Require<perm::ParticipantRead>,
    State(state): State<AppState>,
    meta: RequestMeta,
    Path(raw_id): Path<String>,
    query: Result<Query<PageQuery>, QueryRejection>,
) -> AppResult<Json<EncounterPage>> {
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
        EncounterRow,
        r#"SELECT e.id, e.org_id, e.participant_id, e.kind, e.occurred_at, e.clinician_id,
                  u.display_name AS "clinician_name?", e.reason, e.summary_ct, e.summary_key_id,
                  e.status, e.follow_up_due_at, e.follow_up_done_at, e.row_version, e.created_at,
                  e.updated_at,
                  ARRAY(SELECT s.id FROM sessions s WHERE s.encounter_id = e.id
                        ORDER BY s.started_at, s.id) AS "session_ids!"
           FROM encounters e LEFT JOIN users u ON u.id = e.clinician_id
           WHERE e.participant_id = $1 AND e.org_id = $2
             AND ($3::timestamptz IS NULL OR (e.occurred_at, e.id) < ($3, $4::uuid))
           ORDER BY e.occurred_at DESC, e.id DESC
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
        r#"SELECT count(*) AS "n!" FROM encounters WHERE participant_id = $1 AND org_id = $2"#,
        participant,
        org
    )
    .fetch_one(&state.db)
    .await?;

    let limit = usize::try_from(limit).unwrap_or(usize::MAX);
    let has_more = rows.len() > limit;
    let page: Vec<EncounterRow> = rows.into_iter().take(limit).collect();
    let crypto = summary_reader(&state, &auth.principal, &page.iter().collect::<Vec<_>>())?;
    if crypto.is_some() {
        // HIP-1: one row per page naming the encounters whose summary is returned.
        let ids: Vec<String> = page
            .iter()
            .filter(|r| r.has_summary())
            .map(|r| r.id.to_string())
            .collect();
        crate::audit::record_read(
            &state.db,
            &auth.principal,
            &meta,
            org,
            "encounter.read",
            "participant",
            &participant.to_string(),
            Some(json!({ "encounter_ids": ids })),
        )
        .await?;
    }
    let next_cursor = if has_more {
        page.last()
            .map(|r| Cursor::new(r.occurred_at, r.id.to_string()).encode())
    } else {
        None
    };
    let mut items = Vec::with_capacity(page.len());
    for row in page {
        items.push(row.into_dto(crypto.as_ref()).await?);
    }
    Ok(Json(EncounterPage {
        items,
        next_cursor,
        total: Some(u64::try_from(total).unwrap_or(0)),
    }))
}

/// A PATCH value for an optional text field: absent keeps, `""` clears, else sets.
enum Change<T> {
    Keep,
    Set(Option<T>),
}

impl<T> Change<T> {
    fn is_change(&self) -> bool {
        matches!(self, Change::Set(_))
    }

    fn apply(self, current: Option<T>) -> Option<T> {
        match self {
            Change::Keep => current,
            Change::Set(v) => v,
        }
    }
}

async fn update(
    auth: Require<perm::EncounterWrite>,
    State(state): State<AppState>,
    meta: RequestMeta,
    Path(raw_id): Path<String>,
    headers: HeaderMap,
    ApiJson(req): ApiJson<UpdateEncounterRequest>,
) -> AppResult<Response> {
    let expected = if_match(&headers)?;
    let current = load_for(&state, &auth.principal, &raw_id).await?;
    let (id, org) = (current.id, current.org_id);
    let role = auth.principal.role;

    if req.summary.is_some() && !rbac::allows(role, Action::ClinicalNoteWrite) {
        return Err(summary_forbidden());
    }
    // REC-4: a closed encounter accepts only `follow_up_done_at` and a reopen.
    let was_closed = current.status == EncounterStatus::Closed.as_str();
    let reopen = req.status == Some(EncounterStatus::Open);
    if was_closed {
        let may_reopen = matches!(role, Role::Owner | Role::Clinician);
        let other_fields = req.kind.is_some()
            || req.occurred_at.is_some()
            || req.clinician_id.is_some()
            || req.reason.is_some()
            || req.summary.is_some()
            || req.follow_up_due_at.is_some();
        if (reopen && !may_reopen) || (!reopen && other_fields) {
            return Err(closed());
        }
    }

    let occurred_at = match req.occurred_at.as_deref() {
        Some(raw) => Some(timestamp("occurred_at", raw)?),
        None => None,
    };
    let reason = match req.reason.as_deref() {
        Some(raw) => Change::Set(reason(Some(raw))?),
        None => Change::Keep,
    };
    let clinician_id = match req.clinician_id.as_deref() {
        Some(raw) => Change::Set(clinician(&state.db, org, Some(raw)).await?),
        None => Change::Keep,
    };
    let due = match req.follow_up_due_at.as_deref() {
        Some(raw) => Change::Set(opt_timestamp("follow_up_due_at", Some(raw))?),
        None => Change::Keep,
    };
    let done = match req.follow_up_done_at.as_deref() {
        Some(raw) => Change::Set(opt_timestamp("follow_up_done_at", Some(raw))?),
        None => Change::Keep,
    };
    let summary_change = match req.summary.as_deref() {
        Some(raw) => Change::Set(match summary(Some(raw))? {
            Some(text) => {
                let crypto = ClinicalCrypto::from_state(&state)?;
                Some(
                    crypto
                        .encrypt_str(org, &summary_aad(org, id), &text)
                        .await?,
                )
            }
            None => None,
        }),
        None => Change::Keep,
    };

    let mut changed: Vec<&'static str> = Vec::new();
    for (name, is) in [
        ("kind", req.kind.is_some()),
        ("occurred_at", occurred_at.is_some()),
        ("clinician_id", clinician_id.is_change()),
        ("reason", reason.is_change()),
        ("summary", summary_change.is_change()),
        ("status", req.status.is_some()),
        ("follow_up_due_at", due.is_change()),
        ("follow_up_done_at", done.is_change()),
    ] {
        if is {
            changed.push(name);
        }
    }

    let mut tx = state.db.begin().await?;
    let locked = sqlx::query!(
        "SELECT row_version, status FROM encounters WHERE id = $1 FOR UPDATE",
        id
    )
    .fetch_one(&mut *tx)
    .await?;
    if locked.row_version != expected {
        return Err(AppError::PreconditionFailed {
            slug: STALE_VERSION,
            detail: format!(
                "the encounter is at row_version {}; reload and retry",
                locked.row_version
            ),
        });
    }
    let had_summary = current.has_summary();
    let (summary_ct, summary_key_id) = match summary_change {
        Change::Keep => (current.summary_ct, current.summary_key_id),
        Change::Set(Some(s)) => (Some(s.ciphertext), Some(s.key_id)),
        Change::Set(None) => (None, None),
    };
    let has_summary = summary_ct.is_some();
    let kind = req.kind.map_or(current.kind, |k| k.as_str().to_owned());
    let status = req
        .status
        .map_or(current.status.clone(), |s| s.as_str().to_owned());
    sqlx::query!(
        "UPDATE encounters SET
             kind = $2, occurred_at = $3, clinician_id = $4, reason = $5, summary_ct = $6,
             summary_key_id = $7, status = $8, follow_up_due_at = $9, follow_up_done_at = $10,
             row_version = row_version + 1, updated_at = now()
         WHERE id = $1",
        id,
        kind,
        occurred_at.unwrap_or(current.occurred_at),
        clinician_id.apply(current.clinician_id),
        reason.apply(current.reason),
        summary_ct,
        summary_key_id,
        status,
        due.apply(current.follow_up_due_at),
        done.apply(current.follow_up_done_at),
    )
    .execute(&mut *tx)
    .await?;
    AuditEvent::user(&auth.principal, "encounter.update", "encounter", id)
        .org(org)
        .before(json!({
            "row_version": locked.row_version,
            "status": locked.status,
            "has_summary": had_summary,
        }))
        .after(json!({
            "row_version": locked.row_version + 1,
            "status": status,
            "has_summary": has_summary,
            "changed": changed,
        }))
        .meta(&meta)
        .record(&mut tx)
        .await?;
    tx.commit().await?;

    let wrote_summary = changed.contains(&"summary") && has_summary;
    reply(
        &state,
        &auth.principal,
        &meta,
        id,
        StatusCode::OK,
        wrote_summary,
    )
    .await
}

async fn link(
    auth: Require<perm::EncounterWrite>,
    State(state): State<AppState>,
    meta: RequestMeta,
    Path(raw_id): Path<String>,
    ApiJson(req): ApiJson<LinkSessionRequest>,
) -> AppResult<Response> {
    let row = load_for(&state, &auth.principal, &raw_id).await?;
    if row.status == EncounterStatus::Closed.as_str() {
        return Err(closed());
    }
    let mut tx = state.db.begin().await?;
    link_sessions(
        &mut tx,
        row.org_id,
        row.participant_id,
        row.id,
        std::slice::from_ref(&req.session_id),
    )
    .await?;
    AuditEvent::user(
        &auth.principal,
        "encounter.session_link",
        "encounter",
        row.id,
    )
    .org(row.org_id)
    .after(json!({ "session_id": req.session_id }))
    .meta(&meta)
    .record(&mut tx)
    .await?;
    tx.commit().await?;
    reply(
        &state,
        &auth.principal,
        &meta,
        row.id,
        StatusCode::OK,
        false,
    )
    .await
}

async fn unlink(
    auth: Require<perm::EncounterWrite>,
    State(state): State<AppState>,
    meta: RequestMeta,
    Path((raw_id, session_id)): Path<(String, String)>,
) -> AppResult<StatusCode> {
    let row = load_for(&state, &auth.principal, &raw_id).await?;
    if row.status == EncounterStatus::Closed.as_str() {
        return Err(closed());
    }
    let mut tx = state.db.begin().await?;
    let n = sqlx::query!(
        "UPDATE sessions SET encounter_id = NULL WHERE id = $1 AND encounter_id = $2",
        session_id,
        row.id
    )
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if n == 0 {
        return Err(AppError::NotFound);
    }
    AuditEvent::user(
        &auth.principal,
        "encounter.session_unlink",
        "encounter",
        row.id,
    )
    .org(row.org_id)
    .after(json!({ "session_id": session_id }))
    .meta(&meta)
    .record(&mut tx)
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn if_match_forms() {
        let h = |v: &str| {
            let mut m = HeaderMap::new();
            m.insert(header::IF_MATCH, HeaderValue::from_str(v).unwrap());
            m
        };
        assert_eq!(if_match(&h("\"3\"")).unwrap(), 3);
        assert_eq!(if_match(&h("W/\"4\"")).unwrap(), 4);
        assert_eq!(if_match(&h("5")).unwrap(), 5);
        assert!(if_match(&h("\"0\"")).is_err());
        assert!(if_match(&HeaderMap::new()).is_err());
    }

    #[test]
    fn reason_and_summary_limits() {
        assert_eq!(reason(Some("  ")).unwrap(), None);
        assert_eq!(reason(Some(" fever ")).unwrap().as_deref(), Some("fever"));
        assert!(reason(Some(&"x".repeat(201))).is_err());
        assert!(reason(Some("a\u{7}b")).is_err());
        assert!(summary(Some(&"y".repeat(8_001))).is_err());
        assert_eq!(summary(Some(" ")).unwrap(), None);
    }
}
