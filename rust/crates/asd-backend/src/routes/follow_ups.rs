//! owner: WP-B.19
//!
//! `GET /v1/orgs/{id}/follow-ups?due_before=&assigned=&cursor=&limit=` (REC-4,
//! `ParticipantRead`): the org's open follow-ups, i.e. encounters with `follow_up_due_at`
//! set and `follow_up_done_at` unset, earliest due first, cursor over `(due, id)`.
//!
//! - `due_before`: RFC 3339 or `YYYY-MM-DD` (the whole day included).
//! - `assigned`: `me` (the caller is the encounter's clinician), `none` (no clinician) or a
//!   user id.
//!
//! Headers only: no summary is selected, so none can reach this list.

use asd_contract::emr::{EncounterKind, FollowUp, FollowUpPage};
use axum::extract::rejection::QueryRejection;
use axum::extract::{Query, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use uuid::Uuid;

use crate::app::AppState;
use crate::auth::extractors::Require;
use crate::auth::rbac::perm;
use crate::error::{AppError, AppResult};
use crate::problem::InvalidParam;
use crate::read::cursor::{self, Cursor};
use crate::routes::sessions::query_or_problem;

pub fn router() -> Router<AppState> {
    Router::new().route("/orgs/{id}/follow-ups", get(list))
}

fn invalid(name: &str, reason: &str) -> AppError {
    AppError::Validation(vec![InvalidParam {
        name: name.into(),
        reason: reason.into(),
    }])
}

#[derive(Debug, Default, Deserialize)]
struct FollowUpQuery {
    due_before: Option<String>,
    assigned: Option<String>,
    cursor: Option<String>,
    limit: Option<String>,
}

/// The `assigned` filter as SQL parameters: (only unassigned, only this clinician).
fn assigned(raw: Option<&str>, me: Uuid) -> AppResult<(bool, Option<Uuid>)> {
    match raw.map(str::trim).filter(|s| !s.is_empty()) {
        None => Ok((false, None)),
        Some("me") => Ok((false, Some(me))),
        Some("none") => Ok((true, None)),
        Some(id) => Uuid::parse_str(id)
            .map(|u| (false, Some(u)))
            .map_err(|_| invalid("assigned", "must be me, none or a user id")),
    }
}

async fn list(
    auth: Require<perm::ParticipantRead>,
    State(state): State<AppState>,
    query: Result<Query<FollowUpQuery>, QueryRejection>,
) -> AppResult<Json<FollowUpPage>> {
    let org = auth.org();
    let q = query_or_problem(query)?;
    let due_before = match q.due_before.as_deref().filter(|s| !s.is_empty()) {
        Some(raw) => Some(
            crate::read::sessions::parse_bound(raw, true)
                .ok_or_else(|| invalid("due_before", "must be RFC 3339 or YYYY-MM-DD"))?,
        ),
        None => None,
    };
    let (unassigned, clinician) = assigned(q.assigned.as_deref(), auth.principal.user_id)?;
    let limit = match q.limit.filter(|s| !s.is_empty()) {
        Some(raw) => cursor::limit(Some(
            raw.parse()
                .map_err(|_| invalid("limit", "must be an integer"))?,
        ))?,
        None => cursor::limit(None)?,
    };
    let (after_at, after_id) = match q.cursor.filter(|s| !s.is_empty()) {
        Some(c) => {
            let c = Cursor::decode(&c)?;
            let id = Uuid::parse_str(&c.id)
                .map_err(|_| invalid("cursor", "not a cursor from this API"))?;
            (Some(c.at), Some(id))
        }
        None => (None, None),
    };

    let rows = sqlx::query!(
        r#"WITH f AS (
             SELECT e.id, e.participant_id, p.token, e.kind, e.occurred_at,
                    e.follow_up_due_at AS due, e.clinician_id, u.display_name AS clinician_name,
                    e.reason
             FROM encounters e
             JOIN participants p ON p.id = e.participant_id
             LEFT JOIN users u ON u.id = e.clinician_id
             WHERE e.org_id = $1 AND e.follow_up_due_at IS NOT NULL
               AND e.follow_up_done_at IS NULL
               AND ($2::timestamptz IS NULL OR e.follow_up_due_at <= $2)
               AND (NOT $3::bool OR e.clinician_id IS NULL)
               AND ($4::uuid IS NULL OR e.clinician_id = $4)
           )
           SELECT id AS "id!", participant_id AS "participant_id!", token AS "token!",
                  kind AS "kind!", occurred_at AS "occurred_at!", due AS "due!", clinician_id,
                  clinician_name, reason, (SELECT count(*) FROM f) AS "total!"
           FROM f
           WHERE ($5::timestamptz IS NULL OR (due, id) > ($5, $6::uuid))
           ORDER BY due, id
           LIMIT $7"#,
        org,
        due_before,
        unassigned,
        clinician,
        after_at,
        after_id,
        limit + 1,
    )
    .fetch_all(&state.db)
    .await?;

    let total = rows
        .first()
        .map_or(0, |r| u64::try_from(r.total).unwrap_or(0));
    let limit = usize::try_from(limit).unwrap_or(usize::MAX);
    let has_more = rows.len() > limit;
    let mut last = None;
    let mut items = Vec::with_capacity(rows.len().min(limit));
    for r in rows.into_iter().take(limit) {
        last = Some(Cursor::new(r.due, r.id.to_string()));
        items.push(FollowUp {
            encounter_id: r.id.to_string(),
            participant_id: r.participant_id.to_string(),
            participant_token: r.token,
            kind: EncounterKind::parse(&r.kind)
                .ok_or_else(|| AppError::Internal("unknown encounter kind".into()))?,
            occurred_at: cursor::ts(r.occurred_at),
            follow_up_due_at: cursor::ts(r.due),
            clinician_id: r.clinician_id.map(|u| u.to_string()),
            clinician_display_name: r.clinician_name,
            reason: r.reason,
        });
    }
    Ok(Json(FollowUpPage {
        items,
        next_cursor: if has_more {
            last.map(|c| c.encode())
        } else {
            None
        },
        total: Some(total),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assigned_forms() {
        let me = Uuid::new_v4();
        assert_eq!(assigned(None, me).unwrap(), (false, None));
        assert_eq!(assigned(Some("me"), me).unwrap(), (false, Some(me)));
        assert_eq!(assigned(Some("none"), me).unwrap(), (true, None));
        let other = Uuid::new_v4();
        assert_eq!(
            assigned(Some(&other.to_string()), me).unwrap(),
            (false, Some(other))
        );
        assert!(assigned(Some("everyone"), me).is_err());
    }
}
