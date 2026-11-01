//! owner: WP-B.9
//!
//! Dataset export (`POST /v1/jobs/export-dataset`, JSON Lines and Parquet, labels, consent
//! filter, de-identification, signed download), erasure execution (full erasure with the
//! tombstone, EXJ-8 artifact deletion, the clinical-record hold, idempotent re-runs, the
//! device and admin routes, scheduling of left-over requests) and the retention sweep (dry
//! run, purge, audit, nightly scheduling). Needs `DATABASE_URL`.

mod common;
mod export_support;
mod participant_support;

use asd_backend::jobs::{erasure, retention};
use asd_contract::api_v1::Role;
use bytes::Bytes;
use export_support::body;
use http::{Method, StatusCode};
use participant_support::*;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::common::auth::seed_device;

const REASON: &str = "Model training extract for the approved study";

/// P-A: S-1 research-consented (two labels), S-2 no consent; P-W (withdrawn): S-3 consented;
/// P-A also has a synthetic consented session S-SYN.
async fn cohort(c: &Ctx) -> (Uuid, Uuid) {
    let a = c.participant(c.org, "TOKEN-ALPHA").await;
    let w = c.participant(c.org, "TOKEN-WITHDRAWN").await;
    c.exportable_session(
        a,
        "TOKEN-ALPHA",
        "SESS-ONE",
        "2026-10-01T10:00:00Z",
        Some(true),
    )
    .await;
    c.exportable_session(a, "TOKEN-ALPHA", "SESS-TWO", "2026-10-03T10:00:00Z", None)
        .await;
    c.exportable_session(
        a,
        "TOKEN-ALPHA",
        "SESS-SYN",
        "2026-10-04T10:00:00Z",
        Some(true),
    )
    .await;
    sqlx::query("UPDATE sessions SET capture_source = 'synthetic' WHERE id = 'SESS-SYN'")
        .execute(&c.db.pool)
        .await
        .unwrap();
    c.exportable_session(
        w,
        "TOKEN-WITHDRAWN",
        "SESS-THREE",
        "2026-10-02T10:00:00Z",
        Some(true),
    )
    .await;
    c.withdraw(w).await;
    for (clip, key, value) in [
        (None, "gold_standard", json!("typical")),
        (Some("clip-1"), "usable", json!(true)),
    ] {
        sqlx::query(
            "INSERT INTO labels (session_id, clip_id, key, value) VALUES ('SESS-ONE', $1, $2, $3)",
        )
        .bind(clip)
        .bind(key)
        .bind(value)
        .execute(&c.db.pool)
        .await
        .unwrap();
    }
    (a, w)
}

async fn dataset(c: &Ctx, bearer: &str, body: Value) -> Value {
    let (s, made) = c
        .call(Method::POST, "/v1/jobs/export-dataset", Some(body), bearer)
        .await;
    assert_eq!(s, StatusCode::ACCEPTED, "{made}");
    assert_eq!(made["mode"], "deidentified");
    assert_eq!(made["purpose"], "research");
    c.run_jobs().await;
    let (s, done) = c
        .call(
            Method::GET,
            &format!("/v1/exports/{}", made["id"].as_str().unwrap()),
            None,
            bearer,
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{done}");
    done
}

#[tokio::test]
async fn jsonl_dataset_keeps_consented_live_sessions_with_labels_and_no_identifiers() {
    let Some(c) = Ctx::new().await else { return };
    let (a, _) = cohort(&c).await;
    let (_, researcher) = c.token(Role::Researcher).await;

    let done = dataset(
        &c,
        &researcher,
        json!({"format": "jsonl", "reason": REASON}),
    )
    .await;
    assert_eq!(done["status"], "succeeded", "{done}");
    assert_eq!(done["format"], "dataset_jsonl");
    assert_eq!(done["include_labels"], true);
    assert_eq!(done["content_type"], "application/jsonl");
    assert_eq!(done["row_count"], 1);
    assert_eq!(
        done["excluded"],
        json!({"capture_source_refused": 1, "no_research_consent": 1, "withdrawn": 1})
    );
    let id = done["id"].as_str().unwrap();
    assert_eq!(c.export_items(id).await, ["SESS-ONE"]);

    let text = String::from_utf8(c.artifact(id).await).unwrap();
    let lines: Vec<Value> = text
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0]["record_type"], "manifest");
    assert_eq!(lines[0]["notice"], "This is not a diagnostic tool.");
    assert!(lines[0]["watermark"].as_str().unwrap().contains(id));
    let row = &lines[1];
    assert_eq!(row["session_month"], "2026-10");
    assert_eq!(row["labels"].as_array().unwrap().len(), 2);
    assert_eq!(row["labels"][0]["key"], "gold_standard");
    for secret in ["TOKEN-ALPHA", "SESS-ONE", &a.to_string(), "2026-10-01T"] {
        assert!(!text.contains(secret), "dataset contains {secret}");
    }

    // The existing signed-download flow serves it.
    let (s, url) = c
        .call(
            Method::POST,
            &format!("/v1/exports/{id}/download-url"),
            None,
            &researcher,
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{url}");
    assert!(url["url"].as_str().unwrap().contains(id));

    // Without labels.
    let plain = dataset(
        &c,
        &researcher,
        json!({"format": "jsonl", "include_labels": false, "reason": REASON}),
    )
    .await;
    let text = String::from_utf8(c.artifact(plain["id"].as_str().unwrap()).await).unwrap();
    assert!(!text.contains("gold_standard"));
    c.db.cleanup().await;
}

