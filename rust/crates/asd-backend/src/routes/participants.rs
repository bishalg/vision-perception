//! owner: WP-B.3 (list), WP-B.19 (create, search)
//!
//! - `GET /v1/orgs/{id}/participants` (console, `api_v1::ParticipantPage`, CON-20): the
//!   org's pseudonymous participants, newest first, cursor over `(created_at, id)`, with
//!   session count and last session start.
//! - `POST /v1/orgs/{id}/participants` (REC-1, `ParticipantCreate`): a clinic record with a
//!   server-minted token (`participant_token_from_uuid`) and its QR payload. The insert has
//!   the ingest shape `ON CONFLICT (org_id, token)` (trap 2), so a device that posts first
//!   and a console that creates first end on one row, never a `409`.
//! - `POST /v1/orgs/{id}/participants/search` (REC-2, `ParticipantRead`): the list rows
//!   ([`participant_rows`], so `has_patient_ref` follows the same MRN-6 rule) filtered by
//!   token prefix, age bracket, last session range, open follow-up and, with
//!   `PatientRefRead`, the MRN through its blind index (MRN-4: body only), plus the record
//!   counters. One `participant.search` audit row per page names the ids returned.
//!
//! The device-plane `/participants/{token}/*` routes are WP-B.6.

use asd_contract::api_v1::{Participant, ParticipantPage};
use asd_contract::clinical::PatientRefMode;
use asd_contract::emr::{
    participant_token_from_uuid, qr_payload, CreateParticipantRequest, CreatedParticipant,
    ParticipantSearchPage, ParticipantSearchRequest, ParticipantSearchRow,
};
use asd_contract::problem::PATIENT_REF_DISABLED;
use axum::body::Bytes;
use axum::extract::rejection::{BytesRejection, QueryRejection};
use axum::extract::{Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use http::StatusCode;
use serde::Deserialize;
use serde_json::json;
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use crate::app::AppState;
use crate::audit::AuditEvent;
use crate::auth::extractors::{json_or_default, ApiJson, RequestMeta, Require};
use crate::auth::rbac::{self, perm, Action};
use crate::crypto::blind_index::normalize;
use crate::crypto::ClinicalCrypto;
use crate::error::{AppError, AppResult};
use crate::problem::InvalidParam;
use crate::read::cursor::{self, Cursor};
use crate::routes::sessions::{body_or_problem, query_or_problem};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/orgs/{id}/participants", get(list).post(create))
        .route("/orgs/{id}/participants/search", post(search))
}

/// Longest accepted `token_prefix`, `age_bracket`, `language` and `consent_version`.
const MAX_SHORT_TEXT: usize = 64;
/// Longest accepted MRN in a search (MRN-1 bounds the stored one the same way).
const MAX_SEARCH_MRN: usize = asd_contract::clinical::MAX_MRN_CHARS;

fn invalid(name: &str, reason: &str) -> AppError {
    AppError::Validation(vec![InvalidParam {
        name: name.into(),
        reason: reason.into(),
    }])
}

// ---- the shared row builder -------------------------------------------------------------

/// Filters of [`participant_rows`]; the default selects every participant of the org.
#[derive(Debug, Default, Clone)]
pub(crate) struct RowFilters {
    pub token_prefix: Option<String>,
    pub age_bracket: Option<String>,
    pub last_seen_from: Option<DateTime<Utc>>,
    pub last_seen_to: Option<DateTime<Utc>>,
    pub has_open_follow_up: Option<bool>,
    /// Restrict to these ids (the MRN match); `Some(empty)` matches nothing.
    pub ids: Option<Vec<Uuid>>,
}

/// One participant with its counters, as the list and the search read it.
pub(crate) struct ParticipantRow {
    pub id: Uuid,
    pub org_id: Uuid,
    pub token: String,
    pub age_bracket_at_enrol: Option<String>,
    pub language: Option<String>,
    pub withdrawn_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub session_count: i64,
    pub last_session_at: Option<DateTime<Utc>>,
    pub has_patient_ref: bool,
    pub open_encounters_n: i64,
    pub next_follow_up_at: Option<DateTime<Utc>>,
    pub total: i64,
}

