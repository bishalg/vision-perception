//! owner: WP-X.3
//!
//! RUN-8 for every job: `POST /v1/jobs/{id}/cancel` (`JobRun`, same org) cancels a queued
//! job at once (and its analysis run), records the request on a running one (the handler
//! stops where it checks; other kinds run to the end), answers `409 job-finished` on a
//! finished one; `GET /v1/jobs/{id}` returns `progress`. Audit `job.cancel`. Needs
//! `DATABASE_URL`; skipped otherwise.

mod analysis_support;
mod common;
mod research_support;

use analysis_support::*;
use asd_backend::jobs::{self, runner, KIND_NORM_AGGREGATE};
use asd_contract::api_v1::Role;
use http::{Method, StatusCode};
use research_support::*;
use serde_json::{json, Value};
use uuid::Uuid;

async fn enqueue(c: &Ctx, org: Uuid) -> Uuid {
    let mut conn = c.db.pool.acquire().await.unwrap();
    jobs::enqueue(&mut conn, KIND_NORM_AGGREGATE, Some(org), &json!({}), None)
        .await
        .unwrap()
        .id
}

async fn cancel(c: &Ctx, job: Uuid, bearer: &str) -> (StatusCode, Value) {
    c.call(
        Method::POST,
        &format!("/v1/jobs/{job}/cancel"),
        None,
        bearer,
    )
    .await
}

#[tokio::test]
async fn queued_running_and_finished_jobs() {
    let Some(c) = Ctx::new().await else { return };
    let (_, admin) = c.token(Role::Admin).await;

    // Queued: cancelled at once, never claimed.
    let queued = enqueue(&c, c.org).await;
    let (s, b) = cancel(&c, queued, &admin).await;
    assert_eq!(s, StatusCode::ACCEPTED, "{b}");
    assert_eq!(b["status"], "cancelled");
    assert!(b["finished_at"].as_str().is_some());
    assert!(runner::claim_next(&c.db.pool, "w").await.unwrap().is_none());
    let audit = c.audit_rows("job.cancel").await;
    assert_eq!(audit.len(), 1);
    assert_eq!(audit[0]["after"]["outcome"], "cancelled");

    // Running: the request is recorded; progress is visible.
    let running = enqueue(&c, c.org).await;
    let job = runner::claim_next(&c.db.pool, "w").await.unwrap().unwrap();
    assert_eq!(job.id, running);
    c.job_ctx()
        .for_job(&job)
        .progress(2, 5, "pages")
        .await
        .unwrap();
    let (s, b) = cancel(&c, running, &admin).await;
    assert_eq!(s, StatusCode::ACCEPTED, "{b}");
    assert_eq!(b["status"], "running");
    assert_eq!(
        b["progress"],
        json!({"done": 2, "total": 5, "phase": "pages"})
    );
    assert!(c.job_ctx().for_job(&job).cancelled().await.unwrap());
    let (s, b) = c
        .call(Method::GET, &format!("/v1/jobs/{running}"), None, &admin)
        .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(b["progress"]["done"], 2);
    // A second request while it runs is accepted again (idempotent).
    let (s, _) = cancel(&c, running, &admin).await;
    assert_eq!(s, StatusCode::ACCEPTED);

    // Finished: 409.
    sqlx::query("UPDATE jobs SET status = 'succeeded', finished_at = now() WHERE id = $1")
        .bind(running)
        .execute(&c.db.pool)
        .await
        .unwrap();
    let (s, b) = cancel(&c, running, &admin).await;
    assert_eq!(s, StatusCode::CONFLICT, "{b}");
    assert_eq!(common::auth::slug(&b), "job-finished");
    let (s, _) = cancel(&c, queued, &admin).await;
    assert_eq!(s, StatusCode::CONFLICT);
    assert_eq!(c.audit_rows("job.cancel").await.len(), 3);
    c.db.cleanup().await;
}

#[tokio::test]
async fn cancel_is_org_scoped_and_needs_job_run() {
    let Some(c) = Ctx::new().await else { return };
    let other = enqueue(&c, c.other).await;
    let mine = enqueue(&c, c.org).await;
    let (_, admin) = c.token(Role::Admin).await;
    let (s, _) = cancel(&c, other, &admin).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (s, _) = cancel(&c, Uuid::new_v4(), &admin).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (_, clinician) = c.token(Role::Clinician).await;
    let (s, _) = cancel(&c, mine, &clinician).await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, _) = c
        .call(Method::POST, "/v1/jobs/not-a-uuid/cancel", None, &admin)
        .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let still: String = sqlx::query_scalar("SELECT status FROM jobs WHERE id = $1")
        .bind(mine)
        .fetch_one(&c.db.pool)
        .await
        .unwrap();
    assert_eq!(still, "queued");
    c.db.cleanup().await;
}

#[tokio::test]
async fn cancelling_a_queued_analysis_job_cancels_its_run() {
    let Some(c) = Ctx::new().await else { return };
    cohort(&c, None).await;
    let (_, researcher) = c.token(Role::Researcher).await;
    let (exp_id, _) = experiment(&c, &researcher).await;
    let (s, run) = create_run(
        &c,
        json!({"kind": "cohort_rescore", "experiment_id": exp_id, "cohort": {}, "level": "score"}),
        &researcher,
    )
    .await;
    assert_eq!(s, StatusCode::ACCEPTED, "{run}");
    let job: Uuid = run["job_id"].as_str().unwrap().parse().unwrap();
    let (s, b) = cancel(&c, job, &researcher).await;
    assert_eq!(s, StatusCode::ACCEPTED, "{b}");
    let run = get_run(&c, run["id"].as_str().unwrap(), &researcher).await;
    assert_eq!(run["status"], "cancelled");
    assert_eq!(c.run_jobs().await, 0);
    c.db.cleanup().await;
}