#[tokio::test]
async fn parquet_dataset_is_stored_and_audited() {
    let Some(c) = Ctx::new().await else { return };
    cohort(&c).await;
    let (_, owner) = c.token(Role::Owner).await;
    let done = dataset(&c, &owner, json!({"format": "parquet", "reason": REASON})).await;
    assert_eq!(done["status"], "succeeded", "{done}");
    assert_eq!(done["format"], "dataset_parquet");
    assert_eq!(done["content_type"], "application/vnd.apache.parquet");
    assert!(done["file_name"].as_str().unwrap().ends_with(".parquet"));
    let bytes = c.artifact(done["id"].as_str().unwrap()).await;
    assert_eq!(&bytes[..4], b"PAR1");
    assert_eq!(&bytes[bytes.len() - 4..], b"PAR1");
    let created = c.audit_rows("export.create").await;
    assert_eq!(created[0]["after"]["include_labels"], true);
    assert_eq!(created[0]["after"]["purpose"], "research");
    assert_eq!(c.audit_rows("export.complete").await.len(), 1);
    c.db.cleanup().await;
}

#[tokio::test]
async fn dataset_requests_are_validated() {
    let Some(c) = Ctx::new().await else { return };
    let (_, clin) = c.token(Role::Clinician).await;
    let (_, viewer) = c.token(Role::Viewer).await;
    let (_, superadmin) = c.token(Role::Superadmin).await;

    // Datasets are de-identified only; labels only on dataset formats.
    let mut b = body("dataset_jsonl", "identified", "clinical_care");
    let (s, e) = c.request_export(&clin, b.clone()).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{e}");
    assert_eq!(e["invalid_params"][0]["name"], "mode");
    b = body("csv", "deidentified", "research");
    b["include_labels"] = json!(true);
    let (s, e) = c.request_export(&clin, b).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{e}");
    assert_eq!(e["invalid_params"][0]["name"], "include_labels");
    // The org route accepts a dataset format too.
    let (s, e) = c
        .request_export(
            &clin,
            body("dataset_parquet", "deidentified", "quality_improvement"),
        )
        .await;
    assert_eq!(s, StatusCode::ACCEPTED, "{e}");

    for (bearer, want) in [
        (&viewer, StatusCode::FORBIDDEN),
        (&superadmin, StatusCode::FORBIDDEN),
    ] {
        let (s, e) = c
            .call(
                Method::POST,
                "/v1/jobs/export-dataset",
                Some(json!({"format": "jsonl", "reason": REASON})),
                bearer,
            )
            .await;
        assert_eq!(s, want, "{e}");
    }
    let (s, _) = c
        .call(
            Method::POST,
            "/v1/jobs/export-dataset",
            Some(json!({"format": "csv", "reason": REASON})),
            &clin,
        )
        .await;
    assert!(s.is_client_error());
    let (s, e) = c
        .call(
            Method::POST,
            "/v1/jobs/export-dataset",
            Some(json!({"format": "jsonl", "reason": "short"})),
            &clin,
        )
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{e}");
    c.db.cleanup().await;
}

