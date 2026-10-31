//! owner: WP-B.6
//!
//! Care events (device plane, API-1): the port of the removed CareKit sync (`shims.py`).
//!
//! - [`ingest`]: `POST /v1/care/events`. A batch for one participant of the device's org;
//!   each event is stored once per `idempotency_key` (`UNIQUE (org_id, idempotency_key)`),
//!   and an optional `X-Idempotency-Key` makes the whole batch replayable (insert-first
//!   claim as in session ingest, scope `care:{org}`; same key and body replays the stored
//!   counts, a different body is `409 idempotency-key-reused`). A participant who withdrew
//!   consent gets `409 participant-withdrawn` and nothing is stored.
//! - [`summary`]: `GET /v1/participants/{token}/summary`, counts computed in SQL.
//! - [`request_erasure`]: `POST /v1/participants/{token}/erasure` stores the request and
//!   withdraws consent now; the deletion itself is the WP-B.9 job.
//!
//! Every mutation appends its [`AuditEvent`] in the same transaction (AUTH-5), and every
//! query is scoped to the device's org: another org's participant does not exist here.

use asd_contract::care::{
    CareEventInput, CareEventsRequest, CareEventsResponse, CareSummary, ErasureRequestResponse,
    MAX_EVENTS_PER_BATCH,
};
use asd_contract::problem as slugs;
use chrono::{DateTime, Utc};
use http::StatusCode;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::app::AppState;
use crate::audit::AuditEvent;
use crate::auth::extractors::{DevicePrincipal, RequestMeta};
use crate::error::{AppError, AppResult};
use crate::ingest::idempotency::{self, Claim};
use crate::ingest::{ensure_not_withdrawn, validate};
use crate::problem::InvalidParam;
use crate::read::cursor;

/// Longest accepted per-event or batch idempotency key.
pub const MAX_KEY_LEN: usize = 128;
/// Most distinct moods in a summary.
const RECENT_MOODS: i64 = 10;

fn invalid(name: impl Into<String>, reason: impl Into<String>) -> AppError {
    AppError::Validation(vec![InvalidParam {
        name: name.into(),
        reason: reason.into(),
    }])
}

/// A validated event: the instant is resolved, the payload is an object.
struct Event<'a> {
    input: &'a CareEventInput,
    occurred_at: DateTime<Utc>,
}

/// Checks the whole batch and reports every problem at once.
fn validate_batch(req: &CareEventsRequest, now: DateTime<Utc>) -> AppResult<Vec<Event<'_>>> {
    let mut errors = Vec::new();
    if !validate::is_safe_id(&req.participant_token) {
        errors.push(InvalidParam {
            name: "/participant_token".into(),
            reason: format!(
                "must be 1-{} characters of [A-Za-z0-9._~-]",
                validate::MAX_ID_LEN
            ),
        });
    }
    if req.events.is_empty() || req.events.len() > MAX_EVENTS_PER_BATCH {
        errors.push(InvalidParam {
            name: "/events".into(),
            reason: format!("must hold 1-{MAX_EVENTS_PER_BATCH} events"),
        });
    }
    let mut events = Vec::with_capacity(req.events.len().min(MAX_EVENTS_PER_BATCH));
    for (i, input) in req.events.iter().enumerate().take(MAX_EVENTS_PER_BATCH) {
        let mut ok = true;
        if !input.payload.is_object() {
            errors.push(InvalidParam {
                name: format!("/events/{i}/payload"),
                reason: "must be a JSON object".into(),
            });
            ok = false;
        }
        if let Some(key) = &input.idempotency_key {
            if key.is_empty() || key.len() > MAX_KEY_LEN {
                errors.push(InvalidParam {
                    name: format!("/events/{i}/idempotency_key"),
                    reason: format!("must be 1-{MAX_KEY_LEN} characters"),
                });
                ok = false;
            }
        }
        let occurred_at = match &input.occurred_at {
            None => now,
            Some(raw) => match DateTime::parse_from_rfc3339(raw) {
                Ok(at) => at.with_timezone(&Utc),
                Err(_) => {
                    errors.push(InvalidParam {
                        name: format!("/events/{i}/occurred_at"),
                        reason: "must be an RFC 3339 timestamp".into(),
                    });
                    ok = false;
                    now
                }
            },
        };
        if ok {
            events.push(Event { input, occurred_at });
        }
    }
    if errors.is_empty() {
        Ok(events)
    } else {
        Err(AppError::Validation(errors))
    }
}

