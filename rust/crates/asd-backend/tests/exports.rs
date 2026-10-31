//! owner: WP-I.6
//!
//! Export routes (EXJ-1, EXJ-2, EXJ-5, EXJ-6): request validation and authorisation,
//! download URLs (audited, five minutes, org-scoped, `410` after expiry), proxy-mode
//! tokens, and the per-participant disclosure list. Needs `DATABASE_URL`.

mod common;
mod export_support;
mod participant_support;

use asd_contract::api_v1::Role;
use chrono::{DateTime, Utc};
use common::auth::seed_org;
use export_support::body;
use http::{Method, StatusCode};
use participant_support::*;
use serde_json::{json, Value};
use uuid::Uuid;

#[tokio::test]
async fn request_validation_and_authorisation() {
    let Some(c) = Ctx::new().await else { return };
    let p = c.participant(c.org, "P-1").await;
    c.session(c.org, p, "S-1", "2026-10-01T10:00:00Z").await;
    let (_, clin) = c.token(Role::Clinician).await;
    let (_, researcher) = c.token(Role::Researcher).await;
    let (_, reviewer) = c.token(Role::Reviewer).await;
    let (_, root) = c.token(Role::Superadmin).await;

    // XC-5: identified + research is refused before any job exists.
    let (s, b) = c
        .request_export(&clin, body("csv", "identified", "research"))
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{b}");
    assert_eq!(slug_of(&b), "consent-purpose-mismatch");
    let (s, b) = c
        .request_export(&clin, body("csv", "identified", "quality_improvement"))
        .await;
    assert_eq!(
        (s, slug_of(&b)),
        (StatusCode::BAD_REQUEST, "consent-purpose-mismatch")
    );

    // EXJ-2: identified needs ExportIdentified (a researcher has only de-identified).
    let (s, _) = c
        .request_export(&researcher, body("csv", "identified", "clinical_care"))
        .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, _) = c
        .request_export(&reviewer, body("csv", "deidentified", "research"))
        .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    // RBAC-3: the vendor never exports clinic data, not even de-identified.
    let (s, _) = c
        .request_export(&root, body("csv", "deidentified", "research"))
        .await;
    assert_eq!(s, StatusCode::FORBIDDEN);

    // HL7 needs the org's MRN mode (409), names one session, is identified only.
    let mut hl7 = body("hl7v2", "identified", "clinical_care");
    hl7["filters"] = json!({"session_ids": ["S-1"]});
    let (s, b) = c.request_export(&clin, hl7.clone()).await;
    assert_eq!(
        (s, slug_of(&b)),
        (StatusCode::CONFLICT, "patient-ref-disabled"),
        "{b}"
    );
    let mut hl7_deid = hl7.clone();
    hl7_deid["mode"] = json!("deidentified");
    let (s, b) = c.request_export(&clin, hl7_deid).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{b}");
    assert_eq!(b["invalid_params"][0]["name"], "mode");
    let (s, b) = c
        .request_export(&clin, body("hl7v2", "identified", "clinical_care"))
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert_eq!(b["invalid_params"][0]["name"], "filters.session_ids");

    // PDF/A is WP-I.5: a clear refusal, not a failed job.
    let (s, b) = c
        .request_export(&clin, body("pdf_summary", "identified", "clinical_care"))
        .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert_eq!(b["invalid_params"][0]["name"], "format");
    assert!(b["invalid_params"][0]["reason"]
        .as_str()
        .unwrap()
        .contains("WP-I.5"));

    // Reason length, filters, unknown fields, unknown destinations.
    let mut short = body("csv", "deidentified", "research");
    short["reason"] = json!("too short");
    let (s, b) = c.request_export(&clin, short).await;
    assert_eq!(
        (s, b["invalid_params"][0]["name"].clone()),
        (StatusCode::BAD_REQUEST, json!("reason"))
    );
    let mut f = body("csv", "deidentified", "research");
    f["filters"] = json!({"flag": "purple", "clinical_state": ["signed"]});
    let (s, b) = c.request_export(&clin, f).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert_eq!(b["invalid_params"].as_array().unwrap().len(), 2, "{b}");
    let mut extra = body("csv", "deidentified", "research");
    extra["filters"] = json!({"mrn": "x"});
    let (s, _) = c.request_export(&clin, extra).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    let mut dest = body("csv", "deidentified", "research");
    dest["destination_ids"] = json!([Uuid::new_v4()]);
    let (s, b) = c.request_export(&clin, dest).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert_eq!(b["invalid_params"][0]["name"], "destination_ids");

    // Another org's path is 404.
    let other = seed_org(&c.db.pool, "Clinic B").await;
    let (s, _) = c
        .call(
            Method::POST,
            &format!("/v1/orgs/{other}/exports"),
            Some(body("csv", "deidentified", "research")),
            &clin,
        )
        .await;
    assert_eq!(s, StatusCode::NOT_FOUND);

    assert_eq!(c.audit_rows("export.create").await.len(), 0);
    let jobs: i64 = sqlx::query_scalar("SELECT count(*) FROM jobs")
        .fetch_one(&c.db.pool)
        .await
        .unwrap();
    assert_eq!(jobs, 0, "no refused request queued a job");

    // A valid request: 202, queued, one job, one audit row with the reason.
    let (s, made) = c
        .request_export(&researcher, body("csv", "deidentified", "research"))
        .await;
    assert_eq!(s, StatusCode::ACCEPTED, "{made}");
    assert_eq!(made["status"], "queued");
    assert_eq!(made["format"], "csv");
    assert_eq!(made["kind"], "bulk");
    assert!(made.get("download_secret").is_none());
    let rows = c.audit_rows("export.create").await;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["target_id"], made["id"]);
    assert_eq!(
        rows[0]["after"]["reason"],
        "Quarterly extract for the approved study"
    );
    let kind: String = sqlx::query_scalar("SELECT kind FROM jobs")
        .fetch_one(&c.db.pool)
        .await
        .unwrap();
    assert_eq!(kind, "export");

    // Listing and reading: ExportRead, org-scoped; superadmin may read metadata.
    let (s, page) = c
        .call(
            Method::GET,
            &format!("/v1/orgs/{}/exports", c.org),
            None,
            &clin,
        )
        .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(page["total"], 1);
    assert_eq!(page["items"][0]["id"], made["id"]);
    let one = format!("/v1/exports/{}", made["id"].as_str().unwrap());
    let (s, _) = c.call(Method::GET, &one, None, &reviewer).await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, _) = c.call(Method::GET, &one, None, &root).await;
    assert_eq!(s, StatusCode::OK);
    let (_, other_admin) = c.token_in(Some(other), Role::Admin).await;
    let (s, _) = c.call(Method::GET, &one, None, &other_admin).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    c.db.cleanup().await;
}

