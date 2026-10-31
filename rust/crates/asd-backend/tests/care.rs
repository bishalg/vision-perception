//! owner: WP-B.6
//!
//! `POST /v1/care/events` (idempotent batch), `GET /v1/participants/{token}/summary`,
//! `POST /v1/participants/{token}/erasure` (request row only; execution is WP-B.9).
//!
//! Ported semantics of the removed `shims.py` CareKit sync: per-key dedupe, task /
//! medication / diary counts, session and check-in task detection, distinct recent moods,
//! all-zero summary for an unknown token. Org scoping and audit rows follow AUTH-4/AUTH-5.
//! Needs Postgres (`DATABASE_URL`); skipped otherwise, except the no-bearer 401.

mod common;
mod ingest_support;

use common::auth::{audit_count, empty, json};
use common::{send, TestDb};
use http::{Method, StatusCode};
use ingest_support::*;
use serde_json::{json, Value};
use uuid::Uuid;

struct Fixture {
    db: TestDb,
    token: String,
}

impl Fixture {
    async fn new() -> Option<Self> {
        let db = TestDb::migrated().await?;
        let org = seed_quiet_org(&db.pool).await;
        let (_, token) = seed_device(&db.pool, org).await;
        Some(Self { db, token })
    }
    fn router(&self) -> axum::Router {
        app(state(self.db.pool.clone()))
    }
    async fn post(&self, body: &Value) -> (StatusCode, Value) {
        self.post_with(&self.token, None, body).await
    }
    async fn post_with(
        &self,
        device: &str,
        key: Option<&str>,
        body: &Value,
    ) -> (StatusCode, Value) {
        let mut req = json(Method::POST, "/v1/care/events", body, Some(device));
        if let Some(k) = key {
            req.headers_mut()
                .insert("x-idempotency-key", k.parse().unwrap());
        }
        let (s, _, b) = send(self.router(), req).await;
        (s, b)
    }
    async fn summary(&self, device: &str, token: &str) -> (StatusCode, Value) {
        let uri = format!("/v1/participants/{token}/summary");
        let (s, _, b) = send(self.router(), empty(Method::GET, &uri, Some(device))).await;
        (s, b)
    }
    async fn erase(&self, device: &str, token: &str) -> (StatusCode, Value) {
        let uri = format!("/v1/participants/{token}/erasure");
        let (s, _, b) = send(self.router(), empty(Method::POST, &uri, Some(device))).await;
        (s, b)
    }
    async fn count(&self, sql: &'static str) -> i64 {
        sqlx::query_scalar(sql)
            .fetch_one(&self.db.pool)
            .await
            .unwrap()
    }
}

fn batch(token: &str, events: Value) -> Value {
    json!({"participant_token": token, "events": events})
}

fn task(id: &str) -> Value {
    json!({"kind": "task", "payload": {"task_id": id}})
}

#[tokio::test]
async fn without_a_device_bearer_every_route_is_401() {
    for (method, uri) in [
        (Method::POST, "/v1/care/events"),
        (Method::GET, "/v1/participants/p1/summary"),
        (Method::POST, "/v1/participants/p1/erasure"),
    ] {
        let req = json(method, uri, &json!({}), None);
        let (status, _, body) = send(app(common::state_without_db()), req).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{uri}");
        assert_eq!(problem_slug(&body), "unauthorized");
    }
}

#[tokio::test]
async fn batch_is_stored_and_counted_by_kind() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    let (status, body) = f
        .post(&batch(
            "kid-1",
            json!([
                task("session-1"),
                {"kind": "task", "payload": {"task_id": "x", "metadata": {"type": "checkin"}}},
                {"kind": "medication", "payload": {"name": "m"}, "occurred_at": "2026-10-01T08:00:00Z"},
                {"kind": "diary", "payload": {"mood": "happy"}},
                {"kind": "milestone", "payload": {"title": "first word"}},
            ]),
        ))
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(
        body,
        json!({"participant_token": "kid-1", "accepted": 5, "duplicates": 0, "duplicate": false})
    );
    assert_eq!(f.count("SELECT count(*) FROM care_events").await, 5);
    assert_eq!(f.count("SELECT count(*) FROM participants").await, 1);
    // The explicit occurred_at is kept.
    let at: String = sqlx::query_scalar(
        "SELECT to_char(occurred_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI') \
         FROM care_events WHERE kind = 'medication'",
    )
    .fetch_one(&f.db.pool)
    .await
    .unwrap();
    assert_eq!(at, "2026-10-01T08:00");
    assert_eq!(audit_count(&f.db.pool, "care.events.ingest").await, 1);
    f.db.cleanup().await;
}

