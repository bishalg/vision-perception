//! owner: WP-B.5
//!
//! Norm aggregation job and promote, over HTTP with real auth (device tokens ingest the
//! sessions, console users' access tokens call the endpoints) and the real runner.
//!
//! Sessions come from `asd_core::synth` (CORE-9) posted to `POST /v1/sessions`. The
//! expected candidate set is computed in the test by calling `asd_core::norms` directly on
//! the same payloads, so the test proves the backend stores exactly what the core computes
//! for the sessions it deems eligible. Tests need `DATABASE_URL` (see `common`).

mod common;
mod ingest_support;

use std::sync::Arc;
use std::time::Duration;

use asd_backend::app::AppState;
use asd_backend::blob::MemoryBlobStore;
use asd_backend::jobs::norm_aggregate::norm_record;
use asd_backend::jobs::runner;
use asd_backend::jobs::{self, KIND_NORM_AGGREGATE};
use asd_core::norms::{self, AggregateParams};
use asd_core::synth::{generate_session, Profile};
use axum::Router;
use common::auth::{audit_count, empty};
use common::{send, TestDb};
use http::{Method, StatusCode};
use ingest_support::*;
use serde_json::{json, Value};
use sqlx::{PgPool, Row};
use uuid::Uuid;

fn state_for(pool: &PgPool, allow_synthetic: bool) -> AppState {
    state_with(
        pool.clone(),
        Arc::new(MemoryBlobStore::new()),
        config(allow_synthetic),
    )
}

/// A fresh router (fresh rate limiters) over a synthetic-friendly server.
fn api(pool: &PgPool) -> Router {
    app(state_for(pool, true))
}

/// Posts every payload as `device`, one router per 40 posts (the per-device rate limit).
async fn ingest_all(pool: &PgPool, device: &str, payloads: &[Value]) {
    for chunk in payloads.chunks(40) {
        let router = api(pool);
        for p in chunk {
            ingest_ok(&router, device, p).await;
        }
    }
}

fn typical(range: std::ops::Range<u64>) -> Vec<Value> {
    range
        .map(|i| generate_session(7, i, Some(Profile::Typical)))
        .collect()
}

async fn call(pool: &PgPool, req: http::Request<axum::body::Body>) -> (StatusCode, Value) {
    let (status, _, body) = send(api(pool), req).await;
    (status, body)
}

fn run_params() -> Value {
    json!({"source_filter": {"include_synthetic": true}})
}

