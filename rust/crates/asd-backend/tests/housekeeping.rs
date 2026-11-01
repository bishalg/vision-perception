//! owner: WP-B.12
//!
//! The housekeeping sweep (HK-1..5, acceptance 20): seeded delete/keep pairs, replay of a
//! swept idempotency key, rotation with a swept-eligible code, dry run, the superadmin-only
//! route, one scheduled job under concurrent workers and no audit row for an empty
//! scheduled run. Needs `DATABASE_URL`.

mod common;
mod ingest_support;

use asd_backend::jobs::{housekeeping, runner, KIND_HOUSEKEEPING};
use http::{Method, StatusCode};
use ingest_support::*;
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

use crate::common::auth::{json as json_req, seed_enrol_code};

async fn setup() -> Option<(common::TestDb, Uuid)> {
    let db = common::TestDb::migrated().await?;
    let org = seed_quiet_org(&db.pool).await;
    Some((db, org))
}

async fn key_exists(pool: &PgPool, key: &str) -> bool {
    sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM idempotency_keys WHERE key = $1)")
        .bind(key)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn code_exists(pool: &PgPool, id: Uuid) -> bool {
    sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM enrol_codes WHERE id = $1)")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn job_exists(pool: &PgPool, id: Uuid) -> bool {
    sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM jobs WHERE id = $1)")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn audit_rows(pool: &PgPool, action: &str) -> Vec<Value> {
    sqlx::query_scalar("SELECT to_jsonb(a) FROM audit_log a WHERE action = $1 ORDER BY id")
        .bind(action)
        .fetch_all(pool)
        .await
        .unwrap()
}