async fn download(c: &Ctx, export: &Value, bearer: &str) -> (StatusCode, Value) {
    c.call(
        Method::POST,
        &format!(
            "/v1/exports/{}/download-url",
            export["id"].as_str().unwrap()
        ),
        None,
        bearer,
    )
    .await
}

#[tokio::test]
async fn download_urls_are_audited_short_lived_and_org_scoped() {
    let Some(c) = Ctx::new().await else { return };
    let p = c.participant(c.org, "P-1").await;
    c.exportable_session(p, "P-1", "S-1", "2026-10-01T10:00:00Z", Some(true))
        .await;
    let (_, clin) = c.token(Role::Clinician).await;
    let (_, admin) = c.token(Role::Admin).await;
    let (_, researcher) = c.token(Role::Researcher).await;
    let (_, root) = c.token(Role::Superadmin).await;

    let done = c
        .export_and_run(&researcher, body("csv", "deidentified", "research"))
        .await;
    assert_eq!(done["status"], "succeeded", "{done}");

    // Not ready yet: 409. (A second export, never run.)
    let (_, queued) = c
        .request_export(&researcher, body("xlsx", "deidentified", "research"))
        .await;
    let (s, b) = download(&c, &queued, &researcher).await;
    assert_eq!((s, slug_of(&b)), (StatusCode::CONFLICT, "conflict"));

    // Proxy blob mode (the test default): a relative URL valid five minutes.
    let (s, url) = download(&c, &done, &researcher).await;
    assert_eq!(s, StatusCode::OK, "{url}");
    let link = url["url"].as_str().unwrap().to_owned();
    assert!(link.starts_with(&format!(
        "/v1/exports/{}/file?t=",
        done["id"].as_str().unwrap()
    )));
    let exp: DateTime<Utc> = url["expires_at"].as_str().unwrap().parse().unwrap();
    let ttl = (exp - Utc::now()).num_seconds();
    assert!((295..=300).contains(&ttl), "{ttl}");
    let rows = c.audit_rows("export.download").await;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["target_id"], done["id"]);
    assert!(
        !c.audit_dump().await.contains("t="),
        "the URL is never audited"
    );

    // The file: no bearer needed, the token is the credential; bytes are the artifact.
    let (s, headers, bytes) = raw_get(&c, &link).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(headers["cache-control"], "no-store");
    assert!(headers["content-disposition"]
        .to_str()
        .unwrap()
        .starts_with("attachment; filename=\"export-"));
    assert_eq!(bytes, c.artifact(done["id"].as_str().unwrap()).await);

    // Tampered and expired tokens fail.
    let tampered = format!("{}x", &link[..link.len() - 1]);
    assert_eq!(raw_get(&c, &tampered).await.0, StatusCode::FORBIDDEN);
    let (exp_part, rest) = link.split_once("?t=").unwrap().1.split_once('.').unwrap();
    let older = link.replacen(&format!("t={exp_part}.{rest}"), &format!("t=1.{rest}"), 1);
    assert_eq!(raw_get(&c, &older).await.0, StatusCode::FORBIDDEN);
    let other_export = link.replacen(
        done["id"].as_str().unwrap(),
        queued["id"].as_str().unwrap(),
        1,
    );
    assert_ne!(raw_get(&c, &other_export).await.0, StatusCode::OK);
    let no_token = link.split_once('?').unwrap().0.to_owned();
    assert_eq!(raw_get(&c, &no_token).await.0, StatusCode::FORBIDDEN);

    // Same org with ExportRead: fine (de-identified). Superadmin: never a copy. Other org: 404.
    assert_eq!(download(&c, &done, &admin).await.0, StatusCode::OK);
    assert_eq!(download(&c, &done, &root).await.0, StatusCode::FORBIDDEN);
    let other = seed_org(&c.db.pool, "Clinic B").await;
    let (_, other_admin) = c.token_in(Some(other), Role::Admin).await;
    assert_eq!(
        download(&c, &done, &other_admin).await.0,
        StatusCode::NOT_FOUND
    );

    // Identified exports: the requester or a caller with ExportIdentified only.
    let ident = c
        .export_and_run(&clin, body("csv", "identified", "clinical_care"))
        .await;
    assert_eq!(ident["status"], "succeeded", "{ident}");
    assert_eq!(download(&c, &ident, &clin).await.0, StatusCode::OK);
    assert_eq!(download(&c, &ident, &admin).await.0, StatusCode::FORBIDDEN);
    let (_, owner) = c.token(Role::Owner).await;
    assert_eq!(download(&c, &ident, &owner).await.0, StatusCode::OK);

    // Past expires_at: 410 export-expired, for the URL and for an issued link.
    sqlx::query("UPDATE exports SET expires_at = now() - interval '1 minute' WHERE id = $1")
        .bind(done["id"].as_str().unwrap().parse::<Uuid>().unwrap())
        .execute(&c.db.pool)
        .await
        .unwrap();
    let (s, b) = download(&c, &done, &researcher).await;
    assert_eq!((s, slug_of(&b)), (StatusCode::GONE, "export-expired"));
    assert_eq!(raw_get(&c, &link).await.0, StatusCode::GONE);
    c.db.cleanup().await;
}

