//! owner: WP-X.3
//!
//! Analysis runs end to end (RUN-1..6, RUN-8, RUN-9, RPX-3, SWP-2, GOV-1, GOV-3..5;
//! acceptance 10 and 11) over a synthetic cohort of 30 research-consented sessions, 10 with a
//! sealed research frame log (`analysis_support`): `auto` splits by level and the counts add
//! up; `frames` excludes sessions without a log; equal seeds give byte-identical summaries
//! and a re-run reports membership drift; the per-session rows page from the sealed blob and
//! audit once per page; quota, budget and validation refusals; cancellation between pages
//! leaves no blob; the deadline fails the run `budget_exceeded`; the job runs under a
//! database role that may only read sessions and config (GOV-5); RBAC rows. Needs
//! `DATABASE_URL`; skipped otherwise.

mod analysis_support;
mod common;
mod research_support;

use std::time::Duration;

use analysis_support::*;
use asd_backend::auth::rbac::{self, Action};
use asd_backend::jobs::{analysis, runner, JobContext};
use asd_contract::api_v1::Role;
use http::{Method, StatusCode};
use research_support::*;
use serde_json::{json, Value};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{AssertSqlSafe, Executor};
use uuid::Uuid;

fn body(experiment: &str, level: &str, cohort: Value) -> Value {
    json!({
        "kind": "cohort_rescore",
        "experiment_id": experiment,
        "cohort": cohort,
        "level": level,
        "seed": 42,
    })
}