// ---- erasure --------------------------------------------------------------------------

async fn count(c: &Ctx, sql: &'static str, p: Uuid) -> i64 {
    sqlx::query_scalar(sql)
        .bind(p)
        .fetch_one(&c.db.pool)
        .await
        .unwrap()
}

async fn put_blob(c: &Ctx, key: &str) {
    c.state
        .blobs
        .put(key, "application/octet-stream", Bytes::from_static(b"x"))
        .await
        .unwrap();
}

async fn blob_exists(c: &Ctx, key: &str) -> bool {
    c.state.blobs.get(key).await.unwrap().is_some()
}

async fn erase(c: &Ctx, bearer: &str, participant: Uuid) -> (StatusCode, Value) {
    c.call(
        Method::POST,
        &format!("/v1/orgs/{}/participants/{participant}/erasure", c.org),
        None,
        bearer,
    )
    .await
}

async fn request_row(c: &Ctx, participant: Uuid) -> Value {
    sqlx::query_scalar(
        "SELECT to_jsonb(r) FROM erasure_requests r WHERE participant_id = $1
         ORDER BY requested_at DESC LIMIT 1",
    )
    .bind(participant)
    .fetch_one(&c.db.pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn erasure_removes_everything_leaves_a_tombstone_and_is_idempotent() {
    let Some(c) = Ctx::new().await else { return };
    let (a, _) = cohort(&c).await;
    let (_, admin) = c.token(Role::Admin).await;
    let (_, clin) = c.token(Role::Clinician).await;
    let frames = format!("sessions/{}/TOKEN-ALPHA/SESS-ONE/frames.jsonl.enc", c.org);
    let report = format!("reports/{}/TOKEN-ALPHA/SESS-TWO/report.pdf", c.org);
    put_blob(&c, &frames).await;
    put_blob(&c, &report).await;
    sqlx::query(
        "INSERT INTO care_events (org_id, participant_id, kind, occurred_at)
         VALUES ($1, $2, 'task', now())",
    )
    .bind(c.org)
    .bind(a)
    .execute(&c.db.pool)
    .await
    .unwrap();
    // An unexpired artifact that includes the participant (EXJ-8), and one that does not.
    let with = c
        .export_and_run(&clin, body("csv", "identified", "clinical_care"))
        .await;
    assert_eq!(with["status"], "succeeded", "{with}");
    let with_id = with["id"].as_str().unwrap().to_owned();
    let with_key: String = sqlx::query_scalar("SELECT blob_key FROM exports WHERE id = $1")
        .bind(with_id.parse::<Uuid>().unwrap())
        .fetch_one(&c.db.pool)
        .await
        .unwrap();

    let (s, made) = erase(&c, &admin, a).await;
    assert_eq!(s, StatusCode::ACCEPTED, "{made}");
    assert_eq!(made["status"], "pending");
    assert_eq!(made["requested_by_type"], "user");
    // A second request while one is open returns it.
    let (s, again) = erase(&c, &admin, a).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(again["duplicate"], true);
    assert_eq!(again["id"], made["id"]);

    c.run_jobs().await;

    let req = request_row(&c, a).await;
    assert_eq!(req["status"], "completed", "{req}");
    assert!(req["completed_at"].is_string());
    assert_eq!(req["outcome"]["sessions_deleted"], 3);
    assert_eq!(req["outcome"]["sessions_held"], 0);
    assert_eq!(req["outcome"]["care_events_deleted"], 1);
    assert_eq!(req["outcome"]["export_artifacts_deleted"], 1);
    for sql in [
        "SELECT count(*) FROM sessions WHERE participant_id = $1",
        "SELECT count(*) FROM care_events WHERE participant_id = $1",
    ] {
        assert_eq!(count(&c, sql, a).await, 0, "{sql}");
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM labels")
            .fetch_one(&c.db.pool)
            .await
            .unwrap(),
        0
    );
    assert!(!blob_exists(&c, &frames).await);
    assert!(!blob_exists(&c, &report).await);
    assert!(!blob_exists(&c, &with_key).await);
    let (_, expired) = c
        .call(Method::GET, &format!("/v1/exports/{with_id}"), None, &clin)
        .await;
    assert_eq!(expired["status"], "expired");
    // The accounting of disclosures stays (EXJ-6).
    assert!(c
        .export_items(&with_id)
        .await
        .contains(&"SESS-ONE".to_owned()));

    // The tombstone: same token, withdrawn, erased, no demographics.
    let (token, erased, withdrawn, bracket): (String, bool, bool, Option<String>) = sqlx::query_as(
        "SELECT token, erased_at IS NOT NULL, withdrawn_at IS NOT NULL, age_bracket_at_enrol
             FROM participants WHERE id = $1",
    )
    .bind(a)
    .fetch_one(&c.db.pool)
    .await
    .unwrap();
    assert_eq!(token, "TOKEN-ALPHA");
    assert!(erased && withdrawn);
    assert_eq!(bracket, None);

    // Audited without the token.
    let rows = c.audit_rows("participant.erasure_execute").await;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["after"]["status"], "completed");
    assert_eq!(c.audit_rows("export.expire").await.len(), 1);
    assert!(!c.audit_dump().await.contains("TOKEN-ALPHA"));

    // Re-running the job changes nothing (idempotent).
    sqlx::query("UPDATE jobs SET status = 'queued' WHERE kind = 'erasure'")
        .execute(&c.db.pool)
        .await
        .unwrap();
    assert_eq!(c.run_jobs().await, 1);
    assert_eq!(c.audit_rows("participant.erasure_execute").await.len(), 1);

    // The admin list shows it.
    let (s, page) = c
        .call(
            Method::GET,
            &format!("/v1/orgs/{}/erasure-requests", c.org),
            None,
            &admin,
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{page}");
    assert_eq!(page["items"][0]["status"], "completed");
    assert_eq!(page["items"][0]["outcome"]["sessions_deleted"], 3);
    let (s, _) = c
        .call(
            Method::GET,
            &format!("/v1/orgs/{}/erasure-requests", c.org),
            None,
            &clin,
        )
        .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    c.db.cleanup().await;
}

/// A signed clinical review, notes or a final letter put the participant on hold
/// (ADR-0008 Q2 open): those records and their sessions stay, the rest goes.
#[tokio::test]
async fn signed_clinical_records_are_held_and_the_rest_is_erased() {
    let Some(c) = Ctx::new().await else { return };
    let p = c.participant(c.org, "TOKEN-HELD").await;
    c.session(c.org, p, "SESS-SIGNED", "2026-10-01T10:00:00Z")
        .await;
    c.session(c.org, p, "SESS-DRAFT", "2026-10-02T10:00:00Z")
        .await;
    c.session(c.org, p, "SESS-PLAIN", "2026-10-03T10:00:00Z")
        .await;
    let (signer, _) = c.token(Role::Clinician).await;
    let pool = &c.db.pool;
    // SESS-SIGNED: a signed version and its signature. SESS-DRAFT: a draft-only review.
    for (session, status) in [("SESS-SIGNED", "signed"), ("SESS-DRAFT", "draft")] {
        let review: Uuid = sqlx::query_scalar(
            "INSERT INTO clinical_reviews (org_id, session_id, state) VALUES ($1, $2, 'open')
             RETURNING id",
        )
        .bind(c.org)
        .bind(session)
        .fetch_one(pool)
        .await
        .unwrap();
        let signed = status == "signed";
        let version: Uuid = sqlx::query_scalar(
            "INSERT INTO clinical_review_versions
               (review_id, org_id, version_no, status, disposition, content_hash, signed_at)
             VALUES ($1, $2, 1, $3, CASE WHEN $4 THEN 'monitor' END,
                     CASE WHEN $4 THEN decode(repeat('ab', 32), 'hex') END,
                     CASE WHEN $4 THEN now() END)
             RETURNING id",
        )
        .bind(review)
        .bind(c.org)
        .bind(status)
        .bind(signed)
        .fetch_one(pool)
        .await
        .unwrap();
        if signed {
            sqlx::query(
                "INSERT INTO clinical_signatures (org_id, review_id, version_id, signer_id, role,
                     meaning, content_hash, signer_display_name)
                 VALUES ($1, $2, $3, $4, 'sign', 'reviewed', decode(repeat('ab', 32), 'hex'),
                         'Dr. Test')",
            )
            .bind(c.org)
            .bind(review)
            .bind(version)
            .bind(signer)
            .execute(pool)
            .await
            .unwrap();
        }
    }
    sqlx::query(
        "INSERT INTO patient_refs (org_id, participant_id, assigning_authority, mrn_ct,
             mrn_key_id, mrn_bidx)
         VALUES ($1, $2, 'TUTH', '\\x00', gen_random_uuid(), decode(repeat('cd', 32), 'hex'))",
    )
    .bind(c.org)
    .bind(p)
    .execute(pool)
    .await
    .unwrap();

    let (_, owner) = c.token(Role::Owner).await;
    let (s, _) = erase(&c, &owner, p).await;
    assert_eq!(s, StatusCode::ACCEPTED);
    c.run_jobs().await;

    let req = request_row(&c, p).await;
    assert_eq!(req["status"], "held", "{req}");
    assert_eq!(req["hold_reason"], "clinical_record");
    assert_eq!(req["completed_at"], Value::Null);
    assert_eq!(req["outcome"]["sessions_deleted"], 2);
    assert_eq!(req["outcome"]["sessions_held"], 1);
    assert_eq!(req["outcome"]["clinical_drafts_deleted"], 1);
    assert_eq!(req["outcome"]["patient_ref_deleted"], false);
    let left: Vec<String> =
        sqlx::query_scalar("SELECT id FROM sessions WHERE participant_id = $1 ORDER BY id")
            .bind(p)
            .fetch_all(pool)
            .await
            .unwrap();
    assert_eq!(left, ["SESS-SIGNED"]);
    assert_eq!(
        count(
            &c,
            "SELECT count(*) FROM patient_refs WHERE participant_id = $1",
            p
        )
        .await,
        1
    );
    assert_eq!(
        count(
            &c,
            "SELECT count(*) FROM participants WHERE id = $1 AND erased_at IS NULL
                AND withdrawn_at IS NOT NULL",
            p
        )
        .await,
        1
    );
    // A held request stays open: a new request returns it.
    let (s, again) = erase(&c, &owner, p).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(again["status"], "held");
    c.db.cleanup().await;
}