impl ParticipantRow {
    /// The `api_v1::Participant`; `has_patient_ref` only when `show_ref` (MRN-6).
    pub(crate) fn participant(&self, show_ref: bool) -> Participant {
        Participant {
            id: self.id.to_string(),
            org_id: self.org_id.to_string(),
            participant_token: self.token.clone(),
            age_bracket_at_enrol: self.age_bracket_at_enrol.clone(),
            language: self.language.clone(),
            session_count: u32::try_from(self.session_count).unwrap_or(u32::MAX),
            last_session_at: self.last_session_at.map(cursor::ts),
            withdrawn_at: self.withdrawn_at.map(cursor::ts),
            has_patient_ref: show_ref.then_some(self.has_patient_ref),
        }
    }
}

/// A page (`limit + 1` rows, newest first) of the org's participants matching `f`; every
/// row carries the filtered total.
pub(crate) async fn participant_rows(
    db: &PgPool,
    org: Uuid,
    f: &RowFilters,
    after: Option<&Cursor>,
    limit: i64,
) -> AppResult<Vec<ParticipantRow>> {
    let (after_at, after_id) = match after {
        Some(c) => (Some(c.at), Some(c.id.clone())),
        None => (None, None),
    };
    Ok(sqlx::query_as!(
        ParticipantRow,
        r#"WITH m AS (
             SELECT p.id, p.org_id, p.token, p.age_bracket_at_enrol, p.language, p.withdrawn_at,
                    p.created_at, s.n AS session_count, s.last_at AS last_session_at,
                    EXISTS (SELECT 1 FROM patient_refs r WHERE r.participant_id = p.id)
                        AS has_patient_ref,
                    (SELECT count(*) FROM encounters e
                     WHERE e.participant_id = p.id AND e.status = 'open') AS open_encounters_n,
                    (SELECT min(e.follow_up_due_at) FROM encounters e
                     WHERE e.participant_id = p.id AND e.follow_up_due_at IS NOT NULL
                       AND e.follow_up_done_at IS NULL) AS next_follow_up_at
             FROM participants p
             CROSS JOIN LATERAL (
                 SELECT count(*) AS n, max(x.started_at) AS last_at
                 FROM sessions x WHERE x.participant_id = p.id) s
             WHERE p.org_id = $1
               AND ($5::text IS NULL OR starts_with(p.token, $5))
               AND ($6::text IS NULL OR p.age_bracket_at_enrol = $6)
               AND ($7::timestamptz IS NULL OR s.last_at >= $7)
               AND ($8::timestamptz IS NULL OR s.last_at <= $8)
               AND ($9::uuid[] IS NULL OR p.id = ANY($9))
           )
           SELECT id AS "id!", org_id AS "org_id!", token AS "token!", age_bracket_at_enrol,
                  language, withdrawn_at, created_at AS "created_at!",
                  session_count AS "session_count!", last_session_at,
                  has_patient_ref AS "has_patient_ref!", open_encounters_n AS "open_encounters_n!",
                  next_follow_up_at,
                  (SELECT count(*) FROM m
                   WHERE $10::bool IS NULL OR (m.next_follow_up_at IS NOT NULL) = $10)
                      AS "total!"
           FROM m
           WHERE ($10::bool IS NULL OR (next_follow_up_at IS NOT NULL) = $10)
             AND ($2::timestamptz IS NULL OR (created_at, id::text) < ($2, $3::text))
           ORDER BY created_at DESC, id::text DESC
           LIMIT $4"#,
        org,
        after_at,
        after_id,
        limit + 1,
        f.token_prefix,
        f.age_bracket,
        f.last_seen_from,
        f.last_seen_to,
        f.ids.as_deref(),
        f.has_open_follow_up,
    )
    .fetch_all(db)
    .await?)
}