async fn summary_text(c: &Ctx, run: &str) -> String {
    sqlx::query_scalar("SELECT summary::text FROM analysis_runs WHERE id = $1::uuid")
        .bind(run)
        .fetch_one(&c.db.pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn auto_run_splits_by_level_pages_rows_and_audits() {
    let Some(c) = Ctx::new().await else { return };
    cohort(&c, None).await;
    let (_, researcher) = c.token(Role::Researcher).await;
    let (exp_id, exp) = experiment(&c, &researcher).await;
    let before = xmins(&c).await;

    let (s, run) = create_run(&c, body(&exp_id, "auto", json!({})), &researcher).await;
    assert_eq!(s, StatusCode::ACCEPTED, "{run}");
    assert_eq!(run["status"], "queued");
    let id = run["id"].as_str().unwrap().to_owned();
    // RUN-5: every reproducibility field before the job starts; GOV-3: the experiment body.
    assert_eq!(run["profile_hash"], exp["profile_hash"]);
    assert_eq!(run["seed"], 42);
    assert_eq!(run["level_requested"], "auto");
    assert_eq!(run["published_profile_version"], "bundled-1.2.0");
    for f in [
        "definition_hash",
        "cohort_hash",
        "baselines_version",
        "catalog_version",
        "core_version",
        "contract_version",
        "pipeline_version",
        "registry_version",
        "job_id",
    ] {
        assert!(run[f].as_str().is_some_and(|v| !v.is_empty()), "{f}: {run}");
    }
    assert_eq!(run["membership_hash"], Value::Null);
    assert_eq!(c.audit_rows("analysis.run_enqueue").await.len(), 1);
    let last: Option<Uuid> =
        sqlx::query_scalar("SELECT last_run_id FROM experiments WHERE id = $1::uuid")
            .bind(&exp_id)
            .fetch_one(&c.db.pool)
            .await
            .unwrap();
    assert_eq!(last.map(|u| u.to_string()), Some(id.clone()), "GOV-2 link");

    assert_eq!(c.run_jobs().await, 1);
    let run = get_run(&c, &id, &researcher).await;
    assert_eq!(run["status"], "succeeded", "{run}");
    assert_eq!(run["sessions_n"], 30);
    assert_eq!(run["sessions_frames_n"], 10);
    assert_eq!(run["sessions_score_n"], 20);
    assert!(run["membership_hash"].as_str().is_some());
    assert_eq!(
        run["progress"],
        json!({"done": 30, "total": 30, "phase": "done"})
    );
    let counts = &run["summary"]["counts"];
    assert_eq!(counts["sessions"], 30);
    assert_eq!(counts["by_level"], json!({"frames": 10, "score": 20}));
    // RPX-3: the frames knob is not applied at score level, the score knob always is.
    assert_eq!(
        counts["params_not_applied"]["metrics.ivtVelocityUnitsPerSec"], 20,
        "{counts}"
    );
    assert!(counts["params_not_applied"]
        .get("lowConfidenceValidRatio")
        .is_none());
    assert_eq!(run["summary"]["small_cell_k"], 12);
    assert_eq!(run["summary"]["overall"]["n"], 30);
    assert!(run["summary"]["flags"]["transitions"].as_array().is_some());
    assert!(run["result_bytes"].as_u64().unwrap() > 0);
    assert_eq!(c.audit_rows("analysis.read").await.len(), 1);
    let complete = c.audit_rows("analysis.run_complete").await;
    assert_eq!(complete.len(), 1);
    assert_eq!(complete[0]["actor_type"], "system");
    assert_eq!(complete[0]["after"]["sessions"], 30);
    // GOV-3: the publish rule finds the succeeded run over the experiment body.
    let analysed: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM analysis_runs WHERE status = 'succeeded' AND profile_hash = $1)",
    )
    .bind(jcs_hash(&exp["profile"]))
    .fetch_one(&c.db.pool)
    .await
    .unwrap();
    assert!(analysed);
    // GOV-5 / acceptance 10: nothing read was written.
    assert_eq!(before, xmins(&c).await);
    // The job shows the same progress (RUN-8).
    let job_id = run["job_id"].as_str().unwrap();
    let (s, job) = c
        .call(
            Method::GET,
            &format!("/v1/jobs/{job_id}"),
            None,
            &researcher,
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{job}");
    assert_eq!(job["status"], "succeeded");
    assert_eq!(job["progress"]["phase"], "done");

    // Rows: 12 + 12 + 6, one audit per page, levels and deltas.
    let mut uri = format!("/v1/analysis-runs/{id}/sessions?limit=12");
    let mut rows: Vec<Value> = Vec::new();
    let mut pages = 0;
    loop {
        let (s, page) = c.call(Method::GET, &uri, None, &researcher).await;
        assert_eq!(s, StatusCode::OK, "{page}");
        pages += 1;
        rows.extend(page["items"].as_array().unwrap().iter().cloned());
        match page["next_cursor"].as_str() {
            Some(cur) => uri = format!("/v1/analysis-runs/{id}/sessions?limit=12&cursor={cur}"),
            None => break,
        }
    }
    assert_eq!(pages, 3);
    assert_eq!(rows.len(), 30);
    assert_eq!(c.audit_rows("analysis.sessions_read").await.len(), 3);
    let frames: Vec<&Value> = rows.iter().filter(|r| r["level"] == "frames").collect();
    assert_eq!(frames.len(), 10);
    // The frames rows were really reprocessed and scored (no replay failure).
    for r in &frames {
        assert!(r["draft"]["composite"].as_f64().is_some(), "{r}");
        assert!(r["published"]["composite"].as_f64().is_some(), "{r}");
        assert!(
            !r["warnings"].to_string().contains("reprocess_failed"),
            "{r}"
        );
    }
    assert!(frames
        .iter()
        .all(|r| r["session_id"].as_str().unwrap() < "C-10"));
    let score_row = rows.iter().find(|r| r["level"] == "score").unwrap();
    assert!(score_row["warnings"]
        .as_array()
        .unwrap()
        .contains(&json!("param_not_applied: metrics.ivtVelocityUnitsPerSec")));
    for r in &rows {
        assert!(r["age_band"].as_str().is_some(), "{r}");
        assert!(r.get("participant_token").is_none());
        if let (Some(p), Some(d)) = (
            r["published"]["composite"].as_f64(),
            r["draft"]["composite"].as_f64(),
        ) {
            let delta = r["delta"]["composite"].as_f64().unwrap();
            assert!((delta - (d - p)).abs() < 1e-12);
        }
    }
    // WP-X.8 reads exactly what the job wrote (summary, sealed rows, AAD, profile hash).
    let run_uuid: Uuid = id.parse().unwrap();
    let record = asd_backend::export_build::analysis::load_run(&c.db.pool, c.org, run_uuid)
        .await
        .unwrap();
    assert_eq!(record.summary.counts.sessions, 30);
    assert_eq!(record.provenance.profile_hash, exp["profile_hash"]);
    let exported = asd_backend::export_build::analysis::load_rows(
        c.state.blobs.as_ref(),
        c.state.keyring.as_deref().unwrap(),
        c.org,
        run_uuid,
        record.blob_key.as_deref().unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(exported.len(), 30);
    // The blob is ciphertext, not JSON.
    let key = analysis::result_key(c.org, id.parse().unwrap());
    let stored = c.state.blobs.get(&key).await.unwrap().unwrap();
    assert!(serde_json::from_slice::<Value>(&stored.bytes).is_err());
    assert!(!String::from_utf8_lossy(&stored.bytes).contains("C-00"));

    // List: newest first, filters, no summary.
    let (s, list) = c
        .call(
            Method::GET,
            &format!(
                "/v1/orgs/{}/analysis-runs?status=succeeded&experiment_id={exp_id}",
                c.org
            ),
            None,
            &researcher,
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{list}");
    assert_eq!(list["items"].as_array().unwrap().len(), 1);
    assert_eq!(list["items"][0]["summary"], Value::Null);
    let (_, list) = c
        .call(
            Method::GET,
            &format!("/v1/orgs/{}/analysis-runs?status=failed", c.org),
            None,
            &researcher,
        )
        .await;
    assert_eq!(list["items"].as_array().unwrap().len(), 0);
    c.db.cleanup().await;
}

#[tokio::test]
async fn progress_polls_are_not_audited_reads_but_results_are() {
    let Some(c) = Ctx::new().await else { return };
    cohort(&c, None).await;
    let (_, researcher) = c.token(Role::Researcher).await;
    let (exp_id, _) = experiment(&c, &researcher).await;
    let (_, run) = create_run(&c, body(&exp_id, "auto", json!({})), &researcher).await;
    let id = run["id"].as_str().unwrap().to_owned();

    // Queued, then running: the console polls these once a second; no results, no audit row.
    for _ in 0..5 {
        let r = get_run(&c, &id, &researcher).await;
        assert_eq!(r["status"], "queued");
        assert_eq!(r["summary"], Value::Null);
    }
    sqlx::query("UPDATE analysis_runs SET status = 'running' WHERE id = $1::uuid")
        .bind(&id)
        .execute(&c.db.pool)
        .await
        .unwrap();
    for _ in 0..5 {
        let r = get_run(&c, &id, &researcher).await;
        assert_eq!(r["status"], "running");
    }
    assert_eq!(c.audit_rows("analysis.read").await.len(), 0);
    sqlx::query("UPDATE analysis_runs SET status = 'queued' WHERE id = $1::uuid")
        .bind(&id)
        .execute(&c.db.pool)
        .await
        .unwrap();

    // The first read of the finished run is the audited one; the list is never audited.
    assert_eq!(c.run_jobs().await, 1);
    let r = get_run(&c, &id, &researcher).await;
    assert_eq!(r["status"], "succeeded", "{r}");
    assert!(r["summary"].is_object());
    assert_eq!(c.audit_rows("analysis.read").await.len(), 1);
    let (s, _) = c
        .call(
            Method::GET,
            &format!("/v1/orgs/{}/analysis-runs", c.org),
            None,
            &researcher,
        )
        .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(c.audit_rows("analysis.read").await.len(), 1);
    get_run(&c, &id, &researcher).await;
    assert_eq!(c.audit_rows("analysis.read").await.len(), 2);
    c.db.cleanup().await;
}

#[tokio::test]
async fn frames_level_excludes_sessions_without_a_log() {
    let Some(c) = Ctx::new().await else { return };
    let (device, _) = common::auth::seed_device(&c.db.pool, c.org).await;
    // The device's sessions: the 10 with a log and 3 without (C-10..C-12).
    let on: Vec<usize> = (0..13).collect();
    cohort(&c, Some((device, &on))).await;
    let (_, researcher) = c.token(Role::Researcher).await;
    let (exp_id, _) = experiment(&c, &researcher).await;

    let (s, run) = create_run(
        &c,
        body(&exp_id, "frames", json!({"device_ids": [device]})),
        &researcher,
    )
    .await;
    assert_eq!(s, StatusCode::ACCEPTED, "{run}");
    let id = run["id"].as_str().unwrap().to_owned();
    c.run_jobs().await;
    let run = get_run(&c, &id, &researcher).await;
    assert_eq!(run["status"], "succeeded", "{run}");
    assert_eq!(run["sessions_n"], 10);
    assert_eq!(run["sessions_frames_n"], 10);
    assert_eq!(run["sessions_score_n"], 0);
    assert_eq!(run["excluded"], json!({"no_frame_log": 3}));
    assert_eq!(
        run["summary"]["counts"]["excluded"],
        json!({"no_frame_log": 3})
    );
    // Every knob applies at frames level.
    assert_eq!(run["summary"]["counts"]["params_not_applied"], json!({}));

    // `frames` over sessions that have no log at all: refused before queueing.
    let (s, b) = create_run(
        &c,
        body(&exp_id, "frames", json!({"age_bands": ["48-60"]})),
        &researcher,
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{b}");
    assert_eq!(slug(&b), "cohort-empty");
    let (s, b) = create_run(
        &c,
        body(&exp_id, "frames", json!({"device_ids": [Uuid::new_v4()]})),
        &researcher,
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{b}");
    assert_eq!(slug(&b), "cohort-empty");
    sqlx::query("UPDATE sessions SET research_frames_blob_key = NULL, research_frames_bytes = NULL, research_frames_sha256 = NULL")
        .execute(&c.db.pool)
        .await
        .unwrap();
    let (s, b) = create_run(&c, body(&exp_id, "frames", json!({})), &researcher).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{b}");
    assert_eq!(slug(&b), "analysis-level-unavailable");
    c.db.cleanup().await;
}

#[tokio::test]
async fn equal_seed_gives_identical_summary_and_rerun_reports_drift() {
    let Some(c) = Ctx::new().await else { return };
    cohort(&c, None).await;
    let (_, researcher) = c.token(Role::Researcher).await;
    let (exp_id, _) = experiment(&c, &researcher).await;

    let (_, a) = create_run(&c, body(&exp_id, "auto", json!({})), &researcher).await;
    let (_, b) = create_run(&c, body(&exp_id, "auto", json!({})), &researcher).await;
    assert_eq!(c.run_jobs().await, 2);
    let (a, b) = (a["id"].as_str().unwrap(), b["id"].as_str().unwrap());
    let ra = get_run(&c, a, &researcher).await;
    let rb = get_run(&c, b, &researcher).await;
    assert_eq!(ra["definition_hash"], rb["definition_hash"]);
    assert_eq!(ra["membership_hash"], rb["membership_hash"]);
    assert_eq!(
        summary_text(&c, a).await,
        summary_text(&c, b).await,
        "RUN-5"
    );

    // Re-run with no change: same membership, same summary, drift zero.
    let (s, r1) = c
        .call(
            Method::POST,
            &format!("/v1/analysis-runs/{a}/rerun"),
            None,
            &researcher,
        )
        .await;
    assert_eq!(s, StatusCode::ACCEPTED, "{r1}");
    assert_eq!(r1["definition_hash"], ra["definition_hash"]);
    assert_eq!(r1["profile_hash"], ra["profile_hash"]);
    assert_eq!(
        r1["membership_drift"],
        Value::Null,
        "known once it finishes"
    );
    c.run_jobs().await;
    let r1 = get_run(&c, r1["id"].as_str().unwrap(), &researcher).await;
    assert_eq!(r1["membership_drift"], json!({"added": 0, "removed": 0}));
    assert_eq!(
        r1["published_profile_version"],
        ra["published_profile_version"]
    );
    assert_eq!(r1["baselines_version"], ra["baselines_version"]);
    assert_eq!(
        summary_text(&c, r1["id"].as_str().unwrap()).await,
        summary_text(&c, a).await
    );

    // One more consented session: the re-run reports it.
    add_session(&c, c.org, "C-NEW", 99, 30, None).await;
    let (s, r2) = c
        .call(
            Method::POST,
            &format!("/v1/analysis-runs/{a}/rerun"),
            None,
            &researcher,
        )
        .await;
    assert_eq!(s, StatusCode::ACCEPTED, "{r2}");
    c.run_jobs().await;
    let r2 = get_run(&c, r2["id"].as_str().unwrap(), &researcher).await;
    assert_eq!(r2["status"], "succeeded");
    assert_eq!(r2["membership_drift"], json!({"added": 1, "removed": 0}));
    assert_eq!(r2["sessions_n"], 31);
    assert_ne!(r2["membership_hash"], ra["membership_hash"]);
    let enqueues = c.audit_rows("analysis.run_enqueue").await;
    assert_eq!(enqueues.len(), 4);
    assert_eq!(enqueues[3]["after"]["rerun_of"], a);
    c.db.cleanup().await;
}

#[tokio::test]
async fn quota_budget_and_validation_refusals() {
    let Some(c) = ctx_with(|cfg| {
        cfg.analysis.max_sessions = 25;
        cfg.analysis.max_frame_sessions = 8;
    })
    .await
    else {
        return;
    };
    cohort(&c, None).await;
    let (_, researcher) = c.token(Role::Researcher).await;
    let (exp_id, _) = experiment(&c, &researcher).await;

    // RUN-9 budgets, refused before queueing.
    let (s, b) = create_run(&c, body(&exp_id, "score", json!({})), &researcher).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{b}");
    assert_eq!(slug(&b), "cohort-too-large");
    let small = json!({"age_bands": ["12-17", "18-23", "24-35"]});
    let (s, b) = create_run(&c, body(&exp_id, "frames", json!({})), &researcher).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{b}");
    assert_eq!(slug(&b), "cohort-too-large", "10 frame sessions > 8");
    // SWP-2: 30 steps on one axis.
    let mut sweep = body(&exp_id, "score", small.clone());
    sweep["kind"] = json!("param_sweep");
    sweep["sweep"] =
        json!({"axes": [{"param_id": "flag.greenFloor", "from": 0.5, "to": 0.8, "step": 0.01}]});
    let (s, b) = create_run(&c, sweep.clone(), &researcher).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{b}");
    assert_eq!(slug(&b), "sweep-too-large");
    // WP-X.9: a sweep only with param_sweep, and param_sweep only with a sweep.
    sweep["sweep"]["axes"][0]["step"] = json!(0.05);
    sweep["kind"] = json!("cohort_rescore");
    let (s, b) = create_run(&c, sweep.clone(), &researcher).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{b}");
    assert_eq!(b["invalid_params"][0]["name"], "sweep", "{b}");
    sweep["kind"] = json!("param_sweep");
    sweep.as_object_mut().unwrap().remove("sweep");
    let (s, b) = create_run(&c, sweep, &researcher).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{b}");
    assert_eq!(b["invalid_params"][0]["name"], "sweep", "{b}");
    // Request shape.
    let mut both = body(&exp_id, "score", small.clone());
    both["cohort_id"] = json!(Uuid::new_v4());
    let (s, _) = create_run(&c, both, &researcher).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let (s, _) = create_run(
        &c,
        body(&Uuid::new_v4().to_string(), "score", small.clone()),
        &researcher,
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let (s, _) = create_run(
        &c,
        body(&exp_id, "score", json!({"age_bands": ["nope"]})),
        &researcher,
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let mut opts = body(&exp_id, "score", small.clone());
    opts["options"] = json!({"metrics": ["not_a_metric"]});
    let (s, _) = create_run(&c, opts, &researcher).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let (s, b) = create_run(
        &c,
        body(&exp_id, "score", json!({"to": "2001-01-01"})),
        &researcher,
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{b}");
    assert_eq!(slug(&b), "cohort-empty");
    assert_eq!(c.audit_rows("analysis.run_enqueue").await.len(), 0);

    // A saved cohort works like an inline one.
    let (s, cohort) = c
        .call(
            Method::POST,
            &format!("/v1/orgs/{}/cohorts", c.org),
            Some(json!({"name": "young", "definition": small})),
            &researcher,
        )
        .await;
    assert_eq!(s, StatusCode::CREATED, "{cohort}");
    // Quota: five queued, the sixth is 429.
    for i in 0..5 {
        let mut b = json!({"kind": "cohort_rescore", "experiment_id": exp_id, "level": "score"});
        b["cohort_id"] = cohort["id"].clone();
        let (s, run) = create_run(&c, b, &researcher).await;
        assert_eq!(s, StatusCode::ACCEPTED, "{i}: {run}");
        assert_eq!(run["cohort_id"], cohort["id"]);
    }
    let (s, b) = create_run(
        &c,
        body(&exp_id, "score", json!({"age_bands": ["12-17"]})),
        &researcher,
    )
    .await;
    assert_eq!(s, StatusCode::TOO_MANY_REQUESTS, "{b}");
    assert_eq!(slug(&b), "analysis-quota-exceeded");
    // Cancelling a queued run frees a slot at once.
    let (_, list) = c
        .call(
            Method::GET,
            &format!("/v1/orgs/{}/analysis-runs?limit=2", c.org),
            None,
            &researcher,
        )
        .await;
    assert_eq!(list["items"].as_array().unwrap().len(), 2);
    assert!(list["next_cursor"].as_str().is_some());
    let first = list["items"][0]["id"].as_str().unwrap().to_owned();
    let (s, run) = c
        .call(
            Method::POST,
            &format!("/v1/analysis-runs/{first}/cancel"),
            None,
            &researcher,
        )
        .await;
    assert_eq!(s, StatusCode::ACCEPTED, "{run}");
    assert_eq!(run["status"], "cancelled");
    let job_status: String = sqlx::query_scalar("SELECT status FROM jobs WHERE id = $1::uuid")
        .bind(run["job_id"].as_str().unwrap())
        .fetch_one(&c.db.pool)
        .await
        .unwrap();
    assert_eq!(job_status, "cancelled");
    let (s, b) = c
        .call(
            Method::POST,
            &format!("/v1/analysis-runs/{first}/cancel"),
            None,
            &researcher,
        )
        .await;
    assert_eq!(s, StatusCode::CONFLICT, "{b}");
    assert_eq!(slug(&b), "job-finished");
    assert_eq!(c.audit_rows("analysis.run_cancel").await.len(), 1);
    let (s, b) = create_run(
        &c,
        body(&exp_id, "score", json!({"age_bands": ["12-17"]})),
        &researcher,
    )
    .await;
    assert_eq!(s, StatusCode::ACCEPTED, "{b}");

    // RUN-9: one running analysis per org; the worker runs the rest one after another.
    assert_eq!(c.run_jobs().await, 5);
    let left: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM analysis_runs WHERE status <> 'succeeded' AND status <> 'cancelled'",
    )
    .fetch_one(&c.db.pool)
    .await
    .unwrap();
    assert_eq!(left, 0);
    c.db.cleanup().await;
}

#[tokio::test]
async fn the_claim_skips_an_org_whose_analysis_is_running() {
    let Some(c) = Ctx::new().await else { return };
    cohort(&c, None).await;
    let (_, researcher) = c.token(Role::Researcher).await;
    let (exp_id, _) = experiment(&c, &researcher).await;
    for _ in 0..2 {
        let (s, b) = create_run(
            &c,
            body(&exp_id, "score", json!({"age_bands": ["12-17"]})),
            &researcher,
        )
        .await;
        assert_eq!(s, StatusCode::ACCEPTED, "{b}");
    }
    let ctx = c.job_ctx();
    let first = runner::claim_next_in(&ctx, "w1").await.unwrap().unwrap();
    assert_eq!(first.kind, "analysis_run");
    assert!(
        runner::claim_next_in(&ctx, "w2").await.unwrap().is_none(),
        "the org's second run waits while the first runs"
    );
    runner::execute_in(&ctx, &first).await.unwrap();
    let second = runner::claim_next_in(&ctx, "w2").await.unwrap().unwrap();
    assert_eq!(second.kind, "analysis_run");
    c.db.cleanup().await;
}

#[tokio::test]
async fn cancellation_between_pages_leaves_no_blob() {
    let Some(c) = ctx_with(|cfg| cfg.analysis.page_size = 5).await else {
        return;
    };
    cohort(&c, None).await;
    let (_, researcher) = c.token(Role::Researcher).await;
    let (exp_id, _) = experiment(&c, &researcher).await;
    let (_, run) = create_run(&c, body(&exp_id, "auto", json!({})), &researcher).await;
    let id = run["id"].as_str().unwrap().to_owned();

    let ctx = c.job_ctx();
    let job = runner::claim_next_in(&ctx, "w").await.unwrap().unwrap();
    // Running: the request is recorded and the job stops at its next page.
    let (s, b) = c
        .call(
            Method::POST,
            &format!("/v1/analysis-runs/{id}/cancel"),
            None,
            &researcher,
        )
        .await;
    assert_eq!(s, StatusCode::ACCEPTED, "{b}");
    assert_eq!(b["status"], "queued", "the job has not started the run yet");
    let cancel = c.audit_rows("analysis.run_cancel").await;
    assert_eq!(cancel[0]["after"]["outcome"], "cancel_requested");
    assert_eq!(
        runner::execute_in(&ctx, &job).await.unwrap(),
        runner::Outcome::Cancelled
    );
    let run = get_run(&c, &id, &researcher).await;
    assert_eq!(run["status"], "cancelled", "{run}");
    assert_eq!(run["summary"], Value::Null);
    assert_eq!(run["result_bytes"], Value::Null);
    let (status, key): (String, Option<String>) = sqlx::query_as(
        "SELECT j.status, r.result_blob_key FROM analysis_runs r JOIN jobs j ON j.id = r.job_id
         WHERE r.id = $1::uuid",
    )
    .bind(&id)
    .fetch_one(&c.db.pool)
    .await
    .unwrap();
    assert_eq!((status.as_str(), key), ("cancelled", None));
    let blob = analysis::result_key(c.org, id.parse().unwrap());
    assert!(c.state.blobs.get(&blob).await.unwrap().is_none());
    let (s, _) = c
        .call(
            Method::GET,
            &format!("/v1/analysis-runs/{id}/sessions"),
            None,
            &researcher,
        )
        .await;
    assert_eq!(s, StatusCode::CONFLICT);
    c.db.cleanup().await;
}

#[tokio::test]
async fn the_deadline_fails_the_run_budget_exceeded() {
    let Some(c) = ctx_with(|cfg| cfg.analysis.max_duration = Duration::ZERO).await else {
        return;
    };
    cohort(&c, None).await;
    let (_, researcher) = c.token(Role::Researcher).await;
    let (exp_id, _) = experiment(&c, &researcher).await;
    let (_, run) = create_run(&c, body(&exp_id, "auto", json!({})), &researcher).await;
    let id = run["id"].as_str().unwrap().to_owned();
    c.run_jobs().await;
    let run = get_run(&c, &id, &researcher).await;
    assert_eq!(run["status"], "failed", "{run}");
    assert_eq!(run["error"], "budget_exceeded");
    let job: (String, i32) =
        sqlx::query_as("SELECT status, attempts FROM jobs WHERE id = $1::uuid")
            .bind(run["job_id"].as_str().unwrap())
            .fetch_one(&c.db.pool)
            .await
            .unwrap();
    assert_eq!(job, ("failed".to_owned(), 1), "permanent: no retry");
    assert_eq!(
        c.audit_rows("analysis.run_complete").await[0]["after"]["status"],
        "failed"
    );
    c.db.cleanup().await;
}

#[tokio::test]
async fn the_job_only_reads_sessions_and_config() {
    // GOV-5: the job runs under a role that may SELECT everything but write only
    // analysis_runs, jobs, audit_log and org_data_keys; it succeeds, and no session row moves.
    let Some(c) = Ctx::new().await else { return };
    cohort(&c, None).await;
    let (_, researcher) = c.token(Role::Researcher).await;
    let (exp_id, _) = experiment(&c, &researcher).await;
    let (_, run) = create_run(&c, body(&exp_id, "auto", json!({})), &researcher).await;
    let id = run["id"].as_str().unwrap().to_owned();
    let before = xmins(&c).await;

    let role = format!("analysis_ro_{}", Uuid::new_v4().simple());
    for stmt in [
        format!("CREATE ROLE {role} NOLOGIN"),
        format!("GRANT SELECT ON ALL TABLES IN SCHEMA public TO {role}"),
        format!("GRANT INSERT, UPDATE ON analysis_runs, jobs, audit_log, org_data_keys TO {role}"),
        format!("GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA public TO {role}"),
    ] {
        c.db.pool.execute(AssertSqlSafe(stmt)).await.unwrap();
    }
    let url = std::env::var("DATABASE_URL").unwrap();
    let opts: PgConnectOptions = url
        .parse::<PgConnectOptions>()
        .unwrap()
        .database(&c.db.name);
    let set_role = format!("SET ROLE {role}");
    let restricted = PgPoolOptions::new()
        .max_connections(3)
        .after_connect(move |conn, _| {
            let set_role = set_role.clone();
            Box::pin(async move {
                conn.execute(AssertSqlSafe(set_role)).await?;
                Ok(())
            })
        })
        .connect_with(opts)
        .await
        .unwrap();
    // The role really cannot write a session.
    let denied = sqlx::query("UPDATE sessions SET age_months = age_months")
        .execute(&restricted)
        .await;
    assert!(denied.is_err(), "the test role must not write sessions");

    let ctx = JobContext {
        pool: restricted.clone(),
        ..c.job_ctx()
    };
    let job = runner::claim_next_in(&ctx, "w").await.unwrap().unwrap();
    assert_eq!(
        runner::execute_in(&ctx, &job).await.unwrap(),
        runner::Outcome::Succeeded
    );
    let run = get_run(&c, &id, &researcher).await;
    assert_eq!(run["status"], "succeeded", "{run}");
    assert_eq!(before, xmins(&c).await);
    restricted.close().await;
    c.db.pool
        .execute(AssertSqlSafe(format!("DROP OWNED BY {role}")))
        .await
        .unwrap();
    c.db.pool
        .execute(AssertSqlSafe(format!("DROP ROLE {role}")))
        .await
        .unwrap();
    c.db.cleanup().await;
}

#[tokio::test]
async fn routes_follow_the_gov1_matrix() {
    let Some(c) = Ctx::new().await else { return };
    cohort(&c, None).await;
    let (_, researcher) = c.token(Role::Researcher).await;
    let (exp_id, _) = experiment(&c, &researcher).await;
    let small = json!({"age_bands": ["12-17"]});
    let (_, run) = create_run(&c, body(&exp_id, "score", small.clone()), &researcher).await;
    let id = run["id"].as_str().unwrap().to_owned();
    c.run_jobs().await;

    for role in ORG_ROLES {
        let (_, token) = c.token(role).await;
        // Create (AnalysisRun).
        let (s, b) = create_run(&c, body(&exp_id, "score", small.clone()), &token).await;
        let want = if rbac::allows(role, Action::AnalysisRun) {
            StatusCode::ACCEPTED
        } else {
            StatusCode::FORBIDDEN
        };
        assert_eq!(s, want, "{role:?} create {b}");
        // Read (AnalysisRead).
        let (s, _) = c
            .call(
                Method::GET,
                &format!("/v1/analysis-runs/{id}"),
                None,
                &token,
            )
            .await;
        let read = rbac::allows(role, Action::AnalysisRead);
        assert_eq!(
            s,
            if read {
                StatusCode::OK
            } else {
                StatusCode::FORBIDDEN
            },
            "{role:?}"
        );
        let (s, _) = c
            .call(
                Method::GET,
                &format!("/v1/orgs/{}/analysis-runs", c.org),
                None,
                &token,
            )
            .await;
        assert_eq!(
            s,
            if read {
                StatusCode::OK
            } else {
                StatusCode::FORBIDDEN
            },
            "{role:?}"
        );
        // Rows need SessionRead too.
        let (s, _) = c
            .call(
                Method::GET,
                &format!("/v1/analysis-runs/{id}/sessions"),
                None,
                &token,
            )
            .await;
        let rows = read && rbac::allows(role, Action::SessionRead);
        assert_eq!(
            s,
            if rows {
                StatusCode::OK
            } else {
                StatusCode::FORBIDDEN
            },
            "{role:?}"
        );
        // Re-run (AnalysisRun).
        let (s, _) = c
            .call(
                Method::POST,
                &format!("/v1/analysis-runs/{id}/rerun"),
                None,
                &token,
            )
            .await;
        assert_eq!(s, want, "{role:?} rerun");
        c.run_jobs().await;
    }

    // Another org: 404 everywhere; the vendor reads but never runs.
    let (_, stranger) = c.token_in(Some(c.other), Role::Researcher).await;
    for (m, uri) in [
        (Method::GET, format!("/v1/analysis-runs/{id}")),
        (Method::GET, format!("/v1/analysis-runs/{id}/sessions")),
        (Method::POST, format!("/v1/analysis-runs/{id}/rerun")),
        (Method::POST, format!("/v1/analysis-runs/{id}/cancel")),
        (Method::GET, format!("/v1/orgs/{}/analysis-runs", c.org)),
    ] {
        let (s, _) = c.call(m.clone(), &uri, None, &stranger).await;
        assert_eq!(s, StatusCode::NOT_FOUND, "{m} {uri}");
    }
    let (_, root) = c.token(Role::Superadmin).await;
    let (s, _) = create_run(&c, body(&exp_id, "score", small.clone()), &root).await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, _) = c
        .call(
            Method::POST,
            &format!("/v1/analysis-runs/{id}/rerun"),
            None,
            &root,
        )
        .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, _) = c
        .call(Method::GET, &format!("/v1/analysis-runs/{id}"), None, &root)
        .await;
    assert_eq!(s, StatusCode::OK);
    // No bearer: 401.
    let (s, _, _) = common::send(
        c.app.clone(),
        common::get(&format!("/v1/analysis-runs/{id}")),
    )
    .await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    c.db.cleanup().await;
}

fn slug(body: &Value) -> &str {
    common::auth::slug(body)
}
