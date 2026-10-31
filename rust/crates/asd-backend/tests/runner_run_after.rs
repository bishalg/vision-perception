//! owner: WP-I.6
//!
//! Runner changes (G5, EXJ-9, trap 4): a queued job is not claimed before `run_after`; a
//! heartbeating job is not reclaimed by a (short) visibility timeout, and is once the
//! heartbeat stops; a stale run cannot record over a newer claim. Needs `DATABASE_URL`.

mod common;

use std::time::Duration;

use asd_backend::jobs::runner::{self, Heartbeat};
use asd_backend::jobs::{self, KIND_NORM_AGGREGATE};
use common::TestDb;
use serde_json::json;

#[tokio::test]
async fn future_run_after_is_not_claimed() {
    let Some(db) = TestDb::migrated().await else {
        return;
    };
    let pool = &db.pool;
    let mut conn = pool.acquire().await.unwrap();
    let job = jobs::enqueue(&mut conn, KIND_NORM_AGGREGATE, None, &json!({}), None)
        .await
        .unwrap();
    drop(conn);
    sqlx::query("UPDATE jobs SET run_after = now() + interval '1 hour' WHERE id = $1")
        .bind(job.id)
        .execute(pool)
        .await
        .unwrap();
    assert!(runner::claim_next(pool, "w").await.unwrap().is_none());
    assert_eq!(runner::run_pending(pool, "w").await.unwrap(), 0);

    sqlx::query("UPDATE jobs SET run_after = now() - interval '1 second' WHERE id = $1")
        .bind(job.id)
        .execute(pool)
        .await
        .unwrap();
    let claimed = runner::claim_next(pool, "w").await.unwrap().unwrap();
    assert_eq!(claimed.id, job.id);
    db.cleanup().await;
}

#[tokio::test]
async fn heartbeat_prevents_reclaim_until_it_stops() {
    let Some(db) = TestDb::migrated().await else {
        return;
    };
    let pool = &db.pool;
    let mut conn = pool.acquire().await.unwrap();
    jobs::enqueue(&mut conn, KIND_NORM_AGGREGATE, None, &json!({}), None)
        .await
        .unwrap();
    drop(conn);
    let job = runner::claim_next(pool, "w").await.unwrap().unwrap();
    let timeout = Duration::from_millis(400);

    let beat = Heartbeat::start(
        pool.clone(),
        job.id,
        job.attempts,
        Duration::from_millis(50),
    );
    tokio::time::sleep(Duration::from_millis(900)).await;
    assert_eq!(
        runner::reclaim_stale(pool, timeout).await.unwrap(),
        0,
        "a beating job is alive even though it started long ago"
    );
    beat.stop().await;
    tokio::time::sleep(Duration::from_millis(600)).await;
    assert_eq!(runner::reclaim_stale(pool, timeout).await.unwrap(), 1);

    // The old run's heartbeat and success cannot touch the newer claim.
    let again = runner::claim_next(pool, "w2").await.unwrap().unwrap();
    assert_eq!(again.attempts, job.attempts + 1);
    let stale = Heartbeat::start(
        pool.clone(),
        job.id,
        job.attempts,
        Duration::from_millis(20),
    );
    tokio::time::sleep(Duration::from_millis(100)).await;
    stale.stop().await;
    let beat_at: Option<chrono::DateTime<chrono::Utc>> =
        sqlx::query_scalar("SELECT heartbeat_at FROM jobs WHERE id = $1")
            .bind(job.id)
            .fetch_one(pool)
            .await
            .unwrap();
    assert!(beat_at.is_none(), "a stale run's heartbeat is fenced off");
    assert_eq!(
        runner::execute(pool, &job).await.unwrap(),
        runner::Outcome::Failed,
        "a stale run cannot record success"
    );
    let status: String = sqlx::query_scalar("SELECT status FROM jobs WHERE id = $1")
        .bind(job.id)
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(status, "running", "the newer claim is untouched");
    db.cleanup().await;
}
