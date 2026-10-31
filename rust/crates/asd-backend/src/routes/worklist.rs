//! owner: WP-C.10
//!
//! CW-1, CW-2: `GET /v1/orgs/{id}/worklist` (`WorklistRead`). The org's sessions whose
//! clinical review is not `final`, newest received first, cursor over `(received_at, id)`.
//! Filters: `state` (an unclaimed session counts as `open`), `flag`, `assigned=me|
//! unassigned|any` (default `any`), `from`/`to` (on `received_at`; RFC 3339 or
//! `YYYY-MM-DD`, `to` inclusive), `cursor`, `limit` (1..100, default 50).
//!
//! Scope is computed per request from `settings.clinical.worklist_scope` (`flagged` keeps
//! red, yellow and unreliable sessions); synthetic sessions appear only with
//! `ASD_ALLOW_SYNTHETIC=true`. Items carry no clinical content (RBAC-4), so the worklist
//! needs no keyring. `cosign_required` is the stored value once a review awaits a cosign,
//! otherwise the current policy applied to the session's flag.

use asd_contract::api_v1::{SessionAttentionFlag, SessionSummary};
use asd_contract::clinical::{
    ClinicalReviewState, WorklistAssigned, WorklistItem, WorklistPage, WorklistScope,
};
use axum::extract::rejection::QueryRejection;
use axum::extract::{Query, State};
use axum::routing::get;
use axum::{Json, Router};
use chrono::{DateTime, Duration, NaiveDate, Utc};
use serde::Deserialize;
use serde_json::Value;

use crate::app::AppState;
use crate::auth::extractors::Require;
use crate::auth::rbac::perm;
use crate::clinical::review::parse_state;
use crate::clinical::{cosign_required, ClinicalSettings, FLAGGED};
use crate::error::{AppError, AppResult};
use crate::problem::InvalidParam;
use crate::read::cursor::{self, Cursor};
use crate::routes::sessions::query_or_problem;

pub fn router() -> Router<AppState> {
    Router::new().route("/orgs/{id}/worklist", get(list))
}

#[derive(Debug, Default, Deserialize)]
struct WorklistQuery {
    state: Option<String>,
    flag: Option<String>,
    assigned: Option<String>,
    from: Option<String>,
    to: Option<String>,
    cursor: Option<String>,
    limit: Option<String>,
}

#[derive(Debug, PartialEq)]
struct Filters {
    state: Option<ClinicalReviewState>,
    flag: Option<String>,
    assigned: WorklistAssigned,
    from: Option<DateTime<Utc>>,
    to: Option<DateTime<Utc>>,
    cursor: Option<Cursor>,
    limit: i64,
}

fn bad(name: &str, reason: &str) -> InvalidParam {
    InvalidParam {
        name: name.into(),
        reason: reason.into(),
    }
}

/// RFC 3339, or `YYYY-MM-DD` (UTC): the day's first instant, or its last microsecond for
/// an upper bound.
fn parse_bound(raw: &str, upper: bool) -> Option<DateTime<Utc>> {
    if let Ok(t) = DateTime::parse_from_rfc3339(raw) {
        return Some(t.with_timezone(&Utc));
    }
    let start = NaiveDate::parse_from_str(raw, "%Y-%m-%d")
        .ok()?
        .and_hms_opt(0, 0, 0)?
        .and_utc();
    Some(if upper {
        start + Duration::days(1) - Duration::microseconds(1)
    } else {
        start
    })
}