/// `POST /v1/care/events`: `201` with the stored counts, `200` for a replayed batch key.
pub async fn ingest(
    state: &AppState,
    device: DevicePrincipal,
    meta: &RequestMeta,
    batch_key: Option<&str>,
    req: &CareEventsRequest,
) -> AppResult<(StatusCode, CareEventsResponse)> {
    let events = validate_batch(req, Utc::now())?;
    if let Some(key) = batch_key {
        if key.is_empty() || key.len() > MAX_KEY_LEN {
            return Err(invalid(
                "X-Idempotency-Key",
                format!("must be 1-{MAX_KEY_LEN} characters"),
            ));
        }
    }
    let org = device.org_id;
    let mut tx = state.db.begin().await?;
    ensure_not_withdrawn(&mut tx, org, &req.participant_token).await?;

    let scope = format!("care:{org}");
    if let Some(key) = batch_key {
        let body = serde_json::to_value(req)
            .map_err(|e| AppError::Internal(format!("serialise care batch: {e}")))?;
        let hash = idempotency::request_hash(&body)?;
        match idempotency::claim(&mut tx, &scope, key, &hash).await? {
            Claim::New => {}
            Claim::Mismatch => {
                return Err(AppError::Conflict {
                    slug: slugs::IDEMPOTENCY_KEY_REUSED,
                    detail: "this X-Idempotency-Key was used with a different body".into(),
                })
            }
            Claim::Replay { response, .. } => {
                tx.rollback().await?;
                let mut response: CareEventsResponse = serde_json::from_value(response)
                    .map_err(|e| AppError::Internal(format!("stored response: {e}")))?;
                response.duplicate = true;
                return Ok((StatusCode::OK, response));
            }
        }
    }

    let participant_id = sqlx::query_scalar!(
        "INSERT INTO participants (org_id, token) VALUES ($1, $2)
         ON CONFLICT (org_id, token) DO UPDATE SET token = EXCLUDED.token
         RETURNING id",
        org,
        req.participant_token,
    )
    .fetch_one(&mut *tx)
    .await?;

    let (mut accepted, mut duplicates) = (0u32, 0u32);
    for event in &events {
        let stored = sqlx::query_scalar!(
            r#"INSERT INTO care_events
                   (org_id, participant_id, kind, occurred_at, payload, idempotency_key)
               VALUES ($1, $2, $3, $4, $5, $6)
               ON CONFLICT (org_id, idempotency_key) DO NOTHING
               RETURNING 1 AS "one!: i32""#,
            org,
            participant_id,
            event.input.kind.as_str(),
            event.occurred_at,
            event.input.payload,
            event.input.idempotency_key,
        )
        .fetch_optional(&mut *tx)
        .await?;
        if stored.is_some() {
            accepted += 1;
        } else {
            duplicates += 1;
        }
    }

    let response = CareEventsResponse {
        participant_token: req.participant_token.clone(),
        accepted,
        duplicates,
        duplicate: false,
    };
    if let Some(key) = batch_key {
        let stored = serde_json::to_value(&response)
            .map_err(|e| AppError::Internal(format!("serialise response: {e}")))?;
        idempotency::complete(&mut tx, &scope, key, 201, &stored).await?;
    }
    AuditEvent::device(&device, "care.events.ingest", "participant", participant_id)
        .meta(meta)
        .after(json!({"accepted": accepted, "duplicates": duplicates}))
        .record(&mut tx)
        .await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, response))
}

fn count(n: i64) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// `GET /v1/participants/{token}/summary`; all zeros for a token the org has no events for.
pub async fn summary(pool: &PgPool, org: Uuid, token: &str) -> AppResult<CareSummary> {
    let c = sqlx::query!(
        r#"SELECT
               count(*) AS "total!",
               count(*) FILTER (WHERE e.kind = 'task') AS "tasks!",
               count(*) FILTER (WHERE e.kind = 'medication') AS "medications!",
               count(*) FILTER (WHERE e.kind = 'diary') AS "diaries!",
               count(*) FILTER (WHERE e.kind = 'milestone') AS "milestones!",
               count(*) FILTER (WHERE e.kind = 'task' AND (
                   position('session' IN coalesce(e.payload ->> 'task_id', '')) > 0
                   OR e.payload -> 'metadata' ->> 'type' = 'session')) AS "sessions!",
               count(*) FILTER (WHERE e.kind = 'task' AND (
                   position('checkin' IN coalesce(e.payload ->> 'task_id', '')) > 0
                   OR e.payload -> 'metadata' ->> 'type' = 'checkin')) AS "checkins!"
           FROM care_events e
           JOIN participants p ON p.id = e.participant_id
           WHERE p.org_id = $1 AND p.token = $2"#,
        org,
        token,
    )
    .fetch_one(pool)
    .await?;
    let moods = sqlx::query_scalar!(
        r#"SELECT e.payload ->> 'mood' AS "mood!"
           FROM care_events e
           JOIN participants p ON p.id = e.participant_id
           WHERE p.org_id = $1 AND p.token = $2 AND e.kind = 'diary'
             AND coalesce(e.payload ->> 'mood', '') <> ''
           GROUP BY e.payload ->> 'mood'
           ORDER BY max(e.occurred_at) DESC, e.payload ->> 'mood'
           LIMIT $3"#,
        org,
        token,
        RECENT_MOODS,
    )
    .fetch_all(pool)
    .await?;
    Ok(CareSummary {
        participant_token: token.to_owned(),
        total_events: count(c.total),
        task_completions: count(c.tasks),
        medication_doses: count(c.medications),
        diary_entries: count(c.diaries),
        milestones: count(c.milestones),
        session_count: count(c.sessions),
        checkin_count: count(c.checkins),
        completion_rate_percent: if c.tasks > 0 { 100.0 } else { 0.0 },
        recent_moods: moods,
    })
}