#[tokio::test]
async fn device_requests_queue_the_job_and_left_over_requests_are_scheduled() {
    let Some(c) = Ctx::new().await else { return };
    let p = c.participant(c.org, "TOKEN-DEV").await;
    c.session(c.org, p, "SESS-DEV", "2026-10-01T10:00:00Z")
        .await;
    let (_, device) = seed_device(&c.db.pool, c.org).await;
    let (s, b) = c
        .call(
            Method::POST,
            "/v1/participants/TOKEN-DEV/erasure",
            None,
            &device,
        )
        .await;
    assert_eq!(s, StatusCode::ACCEPTED, "{b}");
    let job: Option<Uuid> = sqlx::query_scalar("SELECT job_id FROM erasure_requests")
        .fetch_one(&c.db.pool)
        .await
        .unwrap();
    assert!(job.is_some());
    c.run_jobs().await;
    assert_eq!(request_row(&c, p).await["status"], "completed");

    // A request filed before this package (no job) is picked up by the worker's tick.
    let q = c.participant(c.org, "TOKEN-OLD").await;
    sqlx::query(
        "INSERT INTO erasure_requests (org_id, participant_id, requested_by_type)
         VALUES ($1, $2, 'system')",
    )
    .bind(c.org)
    .bind(q)
    .execute(&c.db.pool)
    .await
    .unwrap();
    assert_eq!(erasure::schedule_pending(&c.db.pool).await.unwrap(), 1);
    assert_eq!(erasure::schedule_pending(&c.db.pool).await.unwrap(), 0);
    c.run_jobs().await;
    assert_eq!(request_row(&c, q).await["status"], "completed");
    c.db.cleanup().await;
}