#[tokio::test]
async fn per_event_idempotency_key_is_deduped_across_batches_and_within_one() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    let ev = |k: &str| json!({"kind": "task", "payload": {"task_id": "t"}, "idempotency_key": k});
    let (s, b) = f
        .post(&batch("kid", json!([ev("a"), ev("b"), ev("a")])))
        .await;
    assert_eq!(s, StatusCode::CREATED, "{b}");
    assert_eq!(
        (b["accepted"].as_u64(), b["duplicates"].as_u64()),
        (Some(2), Some(1))
    );
    let (s, b) = f.post(&batch("kid", json!([ev("a"), ev("c")]))).await;
    assert_eq!(s, StatusCode::CREATED);
    assert_eq!(
        (b["accepted"].as_u64(), b["duplicates"].as_u64()),
        (Some(1), Some(1))
    );
    assert_eq!(f.count("SELECT count(*) FROM care_events").await, 3);
    f.db.cleanup().await;
}

#[tokio::test]
async fn batch_header_key_replays_and_rejects_a_changed_body() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    let body = batch("kid", json!([task("session-1")]));
    let (s, b) = f.post_with(&f.token, Some("batch-1"), &body).await;
    assert_eq!(s, StatusCode::CREATED, "{b}");
    let (s, b) = f.post_with(&f.token, Some("batch-1"), &body).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(b["duplicate"], true);
    assert_eq!(b["accepted"], 1);
    assert_eq!(f.count("SELECT count(*) FROM care_events").await, 1);
    let other = batch("kid", json!([task("session-2")]));
    let (s, b) = f.post_with(&f.token, Some("batch-1"), &other).await;
    assert_eq!(s, StatusCode::CONFLICT);
    assert_eq!(problem_slug(&b), "idempotency-key-reused");
    // A different org may reuse the key.
    let org2 = seed_quiet_org(&f.db.pool).await;
    let (_, dev2) = seed_device(&f.db.pool, org2).await;
    let (s, _) = f.post_with(&dev2, Some("batch-1"), &body).await;
    assert_eq!(s, StatusCode::CREATED);
    f.db.cleanup().await;
}

#[tokio::test]
async fn invalid_batches_are_400_validation_failed() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    let too_many: Vec<Value> = (0..501).map(|_| task("t")).collect();
    for body in [
        batch("kid", json!([])),
        batch("kid", json!(too_many)),
        batch("bad token!", json!([task("t")])),
        batch("kid", json!([{"kind": "sleep", "payload": {}}])),
        batch("kid", json!([{"kind": "task", "payload": "x"}])),
        batch("kid", json!([{"kind": "task", "occurred_at": "yesterday"}])),
        batch("kid", json!([{"kind": "task", "idempotency_key": ""}])),
        json!({"events": [task("t")]}),
    ] {
        let (s, b) = f.post(&body).await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{body:.200}");
        assert_eq!(problem_slug(&b), "validation-failed");
    }
    assert_eq!(f.count("SELECT count(*) FROM care_events").await, 0);
    assert_eq!(f.count("SELECT count(*) FROM participants").await, 0);
    f.db.cleanup().await;
}

#[tokio::test]
async fn summary_matches_the_carekit_port() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    let diary = |mood: &str, at: &str| json!({"kind": "diary", "payload": {"mood": mood}, "occurred_at": at});
    let (s, b) = f
        .post(&batch(
            "kid",
            json!([
                task("session-a"),
                task("daily-checkin"),
                {"kind": "task", "payload": {"task_id": "z", "metadata": {"type": "session"}}},
                task("plain"),
                {"kind": "medication", "payload": {"name": "m"}},
                {"kind": "medication", "payload": {"name": "m"}},
                {"kind": "milestone", "payload": {}},
                diary("calm", "2026-10-01T00:00:00Z"),
                diary("happy", "2026-10-02T00:00:00Z"),
                diary("calm", "2026-10-03T00:00:00Z"),
                {"kind": "diary", "payload": {"note": "no mood"}, "occurred_at": "2026-09-01T00:00:00Z"},
            ]),
        ))
        .await;
    assert_eq!(s, StatusCode::CREATED, "{b}");
    let (s, b) = f.summary(&f.token, "kid").await;
    assert_eq!(s, StatusCode::OK, "{b}");
    assert_eq!(b["participant_token"], "kid");
    assert_eq!(b["total_events"], 11);
    assert_eq!(b["task_completions"], 4);
    assert_eq!(b["medication_doses"], 2);
    assert_eq!(b["diary_entries"], 4);
    assert_eq!(b["milestones"], 1);
    assert_eq!(b["session_count"], 2);
    assert_eq!(b["checkin_count"], 1);
    assert_eq!(b["completion_rate_percent"], 100.0);
    assert_eq!(b["recent_moods"], json!(["calm", "happy"]));
    f.db.cleanup().await;
}