/// Cuts a `limit + 1` page: the kept rows, the next cursor and the total.
fn cut(mut rows: Vec<ParticipantRow>, limit: i64) -> (Vec<ParticipantRow>, Option<String>, u64) {
    let total = rows
        .first()
        .map_or(0, |r| u64::try_from(r.total).unwrap_or(0));
    let limit = usize::try_from(limit).unwrap_or(usize::MAX);
    let has_more = rows.len() > limit;
    rows.truncate(limit);
    let next = if has_more {
        rows.last()
            .map(|r| Cursor::new(r.created_at, r.id.to_string()).encode())
    } else {
        None
    };
    (rows, next, total)
}

// ---- GET /orgs/{id}/participants ---------------------------------------------------------

#[derive(Debug, Default, Deserialize)]
struct PageQuery {
    cursor: Option<String>,
    limit: Option<String>,
}

async fn list(
    auth: Require<perm::ParticipantRead>,
    State(state): State<AppState>,
    query: Result<Query<PageQuery>, QueryRejection>,
) -> AppResult<Json<ParticipantPage>> {
    let org = auth.org();
    // MRN-6: whether an MRN exists is only for callers who may read it (WP-C.11).
    let show_ref = rbac::allows(auth.principal.role, Action::PatientRefRead);
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
    let rows = participant_rows(
        &state.db,
        org,
        &RowFilters::default(),
        after.as_ref(),
        limit,
    )
    .await?;
    let (rows, next_cursor, total) = cut(rows, limit);
    Ok(Json(ParticipantPage {
        items: rows.iter().map(|r| r.participant(show_ref)).collect(),
        next_cursor,
        total: Some(total),
    }))
}

// ---- POST /orgs/{id}/participants (REC-1) ------------------------------------------------

/// A trimmed optional short text: empty is absent; control characters and over-long
/// values are refused.
fn short_text(name: &str, raw: Option<&str>) -> AppResult<Option<String>> {
    let Some(t) = raw.map(str::trim).filter(|t| !t.is_empty()) else {
        return Ok(None);
    };
    if t.chars().count() > MAX_SHORT_TEXT {
        return Err(invalid(
            name,
            &format!("must be at most {MAX_SHORT_TEXT} characters"),
        ));
    }
    if t.chars().any(char::is_control) {
        return Err(invalid(name, "must not contain control characters"));
    }
    Ok(Some(t.to_owned()))
}

fn timestamp(name: &str, raw: Option<&str>) -> AppResult<Option<DateTime<Utc>>> {
    match raw.map(str::trim).filter(|t| !t.is_empty()) {
        None => Ok(None),
        Some(t) => DateTime::parse_from_rfc3339(t)
            .map(|d| Some(d.with_timezone(&Utc)))
            .map_err(|_| invalid(name, "must be an RFC 3339 timestamp")),
    }
}

/// The validated columns of a create.
#[derive(Debug, Default, Clone)]
pub struct NewParticipant {
    pub age_bracket_at_enrol: Option<String>,
    pub language: Option<String>,
    pub consent_version: Option<String>,
    pub consent_at: Option<DateTime<Utc>>,
}

impl NewParticipant {
    fn from_request(req: &CreateParticipantRequest) -> AppResult<Self> {
        let consent_at = timestamp("consent_at", req.consent_at.as_deref())?;
        if consent_at.is_some_and(|t| t > Utc::now() + chrono::Duration::minutes(5)) {
            return Err(invalid("consent_at", "must not be in the future"));
        }
        Ok(Self {
            age_bracket_at_enrol: short_text(
                "age_bracket_at_enrol",
                req.age_bracket_at_enrol.as_deref(),
            )?,
            language: short_text("language", req.language.as_deref())?,
            consent_version: short_text("consent_version", req.consent_version.as_deref())?,
            consent_at,
        })
    }
}

