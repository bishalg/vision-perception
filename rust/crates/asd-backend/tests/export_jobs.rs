//! owner: WP-I.6
//!
//! The `export` job (EXJ-3, EXJ-4, EXJ-9, XC-4, XC-5): consent filtering with exclusion
//! counts, de-identified artifacts free of identifiers, idempotent re-runs, the stored
//! hash, HL7 with the MRN (audited), retry after a blob outage, and the expiry sweep.
//! Needs `DATABASE_URL`.

mod common;
mod export_support;
mod participant_support;

use std::sync::Arc;

use asd_backend::blob::{BlobError, BlobStore, PresignedPut, StoredBlob};
use asd_backend::jobs::{export as export_job, runner, JobContext};
use asd_contract::api_v1::Role;
use async_trait::async_trait;
use bytes::Bytes;
use export_support::body;
use http::{Method, StatusCode};
use participant_support::*;
use serde_json::json;
use sha2::{Digest, Sha256};
use uuid::Uuid;

fn uuid(v: &serde_json::Value) -> Uuid {
    v.as_str().unwrap().parse().unwrap()
}

/// P-A: S-1 research-consented, S-2 no consent recorded; P-W (withdrawn): S-3 consented.
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
        w,
        "TOKEN-WITHDRAWN",
        "SESS-THREE",
        "2026-10-02T10:00:00Z",
        Some(true),
    )
    .await;
    c.withdraw(w).await;
    (a, w)
}

#[tokio::test]
async fn research_exports_keep_only_consented_sessions_and_count_exclusions() {
    let Some(c) = Ctx::new().await else { return };
    cohort(&c).await;
    let (_, researcher) = c.token(Role::Researcher).await;
    let (_, clin) = c.token(Role::Clinician).await;

    let done = c
        .export_and_run(&researcher, body("csv", "deidentified", "research"))
        .await;
    assert_eq!(done["status"], "succeeded", "{done}");
    assert_eq!(done["row_count"], 1);
    assert_eq!(
        done["excluded"],
        json!({"no_research_consent": 1, "withdrawn": 1})
    );
    assert_eq!(
        c.export_items(done["id"].as_str().unwrap()).await,
        ["SESS-ONE"]
    );
    assert_eq!(done["content_type"], "application/zip");
    assert!(done["expires_at"].is_string());

    // The stored hash and size are the artifact's.
    let bytes = c.artifact(done["id"].as_str().unwrap()).await;
    assert_eq!(done["sha256"], hex(&Sha256::digest(&bytes)));
    assert_eq!(done["byte_size"], bytes.len());

    // Clinical care includes sessions without research consent, never the withdrawn one.
    let care = c
        .export_and_run(&clin, body("csv", "identified", "clinical_care"))
        .await;
    assert_eq!(care["row_count"], 2);
    assert_eq!(care["excluded"], json!({"withdrawn": 1}));
    assert_eq!(
        c.export_items(care["id"].as_str().unwrap()).await,
        ["SESS-ONE", "SESS-TWO"]
    );

    // One completion audit row per export, with counts and no identifiers.
    let rows = c.audit_rows("export.complete").await;
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["after"]["row_count"], 1);
    assert!(!c.audit_dump().await.contains("TOKEN-ALPHA"));
    c.db.cleanup().await;
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

#[tokio::test]
async fn deidentified_artifacts_carry_no_token_or_session_id() {
    let Some(c) = Ctx::new().await else { return };
    let (a, _) = cohort(&c).await;
    let (_, clin) = c.token(Role::Clinician).await;

    // FHIR NDJSON is plain text, so the whole artifact can be searched.
    let deid = c
        .export_and_run(&clin, body("fhir_ndjson", "deidentified", "clinical_care"))
        .await;
    assert_eq!(deid["status"], "succeeded", "{deid}");
    assert_eq!(deid["row_count"], 2);
    let text = String::from_utf8(c.artifact(deid["id"].as_str().unwrap()).await).unwrap();
    assert_eq!(text.lines().count(), 2);
    for secret in [
        "TOKEN-ALPHA",
        "SESS-ONE",
        "SESS-TWO",
        &a.to_string(),
        "2026-10-01T",
    ] {
        assert!(
            !text.contains(secret),
            "de-identified artifact contains {secret}"
        );
    }
    assert!(
        text.contains(deid["id"].as_str().unwrap()),
        "watermark: export id"
    );

    // The identified export of the same sessions names them.
    let ident = c
        .export_and_run(&clin, body("fhir_ndjson", "identified", "clinical_care"))
        .await;
    let text = String::from_utf8(c.artifact(ident["id"].as_str().unwrap()).await).unwrap();
    assert!(text.contains("TOKEN-ALPHA"));
    c.db.cleanup().await;
}