impl WorklistQuery {
    fn parse(self) -> AppResult<Filters> {
        let mut errors = Vec::new();
        let some = |v: Option<String>| v.filter(|s| !s.is_empty());
        let state = some(self.state).and_then(|s| {
            let parsed = ClinicalReviewState::parse(&s);
            if parsed.is_none() {
                errors.push(bad(
                    "state",
                    "must be open, in_progress, awaiting_cosign, final or amending",
                ));
            }
            parsed
        });
        let flag = some(self.flag);
        if let Some(f) = &flag {
            if serde_json::from_value::<SessionAttentionFlag>(Value::String(f.clone())).is_err() {
                errors.push(bad("flag", "must be green, yellow, red or unreliable"));
            }
        }
        let assigned = match some(self.assigned) {
            None => WorklistAssigned::Any,
            Some(a) => WorklistAssigned::parse(&a).unwrap_or_else(|| {
                errors.push(bad("assigned", "must be me, unassigned or any"));
                WorklistAssigned::Any
            }),
        };
        let mut bound = |name: &str, raw: Option<String>, upper: bool| {
            some(raw).and_then(|r| {
                let parsed = parse_bound(&r, upper);
                if parsed.is_none() {
                    errors.push(bad(name, "must be RFC 3339 or YYYY-MM-DD"));
                }
                parsed
            })
        };
        let from = bound("from", self.from, false);
        let to = bound("to", self.to, true);
        let limit = match some(self.limit) {
            None => cursor::limit(None),
            Some(raw) => raw
                .parse::<i64>()
                .map_err(|_| AppError::Validation(vec![bad("limit", "must be an integer")]))
                .and_then(|n| cursor::limit(Some(n))),
        };
        let cursor = some(self.cursor).map(|c| Cursor::decode(&c)).transpose();
        match (limit, cursor) {
            (Ok(limit), Ok(cursor)) if errors.is_empty() => Ok(Filters {
                state,
                flag,
                assigned,
                from,
                to,
                cursor,
                limit,
            }),
            (l, c) => {
                for e in [l.err(), c.err()].into_iter().flatten() {
                    if let AppError::Validation(p) = e {
                        errors.extend(p);
                    }
                }
                Err(AppError::Validation(errors))
            }
        }
    }
}