async fn raw_get(c: &Ctx, uri: &str) -> (StatusCode, http::HeaderMap, Vec<u8>) {
    use http_body_util::BodyExt;
    use tower::ServiceExt;
    let response = c.app.clone().oneshot(common::get(uri)).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, headers, bytes.to_vec())
}

#[tokio::test]
async fn presigned_mode_hands_out_a_signed_get() {
    let Some(c) = Ctx::new().await else { return };
    let p = c.participant(c.org, "P-1").await;
    c.exportable_session(p, "P-1", "S-1", "2026-10-01T10:00:00Z", Some(true))
        .await;
    let (_, researcher) = c.token(Role::Researcher).await;
    let done = c
        .export_and_run(&researcher, body("csv", "deidentified", "research"))
        .await;
    // Same database and blob store, server switched to presigned URLs.
    let mut config = (*c.state.config).clone();
    config.blob_mode = asd_backend::config::BlobMode::Presigned;
    let mut state = c.state.clone();
    state.config = std::sync::Arc::new(config);
    let app = common::router(state);
    let (s, _, url) = common::send(
        app,
        common::auth::empty(
            Method::POST,
            &format!("/v1/exports/{}/download-url", done["id"].as_str().unwrap()),
            Some(&researcher),
        ),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{url}");
    let link = url["url"].as_str().unwrap();
    assert!(link.starts_with("https://blobs.memory.test/"), "{link}");
    assert!(link.contains("method=GET"));
    assert!(link.contains(&format!(
        "exports/{}/{}",
        c.org,
        done["id"].as_str().unwrap()
    )));
    let exp: DateTime<Utc> = url["expires_at"].as_str().unwrap().parse().unwrap();
    assert!((exp - Utc::now()).num_seconds() <= 300);
    c.db.cleanup().await;
}

#[tokio::test]
async fn disclosures_list_the_exports_that_included_a_participant() {
    let Some(c) = Ctx::new().await else { return };
    let a = c.participant(c.org, "P-A").await;
    let b = c.participant(c.org, "P-B").await;
    c.exportable_session(a, "P-A", "S-A1", "2026-10-01T10:00:00Z", Some(true))
        .await;
    c.exportable_session(b, "P-B", "S-B1", "2026-10-02T10:00:00Z", Some(true))
        .await;
    let (_, clin) = c.token(Role::Clinician).await;
    let (_, researcher) = c.token(Role::Researcher).await;
    let done = c
        .export_and_run(&clin, body("csv", "deidentified", "research"))
        .await;
    let mut only_b = body("csv", "identified", "clinical_care");
    only_b["filters"] = json!({"participant_ids": [b]});
    let other = c.export_and_run(&clin, only_b).await;
    assert_eq!(other["row_count"], 1);

    let (s, page) = c
        .call(
            Method::GET,
            &format!("/v1/participants/{a}/disclosures"),
            None,
            &clin,
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{page}");
    assert_eq!(page["total"], 1);
    assert_eq!(page["items"][0]["export_id"], done["id"]);
    assert_eq!(page["items"][0]["session_ids"], json!(["S-A1"]));
    assert_eq!(page["items"][0]["purpose"], "research");
    let (_, page_b) = c
        .call(
            Method::GET,
            &format!("/v1/participants/{b}/disclosures"),
            None,
            &clin,
        )
        .await;
    assert_eq!(page_b["total"], 2);
    // ClinicalContentRead only.
    let (s, _) = c
        .call(
            Method::GET,
            &format!("/v1/participants/{a}/disclosures"),
            None,
            &researcher,
        )
        .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    c.db.cleanup().await;
}
