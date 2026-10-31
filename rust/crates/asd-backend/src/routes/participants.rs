//! owner: WP-B.3
//!
//! `GET /v1/orgs/{id}/participants` (console, `api_v1::ParticipantPage`, CON-20): the org's
//! pseudonymous participants, newest first, cursor over `(created_at, id)`, with session
//! count and last session start. The device-plane `/participants/{token}/*` routes are
//! WP-B.6.

use asd_contract::api_v1::{Participant, ParticipantPage};
use axum::extract::rejection::QueryRejection;
use axum::extract::{Query, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;

use crate::app::AppState;
use crate::auth::extractors::Require;
use crate::auth::rbac::{self, perm, Action};
use crate::error::{AppError, AppResult};
use crate::problem::InvalidParam;
use crate::read::cursor::{self, Cursor};
use crate::routes::sessions::query_or_problem;

pub fn router() -> Router<AppState> {
    Router::new().route("/orgs/{id}/participants", get(list))
}

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
        Some(raw) => cursor::limit(Some(raw.parse().map_err(|_| {
            AppError::Validation(vec![InvalidParam {
                name: "limit".into(),
                reason: "must be an integer".into(),
            }])
        })?))?,
        None => cursor::limit(None)?,
    };
    let (after_at, after_id) = match q.cursor.filter(|s| !s.is_empty()) {
        Some(c) => {
            let c = Cursor::decode(&c)?;
            (Some(c.at), Some(c.id))
        }
        None => (None, None),
    };

    let rows = sqlx::query!(
        r#"SELECT p.id, p.org_id, p.token, p.age_bracket_at_enrol, p.language, p.withdrawn_at,
                  p.created_at,
                  (SELECT count(*) FROM sessions s WHERE s.participant_id = p.id) AS "session_count!",
                  (SELECT max(s.started_at) FROM sessions s WHERE s.participant_id = p.id)
                      AS last_session_at,
                  EXISTS (SELECT 1 FROM patient_refs r WHERE r.participant_id = p.id)
                      AS "has_patient_ref!"
           FROM participants p
           WHERE p.org_id = $1
             AND ($2::timestamptz IS NULL OR (p.created_at, p.id::text) < ($2, $3::text))
           ORDER BY p.created_at DESC, p.id::text DESC
           LIMIT $4"#,
        org,
        after_at,
        after_id,
        limit + 1,
    )
    .fetch_all(&state.db)
    .await?;
    let total = sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!" FROM participants WHERE org_id = $1"#,
        org
    )
    .fetch_one(&state.db)
    .await?;

    let limit = usize::try_from(limit).unwrap_or(usize::MAX);
    let has_more = rows.len() > limit;
    let mut last = None;
    let items = rows
        .into_iter()
        .take(limit)
        .map(|r| {
            last = Some(Cursor::new(r.created_at, r.id.to_string()));
            Participant {
                id: r.id.to_string(),
                org_id: r.org_id.to_string(),
                participant_token: r.token,
                age_bracket_at_enrol: r.age_bracket_at_enrol,
                language: r.language,
                session_count: u32::try_from(r.session_count).unwrap_or(u32::MAX),
                last_session_at: r.last_session_at.map(cursor::ts),
                withdrawn_at: r.withdrawn_at.map(cursor::ts),
                has_patient_ref: show_ref.then_some(r.has_patient_ref),
            }
        })
        .collect();
    Ok(Json(ParticipantPage {
        items,
        next_cursor: if has_more {
            last.map(|c| c.encode())
        } else {
            None
        },
        total: Some(u64::try_from(total).unwrap_or(0)),
    }))
}