/// Inserts (or links to) the clinic participant `token` of `org` with the ingest upsert
/// shape (trap 2): a row a device created first keeps its id and gets the fields it lacks;
/// nothing it already has is overwritten. Returns the participant id.
pub async fn upsert_clinic_participant(
    conn: &mut PgConnection,
    org: Uuid,
    token: &str,
    p: &NewParticipant,
) -> AppResult<Uuid> {
    Ok(sqlx::query_scalar!(
        "INSERT INTO participants
             (org_id, token, age_bracket_at_enrol, language, consent_version, consent_at, mode)
         VALUES ($1, $2, $3, $4, $5, $6, 'clinic')
         ON CONFLICT (org_id, token) DO UPDATE SET
             age_bracket_at_enrol = coalesce(participants.age_bracket_at_enrol,
                                             EXCLUDED.age_bracket_at_enrol),
             language = coalesce(participants.language, EXCLUDED.language),
             consent_version = coalesce(participants.consent_version, EXCLUDED.consent_version),
             consent_at = coalesce(participants.consent_at, EXCLUDED.consent_at),
             mode = coalesce(participants.mode, EXCLUDED.mode)
         RETURNING id",
        org,
        token,
        p.age_bracket_at_enrol,
        p.language,
        p.consent_version,
        p.consent_at,
    )
    .fetch_one(&mut *conn)
    .await?)
}

async fn create(
    auth: Require<perm::ParticipantCreate>,
    State(state): State<AppState>,
    meta: RequestMeta,
    body: Result<Bytes, BytesRejection>,
) -> AppResult<(StatusCode, Json<CreatedParticipant>)> {
    let org = auth.org();
    let req: CreateParticipantRequest = json_or_default(&body_or_problem(body)?)?;
    let fields = NewParticipant::from_request(&req)?;
    // REC-1: minted here, never supplied by the caller.
    let token = participant_token_from_uuid(Uuid::new_v4());

    let mut tx = state.db.begin().await?;
    let id = upsert_clinic_participant(&mut tx, org, &token, &fields).await?;
    AuditEvent::user(&auth.principal, "participant.create", "participant", id)
        .org(org)
        .after(json!({
            "mode": "clinic",
            "has_consent": fields.consent_version.is_some() || fields.consent_at.is_some(),
        }))
        .meta(&meta)
        .record(&mut tx)
        .await?;
    tx.commit().await?;

    let show_ref = rbac::allows(auth.principal.role, Action::PatientRefRead);
    let row = participant_rows(
        &state.db,
        org,
        &RowFilters {
            ids: Some(vec![id]),
            ..RowFilters::default()
        },
        None,
        1,
    )
    .await?
    .into_iter()
    .next()
    .ok_or_else(|| AppError::Internal("created participant not found".into()))?;
    Ok((
        StatusCode::CREATED,
        Json(CreatedParticipant {
            participant: row.participant(show_ref),
            qr_payload: qr_payload(org, &row.token),
        }),
    ))
}

// ---- POST /orgs/{id}/participants/search (REC-2) -----------------------------------------

/// MRN-2: `409 patient-ref-disabled` unless the org stores MRNs.
async fn ensure_mrn_mode(db: &PgPool, org: Uuid) -> AppResult<()> {
    let mode: Option<String> = sqlx::query_scalar!(
        "SELECT settings #>> '{identity,patient_ref_mode}' FROM organizations WHERE id = $1",
        org
    )
    .fetch_one(db)
    .await?;
    if mode.as_deref().and_then(PatientRefMode::parse) == Some(PatientRefMode::Mrn) {
        Ok(())
    } else {
        Err(AppError::Conflict {
            slug: PATIENT_REF_DISABLED,
            detail: "this organization does not store hospital patient references".into(),
        })
    }
}

/// The participants whose MRN is `mrn` under any assigning authority the org uses. The
/// request carries no authority (REC-2), so the blind index is computed once per
/// authority on file (a handful per org) and matched exactly (KEY-4).
async fn mrn_matches(
    db: &PgPool,
    crypto: &ClinicalCrypto,
    org: Uuid,
    mrn: &str,
) -> AppResult<Vec<Uuid>> {
    let authorities: Vec<String> = sqlx::query_scalar!(
        "SELECT DISTINCT assigning_authority FROM patient_refs WHERE org_id = $1",
        org
    )
    .fetch_all(db)
    .await?;
    let mut indexes = Vec::with_capacity(authorities.len());
    for authority in &authorities {
        indexes.push(crypto.mrn_blind_index(org, authority, mrn).await?.to_vec());
    }
    Ok(sqlx::query_scalar!(
        "SELECT participant_id FROM patient_refs WHERE org_id = $1 AND mrn_bidx = ANY($2)",
        org,
        &indexes,
    )
    .fetch_all(db)
    .await?)
}