/// Registers a device with a fresh code; returns `(code, device_id, code_id)`.
async fn enrolled(pool: &PgPool, org: Uuid) -> (String, String, Uuid) {
    let code = seed_enrol_code(pool, org, 1).await;
    let (status, _, body) = common::send(
        common::router(state(pool.clone())),
        json_req(
            Method::POST,
            "/v1/devices/register",
            &json!({"enrol_code": code, "platform": "iOS", "app_version": "1.0"}),
            None,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let id = body["device_id"].as_str().unwrap().to_owned();
    let code_id = sqlx::query_scalar("SELECT enrol_code_id FROM devices WHERE id = $1::uuid")
        .bind(&id)
        .fetch_one(pool)
        .await
        .unwrap();
    (code, id, code_id)
}

async fn queue_and_run(pool: &PgPool, superadmin: &str, body: Value) -> Value {
    let router = common::router(state(pool.clone()));
    let (status, _, job) = common::send(
        router,
        user_json(superadmin, "POST", "/v1/jobs/housekeeping", &body),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{job}");
    assert_eq!(job["kind"], "housekeeping_sweep");
    assert_eq!(runner::run_pending(pool, "hk-test").await.unwrap(), 1);
    let id: Uuid = job["id"].as_str().unwrap().parse().unwrap();
    let (status, result): (String, Value) =
        sqlx::query_as("SELECT status, result FROM jobs WHERE id = $1")
            .bind(id)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(status, "succeeded");
    result
}

#[tokio::test]
async fn acceptance_20_sweep_deletes_unreferenced_rows_and_keeps_referenced_ones() {
    let Some((db, org)) = setup().await else {
        return;
    };
    let pool = &db.pool;
    let (user, _) = seed_user(pool, Some(org), "admin").await;
    let (_, superadmin) = seed_user(pool, None, "superadmin").await;

    // Idempotency keys: 8 days old (gone) and fresh (kept).
    for (k, age) in [("old", "8 days"), ("fresh", "1 day")] {
        sqlx::query(
            "INSERT INTO idempotency_keys (scope, key, request_hash, created_at)
             VALUES ('s', $1, sha256('x'::bytea), now() - $2::interval)",
        )
        .bind(k)
        .bind(age)
        .execute(pool)
        .await
        .unwrap();
    }
    for (n, offset) in [("gone", "-2 days"), ("kept", "1 day")] {
        sqlx::query(
            "INSERT INTO refresh_tokens (user_id, family_id, token_hash, expires_at)
             VALUES ($1, gen_random_uuid(), sha256($2::bytea), now() + $3::interval)",
        )
        .bind(user)
        .bind(n.as_bytes())
        .bind(offset)
        .execute(pool)
        .await
        .unwrap();
    }
    // Enrol codes expired 91 days ago: one unreferenced, one a device enrolled with.
    let unreferenced = seed_enrol_code(pool, org, 1).await;
    let unreferenced_hash = asd_backend::auth::enrol_code_hash(&unreferenced);
    let (_, _, referenced_id) = enrolled(pool, org).await;
    sqlx::query("UPDATE enrol_codes SET expires_at = now() - interval '91 days'")
        .execute(pool)
        .await
        .unwrap();
    // Jobs finished 31 days ago: one free, one a norm run references.
    let mut jobs = vec![];
    for _ in 0..2 {
        let id: Uuid = sqlx::query_scalar(
            "INSERT INTO jobs (kind, status, finished_at)
             VALUES ('norm_aggregate', 'succeeded', now() - interval '31 days') RETURNING id",
        )
        .fetch_one(pool)
        .await
        .unwrap();
        jobs.push(id);
    }
    sqlx::query(
        "INSERT INTO norm_runs (job_id, params, cells, n_total) VALUES ($1, '{}', '[]', 0)",
    )
    .bind(jobs[1])
    .execute(pool)
    .await
    .unwrap();

    let result = queue_and_run(pool, &superadmin, json!({})).await;
    assert_eq!(
        result,
        json!({"idempotency_keys": 1, "refresh_tokens": 1, "enrol_codes": 1, "jobs": 1,
               "totp_pending": 0, "dry_run": false})
    );
    assert!(!key_exists(pool, "old").await);
    assert!(key_exists(pool, "fresh").await);
    // The two logins' own live tokens stay with the seeded live one; the expired one is gone.
    let tokens: i64 =
        sqlx::query_scalar("SELECT count(*) FROM refresh_tokens WHERE expires_at < now()")
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(tokens, 0);
    let kept: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM refresh_tokens WHERE token_hash = sha256('kept'::bytea)",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(kept, 1);
    let gone: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM enrol_codes WHERE code_hash = $1)")
            .bind(unreferenced_hash.as_slice())
            .fetch_one(pool)
            .await
            .unwrap();
    assert!(!gone);
    assert!(code_exists(pool, referenced_id).await);
    assert!(!job_exists(pool, jobs[0]).await);
    assert!(job_exists(pool, jobs[1]).await);

    let audit = audit_rows(pool, "housekeeping.sweep").await;
    assert_eq!(audit.len(), 1);
    assert!(audit[0]["org_id"].is_null());
    assert_eq!(audit[0]["after"]["idempotency_keys"], 1);
    assert_eq!(audit[0]["after"]["jobs"], 1);
    db.cleanup().await;
}

#[tokio::test]
async fn replay_of_a_swept_session_is_200_duplicate_and_rotation_with_a_swept_eligible_code_works()
{
    let Some((db, org)) = setup().await else {
        return;
    };
    let pool = &db.pool;
    let (_, superadmin) = seed_user(pool, None, "superadmin").await;
    let (code, device_id, _) = enrolled(pool, org).await;
    // Re-register once to get a usable bearer for the device.
    let (status, _, rotated) = common::send(
        common::router(state(pool.clone())),
        json_req(
            Method::POST,
            "/v1/devices/register",
            &json!({"enrol_code": code, "platform": "iOS", "app_version": "1.0", "device_id": device_id}),
            None,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{rotated}");
    let bearer_token = rotated["device_token"].as_str().unwrap().to_owned();

    let session = golden_with_id("SESS-HK-1");
    let router = common::router(state(pool.clone()));
    let first = ingest_ok(&router, &bearer_token, &session).await;
    assert_eq!(first["duplicate"], false);

    sqlx::query("UPDATE idempotency_keys SET created_at = now() - interval '8 days'")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("UPDATE enrol_codes SET expires_at = now() - interval '91 days'")
        .execute(pool)
        .await
        .unwrap();
    let result = queue_and_run(pool, &superadmin, json!({})).await;
    assert_eq!(result["idempotency_keys"], 1);
    assert_eq!(result["enrol_codes"], 0, "the device's code is referenced");

    let (status, _, again) =
        common::send(router.clone(), post_session(Some(&bearer_token), &session)).await;
    assert_eq!(status, StatusCode::OK, "{again}");
    assert_eq!(again["duplicate"], true);
    let sessions: i64 = sqlx::query_scalar("SELECT count(*) FROM sessions")
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(sessions, 1);

    let (status, _, body) = common::send(
        common::router(state(pool.clone())),
        json_req(
            Method::POST,
            "/v1/devices/register",
            &json!({"enrol_code": code, "platform": "iOS", "app_version": "1.0", "device_id": device_id}),
            None,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["status"], "rotated");
    db.cleanup().await;
}

#[tokio::test]
async fn dry_run_deletes_nothing_and_writes_the_dry_run_audit_row() {
    let Some((db, _)) = setup().await else {
        return;
    };
    let pool = &db.pool;
    let (_, superadmin) = seed_user(pool, None, "superadmin").await;
    sqlx::query(
        "INSERT INTO idempotency_keys (scope, key, request_hash, created_at)
         VALUES ('s', 'old', sha256('x'::bytea), now() - interval '9 days')",
    )
    .execute(pool)
    .await
    .unwrap();
    let result = queue_and_run(pool, &superadmin, json!({"dry_run": true})).await;
    assert_eq!(result["idempotency_keys"], 1);
    assert_eq!(result["dry_run"], true);
    assert!(key_exists(pool, "old").await);
    assert_eq!(
        audit_rows(pool, "housekeeping.sweep_dry_run").await.len(),
        1
    );
    assert!(audit_rows(pool, "housekeeping.sweep").await.is_empty());
    db.cleanup().await;
}

#[tokio::test]
async fn only_a_superadmin_may_queue_a_manual_sweep() {
    let Some((db, org)) = setup().await else {
        return;
    };
    let pool = &db.pool;
    let (_, admin) = seed_user(pool, Some(org), "admin").await;
    let (status, _, _) = common::send(
        common::router(state(pool.clone())),
        user_json(&admin, "POST", "/v1/jobs/housekeeping", &json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let queued: i64 = sqlx::query_scalar("SELECT count(*) FROM jobs WHERE kind = $1")
        .bind(KIND_HOUSEKEEPING)
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(queued, 0);
    db.cleanup().await;
}

#[tokio::test]
async fn concurrent_workers_queue_one_daily_job_and_an_empty_scheduled_run_writes_no_audit() {
    let Some((db, _)) = setup().await else { return };
    let pool = &db.pool;
    let (a, b) = tokio::join!(
        housekeeping::schedule_daily(pool),
        housekeeping::schedule_daily(pool)
    );
    let queued = [a.unwrap(), b.unwrap()];
    assert_eq!(queued.iter().flatten().count(), 1, "{queued:?}");
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM jobs WHERE kind = $1")
        .bind(KIND_HOUSEKEEPING)
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(n, 1);
    assert_eq!(runner::run_pending(pool, "hk-test").await.unwrap(), 1);
    let (status, result): (String, Value) =
        sqlx::query_as("SELECT status, result FROM jobs WHERE kind = $1")
            .bind(KIND_HOUSEKEEPING)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(status, "succeeded");
    assert_eq!(result["idempotency_keys"], 0);
    assert!(audit_rows(pool, "housekeeping.sweep").await.is_empty());
    db.cleanup().await;
}

/// A finished job (`status`, finished `finished_ago` ago) or, for `queued` / `running`, one
/// created long ago that never finished. `run_after` is in the future so the test's
/// `run_pending` claims only the sweep.
async fn seed_job(pool: &PgPool, status: &str, finished_ago: Option<&str>) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO jobs (kind, status, created_at, run_after, started_at, finished_at)
         VALUES ('export', $1, now() - interval '400 days', now() + interval '1 day',
                 CASE WHEN $1 <> 'queued' THEN now() - interval '400 days' END,
                 now() - $2::interval)
         RETURNING id",
    )
    .bind(status)
    .bind(finished_ago)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn seed_export(pool: &PgPool, org: Uuid, job: Option<Uuid>) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO exports (org_id, format, mode, purpose, reason, download_secret, job_id)
         VALUES ($1, 'csv', 'deidentified', 'research', 'housekeeping test export',
                 sha256('s'::bytea), $2)
         RETURNING id",
    )
    .bind(org)
    .bind(job)
    .fetch_one(pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn sweep_keeps_unfinished_recent_and_referenced_jobs() {
    let Some((db, org)) = setup().await else {
        return;
    };
    let pool = &db.pool;
    let (_, superadmin) = seed_user(pool, None, "superadmin").await;

    let queued = seed_job(pool, "queued", None).await;
    let running = seed_job(pool, "running", None).await;
    let recent = seed_job(pool, "succeeded", Some("29 days")).await;
    let by_export = seed_job(pool, "succeeded", Some("31 days")).await;
    let by_delivery = seed_job(pool, "failed", Some("31 days")).await;
    let by_erasure = seed_job(pool, "cancelled", Some("31 days")).await;
    let free = seed_job(pool, "failed", Some("31 days")).await;

    seed_export(pool, org, Some(by_export)).await;
    let export = seed_export(pool, org, None).await;
    let destination: Uuid = sqlx::query_scalar(
        "INSERT INTO integration_destinations (org_id, kind, name)
         VALUES ($1, 'webhook', 'hk') RETURNING id",
    )
    .bind(org)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO export_deliveries (org_id, export_id, destination_id, job_id)
         VALUES ($1, $2, $3, $4)",
    )
    .bind(org)
    .bind(export)
    .bind(destination)
    .bind(by_delivery)
    .execute(pool)
    .await
    .unwrap();
    let participant: Uuid = sqlx::query_scalar(
        "INSERT INTO participants (org_id, token) VALUES ($1, 'hk-p') RETURNING id",
    )
    .bind(org)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO erasure_requests (org_id, participant_id, requested_by_type, job_id)
         VALUES ($1, $2, 'system', $3)",
    )
    .bind(org)
    .bind(participant)
    .bind(by_erasure)
    .execute(pool)
    .await
    .unwrap();

    let result = queue_and_run(pool, &superadmin, json!({})).await;
    assert_eq!(result["jobs"], 1, "{result}");
    assert!(!job_exists(pool, free).await);
    for (name, id) in [
        ("queued", queued),
        ("running", running),
        ("finished 29 days ago", recent),
        ("referenced by an export", by_export),
        ("referenced by a delivery", by_delivery),
        ("referenced by an erasure request", by_erasure),
    ] {
        assert!(job_exists(pool, id).await, "{name} job was deleted");
    }
    db.cleanup().await;
}

#[tokio::test]
async fn sweep_keeps_recently_revoked_codes_and_deletes_old_revoked_ones() {
    let Some((db, org)) = setup().await else {
        return;
    };
    let pool = &db.pool;
    let (_, superadmin) = seed_user(pool, None, "superadmin").await;
    let mut ids = vec![];
    // (revoked ago, expired ago): revoked 30 days ago but expired a year ago stays (the
    // revocation is the later event); revoked 91 days ago and unreferenced goes; a live
    // never-revoked code stays.
    for (revoked, expired) in [
        (Some("30 days"), "365 days"),
        (Some("91 days"), "1 day"),
        (None, "-7 days"),
    ] {
        let id: Uuid = sqlx::query_scalar(
            "INSERT INTO enrol_codes (org_id, code_hash, revoked_at, expires_at)
             VALUES ($1, sha256(gen_random_uuid()::text::bytea), now() - $2::interval,
                     now() - $3::interval)
             RETURNING id",
        )
        .bind(org)
        .bind(revoked)
        .bind(expired)
        .fetch_one(pool)
        .await
        .unwrap();
        ids.push(id);
    }
    let result = queue_and_run(pool, &superadmin, json!({})).await;
    assert_eq!(result["enrol_codes"], 1, "{result}");
    assert!(code_exists(pool, ids[0]).await, "revoked 30 days ago");
    assert!(!code_exists(pool, ids[1]).await, "revoked 91 days ago");
    assert!(code_exists(pool, ids[2]).await, "live");
    db.cleanup().await;
}

#[tokio::test]
async fn sweep_clears_expired_pending_totp_enrolments_and_keeps_live_ones() {
    let Some((db, org)) = setup().await else {
        return;
    };
    let pool = &db.pool;
    let (_, superadmin) = seed_user(pool, None, "superadmin").await;
    let (expired, _) = seed_user(pool, Some(org), "clinician").await;
    let (live, _) = seed_user(pool, Some(org), "clinician").await;
    for (user, offset) in [(expired, "-1 minute"), (live, "5 minutes")] {
        sqlx::query(
            "UPDATE users SET totp_pending_ct = '\\x00'::bytea, totp_pending_kek_id = 'kek-x',
                    totp_pending_expires_at = now() + $2::interval
             WHERE id = $1",
        )
        .bind(user)
        .bind(offset)
        .execute(pool)
        .await
        .unwrap();
    }
    let pending = |user: Uuid| async move {
        sqlx::query_scalar::<_, bool>(
            "SELECT totp_pending_ct IS NOT NULL AND totp_pending_kek_id IS NOT NULL
                    AND totp_pending_expires_at IS NOT NULL
             FROM users WHERE id = $1",
        )
        .bind(user)
        .fetch_one(pool)
        .await
        .unwrap()
    };

    let result = queue_and_run(pool, &superadmin, json!({"dry_run": true})).await;
    assert_eq!(result["totp_pending"], 1, "{result}");
    assert!(pending(expired).await, "a dry run changes nothing");

    let result = queue_and_run(pool, &superadmin, json!({})).await;
    assert_eq!(result["totp_pending"], 1, "{result}");
    assert!(!pending(expired).await);
    assert!(pending(live).await);
    let audit = audit_rows(pool, "housekeeping.sweep").await;
    assert_eq!(audit[0]["after"]["totp_pending"], 1);
    db.cleanup().await;
}