// ---- retention ------------------------------------------------------------------------

#[tokio::test]
async fn retention_sweep_dry_run_then_purges_old_frames_only() {
    let Some(c) = Ctx::new().await else { return };
    let p = c.participant(c.org, "TOKEN-RET").await;
    c.session(c.org, p, "SESS-OLD", "2024-01-01T10:00:00Z")
        .await;
    c.session(c.org, p, "SESS-NEW", "2026-10-01T10:00:00Z")
        .await;
    sqlx::query("UPDATE organizations SET retention_months = 12 WHERE id = $1")
        .bind(c.org)
        .execute(&c.db.pool)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE sessions SET received_at = now() - interval '13 months' WHERE id = 'SESS-OLD'",
    )
    .execute(&c.db.pool)
    .await
    .unwrap();
    let old = format!("sessions/{}/TOKEN-RET/SESS-OLD/frames.jsonl.enc", c.org);
    let new = format!("sessions/{}/TOKEN-RET/SESS-NEW/frames.jsonl.enc", c.org);
    put_blob(&c, &old).await;
    put_blob(&c, &new).await;
    let (_, admin) = c.token(Role::Admin).await;
    let uri = format!("/v1/orgs/{}/retention-sweep", c.org);

    // Dry run (the default): counts, deletes nothing.
    let (s, job) = c.call(Method::POST, &uri, Some(json!({})), &admin).await;
    assert_eq!(s, StatusCode::ACCEPTED, "{job}");
    assert_eq!(job["dry_run"], true);
    c.run_jobs().await;
    let (s, done) = c
        .call(
            Method::GET,
            &format!("/v1/jobs/{}", job["job_id"].as_str().unwrap()),
            None,
            &admin,
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{done}");
    assert_eq!(done["status"], "succeeded", "{done}");
    assert_eq!(done["result"]["orgs"][0]["frames_purged"], 1);
    assert!(blob_exists(&c, &old).await);
    assert_eq!(c.audit_rows("retention.sweep_dry_run").await.len(), 1);

    // The real run purges the old session's frames; the session stays.
    let (s, _) = c
        .call(Method::POST, &uri, Some(json!({"dry_run": false})), &admin)
        .await;
    assert_eq!(s, StatusCode::ACCEPTED);
    c.run_jobs().await;
    assert!(!blob_exists(&c, &old).await);
    assert!(blob_exists(&c, &new).await);
    let purged: Vec<String> = sqlx::query_scalar(
        "SELECT id FROM sessions WHERE frames_purged_at IS NOT NULL ORDER BY id",
    )
    .fetch_all(&c.db.pool)
    .await
    .unwrap();
    assert_eq!(purged, ["SESS-OLD"]);
    assert_eq!(
        count(
            &c,
            "SELECT count(*) FROM sessions WHERE participant_id = $1",
            p
        )
        .await,
        2
    );
    let audit = c.audit_rows("retention.sweep").await;
    assert_eq!(audit.len(), 1);
    assert_eq!(audit[0]["after"]["frames_purged"], 1);
    assert_eq!(c.audit_rows("retention.sweep_request").await.len(), 2);

    // Nothing left to purge.
    let (_, again) = c
        .call(Method::POST, &uri, Some(json!({"dry_run": false})), &admin)
        .await;
    c.run_jobs().await;
    let (_, done) = c
        .call(
            Method::GET,
            &format!("/v1/jobs/{}", again["job_id"].as_str().unwrap()),
            None,
            &admin,
        )
        .await;
    assert_eq!(done["result"]["orgs"][0]["frames_purged"], 0);

    // Only owner/admin may sweep.
    let (_, researcher) = c.token(Role::Researcher).await;
    let (s, _) = c
        .call(Method::POST, &uri, Some(json!({})), &researcher)
        .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    c.db.cleanup().await;
}

#[tokio::test]
async fn the_nightly_sweep_is_queued_once_a_day_for_every_org() {
    let Some(c) = Ctx::new().await else { return };
    let first = retention::schedule_nightly(&c.db.pool).await.unwrap();
    assert!(first.is_some());
    assert_eq!(retention::schedule_nightly(&c.db.pool).await.unwrap(), None);
    c.run_jobs().await;
    let (status, result): (String, Value) =
        sqlx::query_as("SELECT status, result FROM jobs WHERE id = $1")
            .bind(first.unwrap())
            .fetch_one(&c.db.pool)
            .await
            .unwrap();
    assert_eq!(status, "succeeded");
    assert_eq!(result["dry_run"], false);
    // Nothing purged: a scheduled run writes no audit rows.
    assert!(c.audit_rows("retention.sweep").await.is_empty());
    c.db.cleanup().await;
}