/// `POST /v1/participants/{token}/erasure`: `202` for a new request, `200` when one is
/// already open. Unknown tokens (and other orgs' tokens) are `404`.
pub async fn request_erasure(
    state: &AppState,
    device: DevicePrincipal,
    meta: &RequestMeta,
    token: &str,
) -> AppResult<(StatusCode, ErasureRequestResponse)> {
    let org = device.org_id;
    let mut tx = state.db.begin().await?;
    // The row lock serialises concurrent requests for one participant.
    let participant_id = sqlx::query_scalar!(
        "SELECT id FROM participants WHERE org_id = $1 AND token = $2 FOR UPDATE",
        org,
        token,
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::NotFound)?;

    let open = sqlx::query!(
        "SELECT id, scope, requested_at FROM erasure_requests
         WHERE participant_id = $1 AND completed_at IS NULL",
        participant_id,
    )
    .fetch_optional(&mut *tx)
    .await?;
    if let Some(open) = open {
        tx.rollback().await?;
        return Ok((
            StatusCode::OK,
            ErasureRequestResponse {
                id: open.id.to_string(),
                participant_token: token.to_owned(),
                scope: open.scope,
                requested_at: cursor::ts(open.requested_at),
                completed_at: None,
                duplicate: true,
            },
        ));
    }

    let row = sqlx::query!(
        "INSERT INTO erasure_requests (org_id, participant_id, requested_by_type, requested_by)
         VALUES ($1, $2, 'device', $3)
         RETURNING id, scope, requested_at",
        org,
        participant_id,
        device.device_id,
    )
    .fetch_one(&mut *tx)
    .await?;
    sqlx::query!(
        "UPDATE participants SET withdrawn_at = coalesce(withdrawn_at, now()) WHERE id = $1",
        participant_id,
    )
    .execute(&mut *tx)
    .await?;
    AuditEvent::device(
        &device,
        "participant.erasure_request",
        "participant",
        participant_id,
    )
    .meta(meta)
    .after(json!({"erasure_request_id": row.id, "scope": row.scope}))
    .record(&mut tx)
    .await?;
    tx.commit().await?;
    Ok((
        StatusCode::ACCEPTED,
        ErasureRequestResponse {
            id: row.id.to_string(),
            participant_token: token.to_owned(),
            scope: row.scope,
            requested_at: cursor::ts(row.requested_at),
            completed_at: None,
            duplicate: false,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use asd_contract::care::CareEventKind;
    use serde_json::Value;

    fn req(events: Vec<CareEventInput>) -> CareEventsRequest {
        CareEventsRequest {
            participant_token: "kid-1".into(),
            events,
        }
    }

    fn ev(payload: Value, at: Option<&str>, key: Option<&str>) -> CareEventInput {
        CareEventInput {
            kind: CareEventKind::Task,
            occurred_at: at.map(str::to_owned),
            payload,
            idempotency_key: key.map(str::to_owned),
        }
    }

    fn names(err: AppError) -> Vec<String> {
        match err {
            AppError::Validation(p) => p.into_iter().map(|p| p.name).collect(),
            other => panic!("not a validation error: {other:?}"),
        }
    }

    #[test]
    fn valid_batch_resolves_times() {
        let now = Utc::now();
        let r = req(vec![
            ev(json!({}), None, None),
            ev(
                json!({"a": 1}),
                Some("2026-10-01T08:00:00+02:00"),
                Some("k"),
            ),
        ]);
        let events = validate_batch(&r, now).unwrap();
        assert_eq!(events[0].occurred_at, now);
        assert_eq!(
            events[1].occurred_at.to_rfc3339(),
            "2026-10-01T06:00:00+00:00"
        );
    }

    #[test]
    fn every_problem_is_named() {
        let mut r = req(vec![
            ev(json!("x"), Some("soon"), Some("")),
            ev(json!({}), None, None),
        ]);
        r.participant_token = "no good".into();
        let n = names(validate_batch(&r, Utc::now()).err().unwrap());
        assert_eq!(
            n,
            [
                "/participant_token",
                "/events/0/payload",
                "/events/0/idempotency_key",
                "/events/0/occurred_at"
            ]
        );
        assert_eq!(
            names(validate_batch(&req(vec![]), Utc::now()).err().unwrap()),
            ["/events"]
        );
    }
}