async fn list(
    auth: Require<perm::WorklistRead>,
    State(state): State<AppState>,
    query: Result<Query<WorklistQuery>, QueryRejection>,
) -> AppResult<Json<WorklistPage>> {
    let org = auth.org();
    let me = auth.principal.user_id;
    let f = query_or_problem(query)?.parse()?;
    let settings = {
        let mut conn = state.db.acquire().await?;
        ClinicalSettings::load(&mut conn, org).await?
    };
    let flagged_only = settings.worklist_scope == WorklistScope::Flagged;
    let flagged: Vec<String> = FLAGGED.iter().map(|s| (*s).to_owned()).collect();
    let allow_synthetic = state.config.allow_synthetic;
    let review_state = f.state.map(ClinicalReviewState::as_str);
    let assigned = f.assigned.as_str();
    let (after_at, after_id) = match &f.cursor {
        Some(c) => (Some(c.at), Some(c.id.clone())),
        None => (None, None),
    };

    let rows = sqlx::query!(
        r#"SELECT s.id, s.participant_id, s.participant_token, s.started_at, s.duration_sec,
                  s.battery, s.capture_source, s.quality_verdict, s.attention_flag,
                  s.composite_score, s.composite_confidence, s.bundle_version, s.device_id,
                  s.received_at, s.tracking_verdict, s.tracking_accuracy_deg,
                  EXISTS (SELECT 1 FROM review_items r
                          WHERE r.session_id = s.id AND r.status IN ('done', 'dismissed'))
                      AS "reviewed!",
                  cr.id AS "review_id?", cr.state AS "clinical_state?",
                  cr.assigned_to AS "assigned_to?", cr.cosign_required AS "cosign_required?",
                  EXISTS (SELECT 1 FROM patient_refs p WHERE p.participant_id = s.participant_id)
                      AS "has_patient_ref!",
                  EXTRACT(EPOCH FROM (now() - s.received_at))::float8 AS "age_secs!"
           FROM sessions s
           LEFT JOIN clinical_reviews cr ON cr.session_id = s.id
           WHERE s.org_id = $1
             AND (cr.state IS NULL OR cr.state <> 'final')
             AND (NOT $2::bool OR s.attention_flag = ANY($3::text[]))
             AND ($4::bool OR s.capture_source <> 'synthetic')
             AND ($5::text IS NULL OR COALESCE(cr.state, 'open') = $5)
             AND ($6::text IS NULL OR s.attention_flag = $6)
             AND ($7::text = 'any'
                  OR ($7 = 'me' AND cr.assigned_to = $8)
                  OR ($7 = 'unassigned' AND cr.assigned_to IS NULL))
             AND ($9::timestamptz IS NULL OR s.received_at >= $9)
             AND ($10::timestamptz IS NULL OR s.received_at <= $10)
             AND ($11::timestamptz IS NULL OR (s.received_at, s.id) < ($11, $12::text))
           ORDER BY s.received_at DESC, s.id DESC
           LIMIT $13"#,
        org,
        flagged_only,
        &flagged,
        allow_synthetic,
        review_state,
        f.flag,
        assigned,
        me,
        f.from,
        f.to,
        after_at,
        after_id,
        f.limit + 1,
    )
    .fetch_all(&state.db)
    .await?;

    let total = sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!"
           FROM sessions s
           LEFT JOIN clinical_reviews cr ON cr.session_id = s.id
           WHERE s.org_id = $1
             AND (cr.state IS NULL OR cr.state <> 'final')
             AND (NOT $2::bool OR s.attention_flag = ANY($3::text[]))
             AND ($4::bool OR s.capture_source <> 'synthetic')
             AND ($5::text IS NULL OR COALESCE(cr.state, 'open') = $5)
             AND ($6::text IS NULL OR s.attention_flag = $6)
             AND ($7::text = 'any'
                  OR ($7 = 'me' AND cr.assigned_to = $8)
                  OR ($7 = 'unassigned' AND cr.assigned_to IS NULL))
             AND ($9::timestamptz IS NULL OR s.received_at >= $9)
             AND ($10::timestamptz IS NULL OR s.received_at <= $10)"#,
        org,
        flagged_only,
        &flagged,
        allow_synthetic,
        review_state,
        f.flag,
        assigned,
        me,
        f.from,
        f.to,
    )
    .fetch_one(&state.db)
    .await?;

    let limit = usize::try_from(f.limit).unwrap_or(usize::MAX);
    let has_more = rows.len() > limit;
    let mut items = Vec::with_capacity(rows.len().min(limit));
    let mut last = None;
    for r in rows.into_iter().take(limit) {
        last = Some(Cursor::new(r.received_at, r.id.clone()));
        let clinical_state = r.clinical_state.as_deref().map(parse_state).transpose()?;
        let cosign = match clinical_state {
            Some(ClinicalReviewState::AwaitingCosign) => r.cosign_required.unwrap_or(true),
            _ => cosign_required(settings.cosign_policy, &r.attention_flag),
        };
        let attention_flag =
            serde_json::from_value(Value::String(r.attention_flag)).map_err(|_| {
                AppError::Internal("stored attention_flag is not in the contract".into())
            })?;
        items.push(WorklistItem {
            session: SessionSummary {
                id: r.id,
                participant_id: r.participant_id.to_string(),
                participant_token: r.participant_token,
                started_at: cursor::ts(r.started_at),
                duration_sec: r.duration_sec,
                battery: r.battery,
                capture_source: serde_json::from_value(Value::String(r.capture_source)).ok(),
                quality_verdict: r.quality_verdict,
                attention_flag,
                composite_score: r.composite_score,
                composite_confidence: r.composite_confidence,
                bundle_version: r.bundle_version,
                device_id: r.device_id.map(|d| d.to_string()),
                reviewed: Some(r.reviewed),
                tracking_verdict: crate::read::sessions::parse_verdict(r.tracking_verdict)?,
                tracking_accuracy_deg: r.tracking_accuracy_deg,
            },
            review_id: r.review_id.map(|u| u.to_string()),
            clinical_state,
            assigned_to: r.assigned_to.map(|u| u.to_string()),
            age_hours: (r.age_secs / 3600.0).max(0.0),
            cosign_required: cosign,
            has_patient_ref: r.has_patient_ref,
        });
    }
    Ok(Json(WorklistPage {
        items,
        next_cursor: if has_more {
            last.map(|c| c.encode())
        } else {
            None
        },
        total: Some(u64::try_from(total).unwrap_or(0)),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_parses_and_collects_errors() {
        let ok = WorklistQuery {
            state: Some("in_progress".into()),
            flag: Some("red".into()),
            assigned: Some("me".into()),
            from: Some("2026-10-01".into()),
            to: Some("2026-10-02".into()),
            ..Default::default()
        }
        .parse()
        .unwrap();
        assert_eq!(ok.state, Some(ClinicalReviewState::InProgress));
        assert_eq!(ok.assigned, WorklistAssigned::Me);
        assert_eq!(ok.limit, 50);
        assert_eq!(
            ok.to.unwrap().to_rfc3339(),
            "2026-10-02T23:59:59.999999+00:00"
        );

        let Err(AppError::Validation(p)) = WorklistQuery {
            state: Some("done".into()),
            flag: Some("purple".into()),
            assigned: Some("all".into()),
            limit: Some("0".into()),
            ..Default::default()
        }
        .parse() else {
            panic!("expected validation error")
        };
        let names: Vec<_> = p.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["state", "flag", "assigned", "limit"]);
    }
}