/// Queues a run as `user`, drains the queue with the real runner, returns the norm run.
async fn run_to_completion(pool: &PgPool, user: &str, params: &Value) -> Value {
    let (status, job) = call(
        pool,
        user_json(user, "POST", "/v1/jobs/norm-aggregate", params),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{job}");
    assert_eq!(job["status"], "queued");
    assert_eq!(runner::run_pending(pool, "test-worker").await.unwrap(), 1);
    let (status, job) = call(
        pool,
        user_get(user, &format!("/v1/jobs/{}", job["id"].as_str().unwrap())),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(job["status"], "succeeded", "{job}");
    let run_id = job["result"]["norm_run_id"].as_str().expect("norm_run_id");
    let (status, run) = call(pool, user_get(user, &format!("/v1/norm-runs/{run_id}"))).await;
    assert_eq!(status, StatusCode::OK, "{run}");
    run
}

async fn open_review_sessions(pool: &PgPool) -> Vec<String> {
    sqlx::query_scalar("SELECT session_id FROM review_items WHERE status IN ('open','in_review')")
        .fetch_all(pool)
        .await
        .unwrap()
}

/// The candidate `asd_core::norms` computes for `payloads` (the sessions the test expects
/// to be eligible), and how many sessions fed it.
fn expected(payloads: &[&Value]) -> (Value, usize) {
    let records: Vec<Value> = payloads
        .iter()
        .filter_map(|p| {
            norm_record(
                p["rawPayload"].get("perVideo").cloned(),
                p["rawPayload"].get("ageMonths").cloned(),
            )
        })
        .collect();
    let candidate = norms::aggregate(&records, &AggregateParams::default());
    (serde_json::to_value(candidate).unwrap(), records.len())
}

fn eligible_by_quality(p: &Value) -> bool {
    matches!(p["qualityVerdict"].as_str(), Some("optimal" | "acceptable"))
        && p["validRatio"].as_f64().unwrap() >= 0.80
}

// ---- the full flow ---------------------------------------------------------------------

#[tokio::test]
async fn run_aggregates_eligible_sessions_with_asd_core_then_promotes_a_draft() {
    let Some(db) = TestDb::migrated().await else {
        return;
    };
    let pool = &db.pool;
    let org = seed_quiet_org(pool).await;
    let (_, device) = seed_device(pool, org).await;
    let (_, admin) = seed_user(pool, Some(org), "admin").await;

    // 250 typical sessions plus 10 low-quality ones (rejected verdict, never eligible).
    let good = typical(0..250);
    let low: Vec<Value> = (1000..1010)
        .map(|i| generate_session(7, i, Some(Profile::LowQuality)))
        .collect();
    ingest_all(pool, &device, &good).await;
    ingest_all(pool, &device, &low).await;

    // Trigger.
    let (status, job) = call(
        pool,
        user_json(&admin, "POST", "/v1/jobs/norm-aggregate", &run_params()),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{job}");
    assert_eq!(job["status"], "queued");
    assert_eq!(job["kind"], "norm_aggregate");
    assert_eq!(job["org_id"], org.to_string());
    assert_eq!(job["params"]["min_n"], 12);
    assert_eq!(job["params"]["include_imputed"], false);
    assert_eq!(audit_count(pool, "job.norm_aggregate.enqueue").await, 1);
    let job_id = job["id"].as_str().unwrap().to_owned();

    // Nothing ran yet: the API never executes a job.
    let (_, queued) = call(pool, user_get(&admin, &format!("/v1/jobs/{job_id}"))).await;
    assert_eq!(queued["status"], "queued");
    let (_, runs) = call(pool, user_get(&admin, "/v1/norm-runs")).await;
    assert_eq!(runs["items"], json!([]));

    // Run.
    assert_eq!(runner::run_pending(pool, "w1").await.unwrap(), 1);
    let (_, done) = call(pool, user_get(&admin, &format!("/v1/jobs/{job_id}"))).await;
    assert_eq!(done["status"], "succeeded", "{done}");
    assert_eq!(done["attempts"], 1);
    let run_id = done["result"]["norm_run_id"].as_str().unwrap().to_owned();
    assert_eq!(audit_count(pool, "norm_run.create").await, 1);

    // Expected: only quality-valid sessions without a pending review item.
    let pending = open_review_sessions(pool).await;
    let eligible: Vec<&Value> = good
        .iter()
        .filter(|p| eligible_by_quality(p))
        .filter(|p| !pending.contains(&p["sessionId"].as_str().unwrap().to_owned()))
        .collect();
    let (want_cells, want_n) = expected(&eligible);
    assert!(
        want_n > 100,
        "the corpus should be mostly eligible: {want_n}"
    );

    let (status, run) = call(pool, user_get(&admin, &format!("/v1/norm-runs/{run_id}"))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        run["cells"], want_cells,
        "cells are exactly asd-core's output"
    );
    assert_eq!(run["n_total"], want_n);
    assert_eq!(run["job_id"], job_id);
    assert_eq!(run["org_id"], org.to_string());
    assert_eq!(run["promoted_to_config_id"], Value::Null);
    assert_eq!(run["cells"]["status"], "candidate-collected");
    let qualifying = run["qualifying_cells"].as_u64().unwrap();
    let total = run["total_cells"].as_u64().unwrap();
    assert!(qualifying > 0, "some cells reach n >= 12");
    assert!(total >= qualifying);
    assert_eq!(done["result"]["qualifying_cells"], qualifying);
    assert_eq!(done["result"]["sessions_used"], want_n);

    // Imputed rows never reach a cell: the low-quality profile imputes, and asd-core drops
    // those rows; the proof is the equality above plus no `n` exceeding the clean count.
    for video in run["cells"]["videos"].as_object().unwrap().values() {
        for exp in video["segments"][0]["expectations"].as_array().unwrap() {
            for cell in exp["byAge"].as_object().unwrap().values() {
                if let Some(n) = cell.get("n").and_then(Value::as_u64) {
                    assert!(n >= 12, "a published cell has n >= 12");
                }
            }
        }
    }

    // List shows it, newest first, without the cells.
    let (_, page) = call(pool, user_get(&admin, "/v1/norm-runs")).await;
    assert_eq!(page["items"].as_array().unwrap().len(), 1);
    assert_eq!(page["items"][0]["id"], run_id);
    assert!(page["items"][0].get("cells").is_none());
    assert_eq!(page["next_cursor"], Value::Null);

    // A session under review drops out of the next run ("only reviewed sessions").
    let victim = eligible[0]["sessionId"].as_str().unwrap().to_owned();
    let (status, item) = call(
        pool,
        user_json(
            &admin,
            "POST",
            &format!("/v1/orgs/{org}/review"),
            &json!({"session_id": victim}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{item}");
    let second = run_to_completion(pool, &admin, &run_params()).await;
    assert_eq!(second["n_total"], want_n - 1);
    // ... and an "exclude from norms" label removes another.
    let labelled = eligible[1]["sessionId"].as_str().unwrap().to_owned();
    sqlx::query(
        "INSERT INTO labels (session_id, key, value) VALUES ($1, 'exclude_from_norms', 'true')",
    )
    .bind(&labelled)
    .execute(pool)
    .await
    .unwrap();
    let third = run_to_completion(pool, &admin, &run_params()).await;
    assert_eq!(third["n_total"], want_n - 2);

    // ---- promote
    let (status, list) = call(pool, user_get(&admin, "/v1/norm-runs?limit=2")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list["items"].as_array().unwrap().len(), 2);
    assert!(list["next_cursor"].is_string());
    let (_, rest) = call(
        pool,
        user_get(
            &admin,
            &format!(
                "/v1/norm-runs?limit=2&cursor={}",
                list["next_cursor"].as_str().unwrap()
            ),
        ),
    )
    .await;
    assert_eq!(rest["items"].as_array().unwrap().len(), 1);
    assert_eq!(rest["items"][0]["id"], run_id, "oldest run last");

    let (status, promoted) = call(
        pool,
        empty(
            Method::POST,
            &format!("/v1/norm-runs/{run_id}/promote"),
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{promoted}");
    assert_eq!(promoted["status"], "draft");
    assert_eq!(promoted["kind"], "normative_baselines");
    assert_eq!(promoted["norm_run_id"], run_id);
    let doc_id: Uuid = promoted["config_document_id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let doc = sqlx::query(
        "SELECT kind, status, body, published_at, signature, created_by
         FROM config_documents WHERE id = $1",
    )
    .bind(doc_id)
    .fetch_one(pool)
    .await
    .unwrap();
    let kind: String = doc.get("kind");
    let status: String = doc.get("status");
    let body: Value = doc.get("body");
    let published_at: Option<chrono::DateTime<chrono::Utc>> = doc.get("published_at");
    let signature: Option<String> = doc.get("signature");
    let created_by: Option<Uuid> = doc.get("created_by");
    assert_eq!(kind, "normative_baselines");
    assert_eq!(status, "draft", "promote never publishes");
    assert_eq!(body, want_cells);
    assert!(published_at.is_none() && signature.is_none());
    assert!(created_by.is_some());
    let bundles: i64 = sqlx::query_scalar("SELECT count(*) FROM config_bundles")
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(bundles, 0);
    let (_, run_after) = call(pool, user_get(&admin, &format!("/v1/norm-runs/{run_id}"))).await;
    assert_eq!(run_after["promoted_to_config_id"], doc_id.to_string());
    assert_eq!(audit_count(pool, "norm_run.promote").await, 1);

    // Promoting twice is a conflict and creates nothing.
    let (status, again) = call(
        pool,
        empty(
            Method::POST,
            &format!("/v1/norm-runs/{run_id}/promote"),
            Some(&admin),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(problem_slug(&again), "norm-run-promoted");
    let docs: i64 = sqlx::query_scalar("SELECT count(*) FROM config_documents")
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(docs, 1);
    assert_eq!(audit_count(pool, "norm_run.promote").await, 1);

    db.cleanup().await;
}

#[tokio::test]
async fn synthetic_data_is_excluded_unless_asked_and_an_empty_run_cannot_be_promoted() {
    let Some(db) = TestDb::migrated().await else {
        return;
    };
    let pool = &db.pool;
    let org = seed_quiet_org(pool).await;
    let (_, device) = seed_device(pool, org).await;
    let (_, admin) = seed_user(pool, Some(org), "admin").await;
    ingest_all(pool, &device, &typical(0..30)).await;

    let run = run_to_completion(pool, &admin, &json!({})).await;
    assert_eq!(run["n_total"], 0, "synthetic rows are excluded by default");
    assert_eq!(run["qualifying_cells"], 0);
    assert_eq!(run["cells"]["videos"], json!({}));

    let with = run_to_completion(pool, &admin, &run_params()).await;
    assert!(with["n_total"].as_u64().unwrap() > 0);
    assert_eq!(
        with["qualifying_cells"], 0,
        "30 sessions cannot fill a 12-row cell"
    );
    assert!(
        with["total_cells"].as_u64().unwrap() > 0,
        "every cell is withheld, none dropped"
    );
    let any_cell = with["cells"]["videos"]
        .as_object()
        .unwrap()
        .values()
        .all(|v| {
            v["segments"][0]["expectations"]
                .as_array()
                .unwrap()
                .iter()
                .all(|e| e["byAge"].as_object().unwrap().values().all(Value::is_null))
        });
    assert!(any_cell, "cells below n = 12 are withheld (null)");

    // Nothing to promote: every cell is withheld.
    for r in [&run, &with] {
        let (status, body) = call(
            pool,
            empty(
                Method::POST,
                &format!("/v1/norm-runs/{}/promote", r["id"].as_str().unwrap()),
                Some(&admin),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT, "{body}");
        assert_eq!(problem_slug(&body), "no-qualifying-cells");
    }
    let docs: i64 = sqlx::query_scalar("SELECT count(*) FROM config_documents")
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(docs, 0);

    // A server that does not accept synthetic data refuses to aggregate it.
    let (status, body) = send(
        app(state_for(pool, false)),
        user_json(&admin, "POST", "/v1/jobs/norm-aggregate", &run_params()),
    )
    .await
    .pipe_status_body();
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(
        body["invalid_params"][0]["name"],
        "source_filter.include_synthetic"
    );

    db.cleanup().await;
}

trait PipeStatusBody {
    fn pipe_status_body(self) -> (StatusCode, Value);
}
impl PipeStatusBody for (StatusCode, http::HeaderMap, Value) {
    fn pipe_status_body(self) -> (StatusCode, Value) {
        (self.0, self.2)
    }
}

// ---- eligibility of device sessions ----------------------------------------------------

/// A synthetic payload turned into a device capture, optionally with norm consent.
fn device_session(index: u64, consented: bool) -> Value {
    let mut p = (0..)
        .map(|i| generate_session(7, index * 50 + i, Some(Profile::Typical)))
        .find(|p| p["qualityVerdict"] == "optimal")
        .unwrap();
    p["captureSource"] = json!("truedepth");
    p["rawPayload"]["captureSource"] = json!("truedepth");
    let raw = p["rawPayload"].as_object_mut().unwrap();
    for key in [
        "synthetic",
        "generator",
        "generatorVersion",
        "seed",
        "index",
        "profile",
    ] {
        raw.remove(key);
    }
    if consented {
        raw.insert("normCollection".into(), json!({"consented": true}));
    }
    p
}

async fn norm_item(pool: &PgPool, session: &str) -> Uuid {
    sqlx::query_scalar(
        "SELECT id FROM review_items WHERE session_id = $1 AND reason = 'norm_candidate'",
    )
    .bind(session)
    .fetch_one(pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn device_sessions_need_consent_and_a_closed_norm_review() {
    let Some(db) = TestDb::migrated().await else {
        return;
    };
    let pool = &db.pool;
    let org = seed_quiet_org(pool).await;
    let (_, device) = seed_device(pool, org).await;
    let (_, admin) = seed_user(pool, Some(org), "admin").await;
    let (_, reviewer) = seed_user(pool, Some(org), "reviewer").await;

    let ok = device_session(1, true); // consented, review closed: eligible
    let pending = device_session(2, true); // consented, review still open
    let no_consent = device_session(3, false);
    let excluded = device_session(4, true); // closed with exclude_from_norms
    let withdrawn = device_session(5, true); // consent withdrawn afterwards
    let dismissed = device_session(6, true); // reviewer dismissed it
    let all = [
        &ok,
        &pending,
        &no_consent,
        &excluded,
        &withdrawn,
        &dismissed,
    ];
    for p in all {
        // Distinct sessions and children (synth ids are unique per index already).
        assert!(p["participantToken"].is_string());
    }
    ingest_all(
        pool,
        &device,
        &all.iter().map(|p| (*p).clone()).collect::<Vec<_>>(),
    )
    .await;

    let sid = |p: &Value| p["sessionId"].as_str().unwrap().to_owned();
    let patch =
        |id: Uuid, body: Value| user_json(&reviewer, "PATCH", &format!("/v1/review/{id}"), &body);
    for (p, body) in [
        (&ok, json!({"status": "done"})),
        (
            &excluded,
            json!({"status": "done", "decision": {"exclude_from_norms": true}}),
        ),
        (&withdrawn, json!({"status": "done"})),
        (&dismissed, json!({"status": "dismissed"})),
    ] {
        let id = norm_item(pool, &sid(p)).await;
        let (status, resp) = call(pool, patch(id, body)).await;
        assert_eq!(status, StatusCode::OK, "{resp}");
    }
    assert!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM review_items WHERE session_id = $1")
            .bind(sid(&no_consent))
            .fetch_one(pool)
            .await
            .unwrap()
            == 0,
        "no consent means no norm_candidate item"
    );
    sqlx::query("UPDATE participants SET withdrawn_at = now() WHERE token = $1")
        .bind(withdrawn["participantToken"].as_str().unwrap())
        .execute(pool)
        .await
        .unwrap();

    // Synthetic is not requested here: only device sessions count.
    let run = run_to_completion(pool, &admin, &json!({})).await;
    assert_eq!(
        run["n_total"], 1,
        "only the consented, reviewed, non-withdrawn session"
    );

    db.cleanup().await;
}

// ---- RBAC, scoping, audit, validation --------------------------------------------------

#[tokio::test]
async fn rbac_org_scoping_validation_and_audit() {
    let Some(db) = TestDb::migrated().await else {
        return;
    };
    let pool = &db.pool;
    let org_a = seed_quiet_org(pool).await;
    let org_b = seed_quiet_org(pool).await;
    let mut tokens = std::collections::HashMap::new();
    for role in [
        "owner",
        "admin",
        "clinician",
        "researcher",
        "reviewer",
        "viewer",
    ] {
        tokens.insert(role, seed_user(pool, Some(org_a), role).await.1);
    }
    let (_, admin_b) = seed_user(pool, Some(org_b), "admin").await;
    let (_, root) = seed_user(pool, None, "superadmin").await;

    // Unauthenticated: 401 problem on every endpoint.
    let id = Uuid::new_v4();
    for (method, uri) in [
        (Method::POST, "/v1/jobs/norm-aggregate".to_owned()),
        (Method::GET, format!("/v1/jobs/{id}")),
        (Method::GET, "/v1/norm-runs".to_owned()),
        (Method::GET, format!("/v1/norm-runs/{id}")),
        (Method::POST, format!("/v1/norm-runs/{id}/promote")),
    ] {
        let (status, _, body) = send(api(pool), empty(method, &uri, None)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{uri}");
        assert_eq!(problem_slug(&body), "unauthorized");
    }

    // Role matrix: JobRun = owner/admin/researcher; the rest are 403 and leave no trace.
    for role in ["clinician", "reviewer", "viewer"] {
        let (status, _) = call(
            pool,
            user_json(&tokens[role], "POST", "/v1/jobs/norm-aggregate", &json!({})),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{role}");
        let (status, _) = call(pool, user_get(&tokens[role], "/v1/norm-runs")).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{role}");
    }
    let jobs: i64 = sqlx::query_scalar("SELECT count(*) FROM jobs")
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(jobs, 0);
    let job_audits: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit_log WHERE action LIKE 'job.%' OR action LIKE 'norm_run.%'",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(job_audits, 0, "denied calls are not audited");
    let mut job_ids = Vec::new();
    for role in ["owner", "admin", "researcher"] {
        let (status, job) = call(
            pool,
            user_json(&tokens[role], "POST", "/v1/jobs/norm-aggregate", &json!({})),
        )
        .await;
        assert_eq!(status, StatusCode::ACCEPTED, "{role}: {job}");
        job_ids.push(job["id"].as_str().unwrap().to_owned());
    }
    assert_eq!(audit_count(pool, "job.norm_aggregate.enqueue").await, 3);

    // Validation.
    for (body, field) in [
        (json!({"min_n": 11}), "min_n"),
        (json!({"min_n": 0}), "min_n"),
        (
            json!({"source_filter": {"from": "2026-02-01T00:00:00Z", "to": "2026-01-01T00:00:00Z"}}),
            "source_filter.from",
        ),
    ] {
        let (status, resp) = call(
            pool,
            user_json(&tokens["admin"], "POST", "/v1/jobs/norm-aggregate", &body),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        assert_eq!(problem_slug(&resp), "validation-failed");
        assert_eq!(resp["invalid_params"][0]["name"], field);
    }
    let (status, resp) = call(
        pool,
        user_json(
            &tokens["admin"],
            "POST",
            "/v1/jobs/norm-aggregate",
            &json!({"unknown": 1}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{resp}");
    assert_eq!(audit_count(pool, "job.norm_aggregate.enqueue").await, 3);

    // Scoping: org B never sees org A's jobs; naming another org is a 404.
    for job in &job_ids {
        let (status, _) = call(pool, user_get(&admin_b, &format!("/v1/jobs/{job}"))).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        let (status, _) = call(pool, user_get(&root, &format!("/v1/jobs/{job}"))).await;
        assert_eq!(status, StatusCode::OK);
    }
    let (status, _) = call(
        pool,
        user_json(
            &admin_b,
            "POST",
            "/v1/jobs/norm-aggregate",
            &json!({"source_filter": {"org_id": org_a}}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = call(pool, user_get(&admin_b, "/v1/jobs/not-a-uuid")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // Drain: three runs for org A. Org B sees none; org A sees three; superadmin sees them
    // and may also run across every org (org_id null), which org users cannot read.
    assert_eq!(runner::run_pending(pool, "w").await.unwrap(), 3);
    let (_, a) = call(pool, user_get(&tokens["researcher"], "/v1/norm-runs")).await;
    assert_eq!(a["items"].as_array().unwrap().len(), 3);
    let (_, b) = call(pool, user_get(&admin_b, "/v1/norm-runs")).await;
    assert_eq!(b["items"], json!([]));
    let run_a = a["items"][0]["id"].as_str().unwrap().to_owned();
    let (status, _) = call(pool, user_get(&admin_b, &format!("/v1/norm-runs/{run_a}"))).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = call(
        pool,
        empty(
            Method::POST,
            &format!("/v1/norm-runs/{run_a}/promote"),
            Some(&admin_b),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let global = run_to_completion(pool, &root, &json!({})).await;
    assert_eq!(global["org_id"], Value::Null);
    let (status, _) = call(
        pool,
        user_get(
            &tokens["admin"],
            &format!("/v1/norm-runs/{}", global["id"].as_str().unwrap()),
        ),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "all-orgs runs are superadmin only"
    );
    let (_, all) = call(pool, user_get(&root, "/v1/norm-runs")).await;
    assert_eq!(all["items"].as_array().unwrap().len(), 4);
    let (_, a2) = call(pool, user_get(&tokens["admin"], "/v1/norm-runs")).await;
    assert_eq!(a2["items"].as_array().unwrap().len(), 3);

    // Promote needs ConfigWrite (a viewer cannot), checked before the run is even read.
    let (status, _) = call(
        pool,
        empty(
            Method::POST,
            &format!("/v1/norm-runs/{run_a}/promote"),
            Some(&tokens["viewer"]),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(audit_count(pool, "norm_run.promote").await, 0);

    db.cleanup().await;
}

// ---- the runner ------------------------------------------------------------------------

#[tokio::test]
async fn concurrent_workers_never_claim_the_same_job() {
    let Some(db) = TestDb::migrated().await else {
        return;
    };
    let pool = db.pool.clone();
    {
        let mut conn = pool.acquire().await.unwrap();
        for _ in 0..40 {
            jobs::enqueue(&mut conn, KIND_NORM_AGGREGATE, None, &json!({}), None)
                .await
                .unwrap();
        }
    }
    let mut tasks = Vec::new();
    for w in 0..8 {
        let pool = pool.clone();
        tasks.push(tokio::spawn(async move {
            let mut mine = Vec::new();
            while let Some(job) = runner::claim_next(&pool, &format!("w{w}")).await.unwrap() {
                assert_eq!(job.status, "running");
                assert_eq!(job.attempts, 1);
                mine.push(job.id);
            }
            mine
        }));
    }
    let mut claimed = Vec::new();
    for t in tasks {
        claimed.extend(t.await.unwrap());
    }
    assert_eq!(claimed.len(), 40, "every job claimed");
    claimed.sort();
    claimed.dedup();
    assert_eq!(claimed.len(), 40, "no job claimed twice");
    assert!(runner::claim_next(&pool, "w").await.unwrap().is_none());
    db.cleanup().await;
}

#[tokio::test]
async fn failures_unknown_kinds_and_orphaned_jobs() {
    let Some(db) = TestDb::migrated().await else {
        return;
    };
    let pool = &db.pool;
    let mut conn = pool.acquire().await.unwrap();
    let unknown = jobs::enqueue(&mut conn, "mystery", None, &json!({}), None)
        .await
        .unwrap();
    let bad = jobs::enqueue(
        &mut conn,
        KIND_NORM_AGGREGATE,
        None,
        &json!({"min_n": 3}),
        None,
    )
    .await
    .unwrap();
    drop(conn);

    // Permanent errors fail at once, with a reason, and write no norm run.
    assert_eq!(runner::run_pending(pool, "w").await.unwrap(), 2);
    for id in [unknown.id, bad.id] {
        let (status, error, attempts): (String, Option<String>, i32) =
            sqlx::query_as("SELECT status, error, attempts FROM jobs WHERE id = $1")
                .bind(id)
                .fetch_one(pool)
                .await
                .unwrap();
        assert_eq!(status, "failed");
        assert_eq!(attempts, 1);
        assert!(error.unwrap().contains("unknown job kind") || id == bad.id);
    }
    let runs: i64 = sqlx::query_scalar("SELECT count(*) FROM norm_runs")
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(runs, 0);

    // A worker that died mid-job: the row stays `running` until the visibility timeout.
    let mut conn = pool.acquire().await.unwrap();
    let orphan = jobs::enqueue(&mut conn, KIND_NORM_AGGREGATE, None, &json!({}), None)
        .await
        .unwrap();
    drop(conn);
    let claimed = runner::claim_next(pool, "dead-worker")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(claimed.id, orphan.id);
    assert_eq!(
        runner::reclaim_stale(pool, Duration::from_secs(60))
            .await
            .unwrap(),
        0,
        "still fresh"
    );
    sqlx::query("UPDATE jobs SET started_at = now() - interval '1 hour' WHERE id = $1")
        .bind(orphan.id)
        .execute(pool)
        .await
        .unwrap();
    assert_eq!(
        runner::reclaim_stale(pool, Duration::from_secs(60))
            .await
            .unwrap(),
        1
    );
    assert_eq!(runner::run_pending(pool, "w2").await.unwrap(), 1);
    let (status, attempts): (String, i32) =
        sqlx::query_as("SELECT status, attempts FROM jobs WHERE id = $1")
            .bind(orphan.id)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!((status.as_str(), attempts), ("succeeded", 2));

    // At the attempt limit an orphan is failed, not requeued forever.
    sqlx::query("UPDATE jobs SET status = 'running', attempts = $2, started_at = now() - interval '1 hour' WHERE id = $1")
        .bind(orphan.id)
        .bind(runner::MAX_ATTEMPTS)
        .execute(pool)
        .await
        .unwrap();
    assert_eq!(
        runner::reclaim_stale(pool, Duration::from_secs(60))
            .await
            .unwrap(),
        1
    );
    let status: String = sqlx::query_scalar("SELECT status FROM jobs WHERE id = $1")
        .bind(orphan.id)
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(status, "failed");
    db.cleanup().await;
}

#[tokio::test]
async fn worker_loop_runs_queued_jobs_and_stops_on_shutdown() {
    let Some(db) = TestDb::migrated().await else {
        return;
    };
    let pool = db.pool.clone();
    let org = seed_quiet_org(&pool).await;
    let (_, admin) = seed_user(&pool, Some(org), "admin").await;
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let worker = tokio::spawn(runner::worker_loop(
        pool.clone(),
        "loop-worker".into(),
        Duration::from_millis(50),
        runner::DEFAULT_VISIBILITY_TIMEOUT,
        async move {
            let _ = stopped.await;
        },
    ));

    let (status, job) = call(
        &pool,
        user_json(&admin, "POST", "/v1/jobs/norm-aggregate", &json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED);
    let uri = format!("/v1/jobs/{}", job["id"].as_str().unwrap());
    let mut state = String::new();
    for _ in 0..100 {
        let (_, j) = call(&pool, user_get(&admin, &uri)).await;
        state = j["status"].as_str().unwrap().to_owned();
        if state == "succeeded" {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(state, "succeeded");
    stop.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(5), worker)
        .await
        .expect("worker stops on shutdown")
        .unwrap();
    db.cleanup().await;
}
