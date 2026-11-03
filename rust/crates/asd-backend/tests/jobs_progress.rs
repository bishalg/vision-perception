//! owner: WP-X.0
//!
//! RUN-8 seams (phase 8 step 0): `JobContext::progress` writes `jobs.progress` with the
//! heartbeat, fenced by `attempts`; `JobContext::cancelled` reads
//! `jobs.cancel_requested_at`; the `analysis_run` kind is routed by the runner and fails
//! permanently until WP-X.3. Needs `DATABASE_URL`; skipped otherwise.

mod common;

use asd_backend::jobs::{self, runner, JobContext, KINDS, KIND_ANALYSIS};
use common::TestDb;
use serde_json::{json, Value};

#[tokio::test]
async fn progress_is_written_and_cancel_is_read() {
    let Some(db) = TestDb::migrated().await else {
        return;
    };
    let pool = &db.pool;
    let mut conn = pool.acquire().await.unwrap();
    jobs::enqueue(&mut conn, KIND_ANALYSIS, None, &json!({}), None)
        .await
        .unwrap();
    drop(conn);
    let job = runner::claim_next(pool, "w").await.unwrap().unwrap();

    // Unbound contexts refuse (no job to write to).
    let unbound = JobContext::db_only(pool.clone());
    assert!(unbound.progress(1, 2, "x").await.is_err());
    assert!(unbound.cancelled().await.is_err());

    let ctx = unbound.for_job(&job);
    ctx.progress(3, 10, "evaluate").await.unwrap();
    let (progress, beat): (Option<Value>, bool) =
        sqlx::query_as("SELECT progress, heartbeat_at IS NOT NULL FROM jobs WHERE id = $1")
            .bind(job.id)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(
        progress,
        Some(json!({"done": 3, "total": 10, "phase": "evaluate"}))
    );
    assert!(beat, "progress refreshes the heartbeat");

    assert!(!ctx.cancelled().await.unwrap());
    sqlx::query("UPDATE jobs SET cancel_requested_at = now() WHERE id = $1")
        .bind(job.id)
        .execute(pool)
        .await
        .unwrap();
    assert!(ctx.cancelled().await.unwrap());

    // Fenced by attempts: a stale run (an older claim) writes nothing.
    let stale = JobContext::db_only(pool.clone()).for_job(&jobs::Job {
        attempts: job.attempts - 1,
        ..job.clone()
    });
    stale.progress(9, 10, "stale").await.unwrap();
    let progress: Option<Value> = sqlx::query_scalar("SELECT progress FROM jobs WHERE id = $1")
        .bind(job.id)
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(progress.unwrap()["phase"], "evaluate");
    db.cleanup().await;
}

#[tokio::test]
async fn analysis_run_is_a_known_kind_that_fails_until_implemented() {
    let Some(db) = TestDb::migrated().await else {
        return;
    };
    assert!(KINDS.contains(&KIND_ANALYSIS));
    assert_eq!(KIND_ANALYSIS, "analysis_run");
    let pool = &db.pool;
    let mut conn = pool.acquire().await.unwrap();
    let job = jobs::enqueue(&mut conn, KIND_ANALYSIS, None, &json!({}), None)
        .await
        .unwrap();
    drop(conn);
    assert_eq!(runner::run_pending(pool, "w").await.unwrap(), 1);
    let (status, error, attempts): (String, Option<String>, i32) =
        sqlx::query_as("SELECT status, error, attempts FROM jobs WHERE id = $1")
            .bind(job.id)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(status, "failed");
    assert_eq!(error.as_deref(), Some("not implemented"));
    assert_eq!(attempts, 1, "permanent: no retry");
    db.cleanup().await;
}