#[tokio::test]
async fn jobs_are_idempotent_and_renders_deterministic() {
    let Some(c) = Ctx::new().await else { return };
    cohort(&c).await;
    let (_, clin) = c.token(Role::Clinician).await;
    let done = c
        .export_and_run(&clin, body("csv", "identified", "clinical_care"))
        .await;
    let id = uuid(&done["id"]);
    let key: String = sqlx::query_scalar("SELECT blob_key FROM exports WHERE id = $1")
        .bind(id)
        .fetch_one(&c.db.pool)
        .await
        .unwrap();
    assert_eq!(
        key,
        format!(
            "exports/{}/{id}/{}",
            c.org,
            done["file_name"].as_str().unwrap()
        )
    );

    // The job runs again (a reclaim after success): nothing is rendered twice.
    sqlx::query("UPDATE jobs SET status = 'queued'")
        .execute(&c.db.pool)
        .await
        .unwrap();
    assert_eq!(c.run_jobs().await, 1);
    let (_, again) = c
        .call(Method::GET, &format!("/v1/exports/{id}"), None, &clin)
        .await;
    assert_eq!(again["sha256"], done["sha256"]);
    assert_eq!(again["completed_at"], done["completed_at"]);
    assert_eq!(c.audit_rows("export.complete").await.len(), 1);

    // A crash before the record phase re-renders: same key, same bytes (XC-8).
    sqlx::query("UPDATE exports SET status = 'running' WHERE id = $1")
        .bind(id)
        .execute(&c.db.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE jobs SET status = 'queued'")
        .execute(&c.db.pool)
        .await
        .unwrap();
    assert_eq!(c.run_jobs().await, 1);
    let (_, rerun) = c
        .call(Method::GET, &format!("/v1/exports/{id}"), None, &clin)
        .await;
    assert_eq!(rerun["status"], "succeeded");
    assert_eq!(rerun["sha256"], done["sha256"]);
    assert_eq!(
        c.export_items(&id.to_string()).await.len(),
        2,
        "items replaced, not doubled"
    );
    let bytes = c.artifact(&id.to_string()).await;
    assert_eq!(rerun["sha256"], hex(&Sha256::digest(&bytes)));
    c.db.cleanup().await;
}

#[tokio::test]
async fn hl7_exports_carry_the_mrn_and_audit_its_read() {
    let Some(c) = Ctx::new().await else { return };
    let p = c.participant(c.org, "TOKEN-H").await;
    c.exportable_session(p, "TOKEN-H", "SESS-H", "2026-10-01T10:00:00Z", None)
        .await;
    c.set_mode(c.org, "mrn").await;
    let (_, clin) = c.token(Role::Clinician).await;
    let (s, b) = c
        .call(
            Method::PUT,
            &format!("/v1/participants/{p}/patient-ref"),
            Some(json!({"assigning_authority": "TUTH", "mrn": "MRN-778899"})),
            &clin,
        )
        .await;
    assert_eq!(s, StatusCode::CREATED, "{b}");

    let mut req = body("hl7v2", "identified", "clinical_care");
    req["filters"] = json!({"session_ids": ["SESS-H"]});
    let done = c.export_and_run(&clin, req).await;
    assert_eq!(done["status"], "succeeded", "{done}");
    assert!(done["file_name"].as_str().unwrap().ends_with(".hl7"));
    let text = String::from_utf8(c.artifact(done["id"].as_str().unwrap()).await).unwrap();
    assert!(text.starts_with("MSH|"), "{text}");
    assert!(text.contains("MRN-778899^^^TUTH^MR"));
    let rows = c.audit_rows("patient_ref.export_read").await;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["after"]["count"], 1);
    assert!(!c.audit_dump().await.contains("MRN-778899"));

    // De-identified exports never decrypt the MRN.
    let deid = c
        .export_and_run(&clin, body("csv", "deidentified", "clinical_care"))
        .await;
    assert_eq!(deid["status"], "succeeded");
    assert_eq!(c.audit_rows("patient_ref.export_read").await.len(), 1);
    c.db.cleanup().await;
}

/// A store that is down for writes.
struct DownStore;