#[tokio::test]
async fn summary_of_an_unknown_token_is_all_zeros_and_org_scoped() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    let (s, b) = f.summary(&f.token, "nobody").await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(b["total_events"], 0);
    assert_eq!(b["completion_rate_percent"], 0.0);
    assert_eq!(b["recent_moods"], json!([]));

    f.post(&batch("kid", json!([task("t")]))).await;
    let org2 = seed_quiet_org(&f.db.pool).await;
    let (_, dev2) = seed_device(&f.db.pool, org2).await;
    let (_, b) = f.summary(&dev2, "kid").await;
    assert_eq!(b["total_events"], 0, "another org sees nothing");
    f.db.cleanup().await;
}

#[tokio::test]
async fn erasure_records_one_open_request_withdraws_consent_and_audits() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    f.post(&batch("kid", json!([task("t")]))).await;
    let (s, b) = f.erase(&f.token, "kid").await;
    assert_eq!(s, StatusCode::ACCEPTED, "{b}");
    assert_eq!(b["participant_token"], "kid");
    assert_eq!(b["scope"], "all");
    assert_eq!(b["duplicate"], false);
    assert!(b["completed_at"].is_null());
    let id = b["id"].as_str().unwrap().to_owned();

    // Repeating returns the open request, no second row, no second audit row.
    let (s, b) = f.erase(&f.token, "kid").await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(b["duplicate"], true);
    assert_eq!(b["id"], id.as_str());
    assert_eq!(f.count("SELECT count(*) FROM erasure_requests").await, 1);
    assert_eq!(
        audit_count(&f.db.pool, "participant.erasure_request").await,
        1
    );

    let by: (String, Option<Uuid>) =
        sqlx::query_as("SELECT requested_by_type, requested_by FROM erasure_requests")
            .fetch_one(&f.db.pool)
            .await
            .unwrap();
    assert_eq!(by.0, "device");
    assert!(by.1.is_some());
    assert_eq!(
        f.count("SELECT count(*) FROM participants WHERE withdrawn_at IS NOT NULL")
            .await,
        1
    );

    // Consent is withdrawn: no more care events or sessions.
    let (s, b) = f.post(&batch("kid", json!([task("t2")]))).await;
    assert_eq!(s, StatusCode::CONFLICT);
    assert_eq!(problem_slug(&b), "participant-withdrawn");
    let mut session = golden_with_id("s-erased");
    session["participantToken"] = json!("kid");
    session["rawPayload"]["participantId"] = json!("kid");
    let (s, _, b) = send(f.router(), post_session(Some(&f.token), &session)).await;
    assert_eq!(s, StatusCode::CONFLICT, "{b}");
    f.db.cleanup().await;
}

#[tokio::test]
async fn erasure_of_an_unknown_or_foreign_participant_is_404() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    let (s, b) = f.erase(&f.token, "nobody").await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    assert_eq!(problem_slug(&b), "not-found");
    f.post(&batch("kid", json!([task("t")]))).await;
    let org2 = seed_quiet_org(&f.db.pool).await;
    let (_, dev2) = seed_device(&f.db.pool, org2).await;
    let (s, _) = f.erase(&dev2, "kid").await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    assert_eq!(f.count("SELECT count(*) FROM erasure_requests").await, 0);
    f.db.cleanup().await;
}

#[tokio::test]
async fn twenty_concurrent_identical_batches_store_once() {
    let Some(f) = Fixture::new().await else {
        return;
    };
    let body = batch("kid", json!([task("t"), task("u")]));
    let router = f.router();
    let mut set = tokio::task::JoinSet::new();
    for _ in 0..20 {
        let (r, tok, body) = (router.clone(), f.token.clone(), body.clone());
        set.spawn(async move {
            let mut req = json(Method::POST, "/v1/care/events", &body, Some(&tok));
            req.headers_mut()
                .insert("x-idempotency-key", "same".parse().unwrap());
            send(r, req).await.0
        });
    }
    let mut created = 0;
    while let Some(s) = set.join_next().await {
        match s.unwrap() {
            StatusCode::CREATED => created += 1,
            StatusCode::OK => {}
            other => panic!("unexpected {other}"),
        }
    }
    assert_eq!(created, 1);
    assert_eq!(f.count("SELECT count(*) FROM care_events").await, 2);
    f.db.cleanup().await;
}
