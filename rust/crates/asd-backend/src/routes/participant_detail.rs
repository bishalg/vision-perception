//! owner: WP-C.11
//!
//! CW-3 participant detail and timeline (`ParticipantRead`):
//!
//! - `GET /v1/participants/{id}`: the [`Participant`] of `GET /v1/orgs/{id}/participants`;
//!   `has_patient_ref` is present only for a caller who may read the MRN (MRN-6).
//! - `GET /v1/participants/{id}/timeline`: [`crate::read::timeline`], headers only.
//!
//! A participant of another org, an unknown id and a malformed id are all `404` (AUTH-4).
//! The helpers below are shared by the MRN and note routes.

use asd_contract::api_v1::Participant;
use asd_contract::clinical::TimelinePage;
use axum::extract::rejection::QueryRejection;
use axum::extract::{Path, Query, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::app::AppState;
use crate::auth::extractors::Require;
use crate::auth::rbac::{self, perm, Action};
use crate::error::{AppError, AppResult};
use crate::problem::InvalidParam;
use crate::read::cursor::{self, Cursor};
use crate::read::timeline;
use crate::routes::sessions::{path_uuid, query_or_problem};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/participants/{id}", get(detail))
        .route("/participants/{id}/timeline", get(timeline_page))
}

/// `?cursor=&limit=` of the paged lists in this package.
#[derive(Debug, Default, Deserialize)]
pub(crate) struct PageQuery {
    cursor: Option<String>,
    limit: Option<String>,
}

impl PageQuery {
    /// The page size (1..=100, default 50) and the decoded cursor.
    pub(crate) fn parse(self) -> AppResult<(i64, Option<Cursor>)> {
        let limit = match self.limit.filter(|s| !s.is_empty()) {
            Some(raw) => cursor::limit(Some(raw.parse().map_err(|_| {
                AppError::Validation(vec![InvalidParam {
                    name: "limit".into(),
                    reason: "must be an integer".into(),
                }])
            })?))?,
            None => cursor::limit(None)?,
        };
        let cursor = self
            .cursor
            .filter(|s| !s.is_empty())
            .map(|c| Cursor::decode(&c))
            .transpose()?;
        Ok((limit, cursor))
    }
}

pub(crate) fn page_query(q: Result<Query<PageQuery>, QueryRejection>) -> AppResult<PageQuery> {
    query_or_problem(q)
}

/// The org of participant `raw_id`, or 404 when the id is malformed or unknown. The caller
/// then runs `ensure_org` so a foreign participant is also 404.
pub(crate) async fn participant_org(db: &PgPool, raw_id: &str) -> AppResult<(Uuid, Uuid)> {
    let id = path_uuid(raw_id)?;
    let org = sqlx::query_scalar!("SELECT org_id FROM participants WHERE id = $1", id)
        .fetch_optional(db)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok((id, org))
}

async fn detail(
    auth: Require<perm::ParticipantRead>,
    State(state): State<AppState>,
    Path(raw_id): Path<String>,
) -> AppResult<Json<Participant>> {
    let (id, org) = participant_org(&state.db, &raw_id).await?;
    auth.ensure_org(org)?;
    let show_ref = rbac::allows(auth.principal.role, Action::PatientRefRead);
    let r = sqlx::query!(
        r#"SELECT p.id, p.org_id, p.token, p.age_bracket_at_enrol, p.language, p.withdrawn_at,
                  (SELECT count(*) FROM sessions s WHERE s.participant_id = p.id)
                      AS "session_count!",
                  (SELECT max(s.started_at) FROM sessions s WHERE s.participant_id = p.id)
                      AS last_session_at,
                  EXISTS (SELECT 1 FROM patient_refs r WHERE r.participant_id = p.id)
                      AS "has_patient_ref!"
           FROM participants p WHERE p.id = $1"#,
        id,
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(Json(Participant {
        id: r.id.to_string(),
        org_id: r.org_id.to_string(),
        participant_token: r.token,
        age_bracket_at_enrol: r.age_bracket_at_enrol,
        language: r.language,
        session_count: u32::try_from(r.session_count).unwrap_or(u32::MAX),
        last_session_at: r.last_session_at.map(cursor::ts),
        withdrawn_at: r.withdrawn_at.map(cursor::ts),
        has_patient_ref: show_ref.then_some(r.has_patient_ref),
    }))
}

async fn timeline_page(
    auth: Require<perm::ParticipantRead>,
    State(state): State<AppState>,
    Path(raw_id): Path<String>,
    query: Result<Query<PageQuery>, QueryRejection>,
) -> AppResult<Json<TimelinePage>> {
    let (id, org) = participant_org(&state.db, &raw_id).await?;
    auth.ensure_org(org)?;
    let (limit, after) = page_query(query)?.parse()?;
    Ok(Json(
        timeline::page(&state.db, org, id, after.as_ref(), limit).await?,
    ))
}