#[async_trait]
impl BlobStore for DownStore {
    async fn presign_put(&self, _: &str, _: &str) -> Result<PresignedPut, BlobError> {
        Err(BlobError::Unavailable("down".into()))
    }
    async fn put(&self, _: &str, _: &str, _: Bytes) -> Result<(), BlobError> {
        Err(BlobError::Unavailable("down".into()))
    }
    async fn get(&self, _: &str) -> Result<Option<StoredBlob>, BlobError> {
        Err(BlobError::Unavailable("down".into()))
    }
    async fn ready(&self) -> Result<(), BlobError> {
        Err(BlobError::Unavailable("down".into()))
    }
}

#[tokio::test]
async fn a_blob_outage_retries_later_instead_of_failing() {
    let Some(c) = Ctx::new().await else { return };
    cohort(&c).await;
    let (_, clin) = c.token(Role::Clinician).await;
    let (s, made) = c
        .request_export(&clin, body("csv", "identified", "clinical_care"))
        .await;
    assert_eq!(s, StatusCode::ACCEPTED);
    let mut ctx = c.job_ctx();
    ctx.blobs = Some(Arc::new(DownStore));
    assert_eq!(
        runner::run_pending_in(&ctx, "w").await.unwrap(),
        1,
        "not re-run at once"
    );
    let (status, later): (String, bool) =
        sqlx::query_as("SELECT status, run_after > now() + interval '30 seconds' FROM jobs")
            .fetch_one(&c.db.pool)
            .await
            .unwrap();
    assert_eq!((status.as_str(), later), ("queued", true));
    let (_, e) = c
        .call(
            Method::GET,
            &format!("/v1/exports/{}", made["id"].as_str().unwrap()),
            None,
            &clin,
        )
        .await;
    assert_eq!(e["status"], "running");

    // Due again: the healthy store finishes it.
    sqlx::query("UPDATE jobs SET run_after = now()")
        .execute(&c.db.pool)
        .await
        .unwrap();
    assert_eq!(c.run_jobs().await, 1);
    let (_, e) = c
        .call(
            Method::GET,
            &format!("/v1/exports/{}", made["id"].as_str().unwrap()),
            None,
            &clin,
        )
        .await;
    assert_eq!(e["status"], "succeeded");
    c.db.cleanup().await;
}

#[tokio::test]
async fn a_worker_without_a_blob_store_fails_the_export_with_a_reason() {
    let Some(c) = Ctx::new().await else { return };
    cohort(&c).await;
    let (_, clin) = c.token(Role::Clinician).await;
    let (_, made) = c
        .request_export(&clin, body("csv", "identified", "clinical_care"))
        .await;
    // The norm tests' database-only runner: permanent failure, export marked failed.
    assert_eq!(runner::run_pending(&c.db.pool, "w").await.unwrap(), 1);
    let (_, e) = c
        .call(
            Method::GET,
            &format!("/v1/exports/{}", made["id"].as_str().unwrap()),
            None,
            &clin,
        )
        .await;
    assert_eq!(e["status"], "failed");
    assert!(e["error"].as_str().unwrap().contains("blob store"), "{e}");
    c.db.cleanup().await;
}

#[tokio::test]
async fn the_sweep_expires_artifacts_and_keeps_the_accounting() {
    let Some(c) = Ctx::new().await else { return };
    cohort(&c).await;
    let (_, clin) = c.token(Role::Clinician).await;
    let done = c
        .export_and_run(&clin, body("csv", "identified", "clinical_care"))
        .await;
    let id = uuid(&done["id"]);
    let ctx: JobContext = c.job_ctx();
    assert_eq!(
        export_job::sweep_expired(&ctx).await.unwrap(),
        0,
        "not due yet"
    );
    sqlx::query("UPDATE exports SET expires_at = now() - interval '1 second' WHERE id = $1")
        .bind(id)
        .execute(&c.db.pool)
        .await
        .unwrap();
    let key: String = sqlx::query_scalar("SELECT blob_key FROM exports WHERE id = $1")
        .bind(id)
        .fetch_one(&c.db.pool)
        .await
        .unwrap();
    assert_eq!(export_job::sweep_expired(&ctx).await.unwrap(), 1);
    assert!(
        c.state.blobs.get(&key).await.unwrap().is_none(),
        "artifact deleted"
    );
    let (_, e) = c
        .call(Method::GET, &format!("/v1/exports/{id}"), None, &clin)
        .await;
    assert_eq!(e["status"], "expired");
    assert_eq!(
        c.export_items(&id.to_string()).await.len(),
        2,
        "accounting stays"
    );
    assert_eq!(c.audit_rows("export.expire").await.len(), 1);
    assert_eq!(
        export_job::sweep_expired(&ctx).await.unwrap(),
        0,
        "re-runnable"
    );
    c.db.cleanup().await;
}