async fn search(
    auth: Require<perm::ParticipantRead>,
    State(state): State<AppState>,
    meta: RequestMeta,
    ApiJson(req): ApiJson<ParticipantSearchRequest>,
) -> AppResult<Json<ParticipantSearchPage>> {
    let org = auth.org();
    let role = auth.principal.role;
    let show_ref = rbac::allows(role, Action::PatientRefRead);

    let mut used: Vec<&'static str> = Vec::new();
    let mut f = RowFilters::default();
    if let Some(prefix) = short_text("token_prefix", req.token_prefix.as_deref())? {
        used.push("token_prefix");
        f.token_prefix = Some(prefix);
    }
    if let Some(bracket) = short_text("age_bracket", req.age_bracket.as_deref())? {
        used.push("age_bracket");
        f.age_bracket = Some(bracket);
    }
    let bound = |name: &str, raw: Option<&str>, upper: bool| -> AppResult<_> {
        match raw.map(str::trim).filter(|t| !t.is_empty()) {
            None => Ok(None),
            Some(t) => crate::read::sessions::parse_bound(t, upper)
                .map(Some)
                .ok_or_else(|| invalid(name, "must be RFC 3339 or YYYY-MM-DD")),
        }
    };
    f.last_seen_from = bound("last_seen_from", req.last_seen_from.as_deref(), false)?;
    f.last_seen_to = bound("last_seen_to", req.last_seen_to.as_deref(), true)?;
    if f.last_seen_from.is_some() || f.last_seen_to.is_some() {
        used.push("last_seen");
    }
    if let Some(open) = req.has_open_follow_up {
        used.push("has_open_follow_up");
        f.has_open_follow_up = Some(open);
    }
    if let Some(raw) = req.mrn.as_deref().filter(|m| !m.trim().is_empty()) {
        // MRN-4: the MRN is only ever in this body; reading by it is reading the mapping.
        if !show_ref {
            return Err(AppError::Forbidden);
        }
        let mrn = raw.trim();
        if mrn.chars().count() > MAX_SEARCH_MRN || normalize(mrn).is_empty() {
            return Err(invalid(
                "mrn",
                &format!("must be 1..{MAX_SEARCH_MRN} characters"),
            ));
        }
        ensure_mrn_mode(&state.db, org).await?;
        let crypto = ClinicalCrypto::from_state(&state)?;
        used.push("mrn");
        f.ids = Some(mrn_matches(&state.db, &crypto, org, mrn).await?);
    }
    let limit = cursor::limit(req.limit.map(i64::from))?;
    let after = req
        .cursor
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(Cursor::decode)
        .transpose()?;

    let rows = participant_rows(&state.db, org, &f, after.as_ref(), limit).await?;
    let (rows, next_cursor, total) = cut(rows, limit);
    // HIP-1: one row per page with the ids shown; filter names only, never their values.
    let ids: Vec<String> = rows.iter().map(|r| r.id.to_string()).collect();
    crate::audit::record_read(
        &state.db,
        &auth.principal,
        &meta,
        org,
        "participant.search",
        "organization",
        &org.to_string(),
        Some(json!({ "participant_ids": ids, "filters": used })),
    )
    .await?;
    let items = rows
        .iter()
        .map(|r| ParticipantSearchRow {
            participant: r.participant(show_ref),
            open_encounters_n: u32::try_from(r.open_encounters_n).unwrap_or(u32::MAX),
            next_follow_up_at: r.next_follow_up_at.map(cursor::ts),
        })
        .collect();
    Ok(Json(ParticipantSearchPage {
        items,
        next_cursor,
        total: Some(total),
    }))
}
