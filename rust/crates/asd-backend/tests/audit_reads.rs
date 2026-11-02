//! owner: WP-B.20
//!
//! HIP-1: every response with clinical content or participant detail writes exactly one
//! `audit_log` row before the body is built; a failing audit insert is a `500` with no
//! data. Needs `DATABASE_URL`; skipped otherwise.

mod common;
mod export_support;
mod participant_support;

use asd_contract::api_v1::Role;
use export_support::body;
use http::{Method, StatusCode};
use participant_support::*;
use serde_json::{json, Value};
use uuid::Uuid;

/// Exactly one row for `action`, checked for target and org; returns it.
async fn one_row(c: &Ctx, action: &str, target_type: &str, target_id: &str) -> Value {
    let rows = c.audit_rows(action).await;
    assert_eq!(rows.len(), 1, "{action}: {rows:?}");
    let r = rows[0].clone();
    assert_eq!(r["target_type"], target_type, "{action}");
    assert_eq!(r["target_id"], target_id, "{action}");
    assert_eq!(r["org_id"], c.org.to_string(), "{action}");
    r
}

async fn raw(
    router: axum::Router,
    request: http::Request<axum::body::Body>,
) -> (StatusCode, http::HeaderMap, Vec<u8>) {
    use http_body_util::BodyExt;
    use tower::ServiceExt;
    let response = router.oneshot(request).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, headers, bytes.to_vec())
}

struct Seed {
    c: Ctx,
    participant: Uuid,
    clin: Uuid,
    token: String,
}

async fn seed() -> Option<Seed> {
    let c = Ctx::new().await?;
    let participant = c.participant(c.org, "P-1").await;
    c.exportable_session(
        participant,
        "P-1",
        "S-1",
        "2026-10-01T10:00:00Z",
        Some(true),
    )
    .await;
    let (clin, token) = c.token(Role::Clinician).await;
    Some(Seed {
        c,
        participant,
        clin,
        token,
    })
}

async fn get(s: &Seed, uri: &str) -> (StatusCode, Value) {
    s.c.call(Method::GET, uri, None, &s.token).await
}

#[tokio::test]
async fn session_reads_write_one_row_per_part() {
    let Some(s) = seed().await else { return };
    let c = &s.c;
    for (uri, part) in [
        ("/v1/sessions/S-1", "detail"),
        ("/v1/sessions/S-1/clips", "clips"),
        ("/v1/sessions/S-1/fhir", "fhir"),
    ] {
        let before = c.audit_rows("session.read").await.len();
        let (st, b) = get(&s, uri).await;
        assert_eq!(st, StatusCode::OK, "{uri} {b}");
        let rows = c.audit_rows("session.read").await;
        assert_eq!(rows.len(), before + 1, "{uri}");
        let r = rows.last().unwrap();
        assert_eq!(r["target_type"], "session");
        assert_eq!(r["target_id"], "S-1");
        assert_eq!(r["actor_id"], s.clin.to_string());
        assert_eq!(r["actor_type"], "user");
        assert_eq!(r["org_id"], c.org.to_string());
        assert_eq!(r["after"], json!({ "part": part }), "{uri}");
    }
    // The server-rendered report (PDF bytes, so call the router directly).
    let before = c.audit_rows("session.read").await.len();
    let req = common::auth::empty(Method::GET, "/v1/sessions/S-1/report.pdf", Some(&s.token));
    let (st, _, _) = raw(c.app.clone(), req).await;
    assert_eq!(st, StatusCode::OK);
    let rows = c.audit_rows("session.read").await;
    assert_eq!(rows.len(), before + 1);
    assert_eq!(rows.last().unwrap()["after"], json!({"part": "report"}));

    // 404s (unknown, other org) write nothing.
    let other = common::auth::seed_org(&c.db.pool, "Clinic B").await;
    let (_, other_tok) = c.token_in(Some(other), Role::Clinician).await;
    let n = c.audit_rows("session.read").await.len();
    let (st, _) = c
        .call(Method::GET, "/v1/sessions/S-1", None, &other_tok)
        .await;
    assert_eq!(st, StatusCode::NOT_FOUND);
    let (st, _) = get(&s, "/v1/sessions/NOPE").await;
    assert_eq!(st, StatusCode::NOT_FOUND);
    assert_eq!(c.audit_rows("session.read").await.len(), n);
    s.c.db.cleanup().await;
}

#[tokio::test]
async fn participant_timeline_disclosures_worklist_rows() {
    let Some(s) = seed().await else { return };
    let c = &s.c;
    let p = s.participant.to_string();

    let (st, b) = get(&s, &format!("/v1/participants/{p}")).await;
    assert_eq!(st, StatusCode::OK, "{b}");
    let r = one_row(c, "participant.read", "participant", &p).await;
    assert_eq!(r["actor_id"], s.clin.to_string());

    let (st, b) = get(&s, &format!("/v1/participants/{p}/timeline")).await;
    assert_eq!(st, StatusCode::OK, "{b}");
    one_row(c, "participant.timeline_read", "participant", &p).await;

    let (st, b) = get(&s, &format!("/v1/participants/{p}/disclosures")).await;
    assert_eq!(st, StatusCode::OK, "{b}");
    one_row(c, "disclosures.read", "participant", &p).await;

    let (st, b) = get(&s, &format!("/v1/orgs/{}/worklist", c.org)).await;
    assert_eq!(st, StatusCode::OK, "{b}");
    let r = one_row(c, "worklist.read", "organization", &c.org.to_string()).await;
    assert_eq!(r["after"]["session_ids"], json!(["S-1"]));
    s.c.db.cleanup().await;
}

#[tokio::test]
async fn review_reads_write_rows() {
    let Some(s) = seed().await else { return };
    let c = &s.c;
    let (st, claimed) = c
        .call(
            Method::POST,
            "/v1/sessions/S-1/clinical-review",
            Some(json!({})),
            &s.token,
        )
        .await;
    assert!(st.is_success(), "{claimed}");
    let id = claimed["id"].as_str().unwrap().to_owned();
    assert!(c.audit_rows("clinical_review.read").await.is_empty());

    let (st, _) = get(&s, "/v1/sessions/S-1/clinical-review").await;
    assert_eq!(st, StatusCode::OK);
    let (st, _) = get(&s, &format!("/v1/clinical-reviews/{id}")).await;
    assert_eq!(st, StatusCode::OK);
    let rows = c.audit_rows("clinical_review.read").await;
    assert_eq!(rows.len(), 2);
    for r in rows {
        assert_eq!(r["target_type"], "clinical_review");
        assert_eq!(r["target_id"], id);
        assert_eq!(r["actor_id"], s.clin.to_string());
    }
    s.c.db.cleanup().await;
}

#[tokio::test]
async fn note_reads_single_and_per_page() {
    let Some(s) = seed().await else { return };
    let c = &s.c;
    let p = s.participant;
    let mut ids = Vec::new();
    for text in ["first secret text", "second secret text"] {
        let (st, n) = c
            .call(
                Method::POST,
                &format!("/v1/participants/{p}/notes"),
                Some(json!({ "body": text })),
                &s.token,
            )
            .await;
        assert_eq!(st, StatusCode::CREATED, "{n}");
        ids.push(n["id"].as_str().unwrap().to_owned());
    }
    assert!(c.audit_rows("note.read").await.is_empty());

    let (st, _) = get(&s, &format!("/v1/notes/{}", ids[0])).await;
    assert_eq!(st, StatusCode::OK);
    one_row(c, "note.read", "clinical_note", &ids[0]).await;

    let (st, page) = get(&s, &format!("/v1/participants/{p}/notes")).await;
    assert_eq!(st, StatusCode::OK, "{page}");
    let rows = c.audit_rows("note.read").await;
    assert_eq!(rows.len(), 2, "one for the single read, one for the page");
    let listed = &rows[1];
    assert_eq!(listed["target_type"], "participant");
    assert_eq!(listed["target_id"], p.to_string());
    let mut got: Vec<String> = listed["after"]["note_ids"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_owned())
        .collect();
    got.sort();
    ids.sort();
    assert_eq!(got, ids);
    assert!(!c.audit_dump().await.contains("secret text"));
    s.c.db.cleanup().await;
}

#[tokio::test]
async fn export_file_served_is_audited() {
    let Some(s) = seed().await else { return };
    let c = &s.c;
    let (researcher_id, researcher) = c.token(Role::Researcher).await;
    let done = c
        .export_and_run(&researcher, body("csv", "deidentified", "research"))
        .await;
    let export = done["id"].as_str().unwrap().to_owned();
    let (st, url) = c
        .call(
            Method::POST,
            &format!("/v1/exports/{export}/download-url"),
            None,
            &researcher,
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{url}");
    let link = url["url"].as_str().unwrap();
    // A refused token writes nothing.
    let (st, _, _) = raw(
        c.app.clone(),
        common::get(&format!("/v1/exports/{export}/file?t=bad")),
    )
    .await;
    assert_eq!(st, StatusCode::FORBIDDEN);
    assert!(c.audit_rows("export.file_served").await.is_empty());

    let (st, _, _) = raw(c.app.clone(), common::get(link)).await;
    assert_eq!(st, StatusCode::OK);
    let r = one_row(c, "export.file_served", "export", &export).await;
    // The requester bound into the download token, not `system` (wave 1 defect 2).
    assert_eq!(r["actor_type"], "user");
    assert_eq!(r["actor_id"], researcher_id.to_string());
    assert!(!c.audit_dump().await.contains("?t="));
    s.c.db.cleanup().await;
}

#[tokio::test]
async fn superadmin_reads_of_allowed_data_are_audited() {
    let Some(s) = seed().await else { return };
    let c = &s.c;
    let (root, root_tok) = c.token(Role::Superadmin).await;
    let (st, b) = c
        .call(Method::GET, "/v1/sessions/S-1", None, &root_tok)
        .await;
    assert_eq!(st, StatusCode::OK, "{b}");
    let r = one_row(c, "session.read", "session", "S-1").await;
    assert_eq!(r["actor_id"], root.to_string());
    let (st, _) = c
        .call(
            Method::GET,
            &format!("/v1/participants/{}", s.participant),
            None,
            &root_tok,
        )
        .await;
    assert_eq!(st, StatusCode::OK);
    one_row(
        c,
        "participant.read",
        "participant",
        &s.participant.to_string(),
    )
    .await;
    s.c.db.cleanup().await;
}

/// A failing audit insert is a 500 and the body carries none of the data.
#[tokio::test]
async fn failing_audit_insert_fails_the_request_with_no_body() {
    let Some(s) = seed().await else { return };
    let c = &s.c;
    sqlx::query(
        "CREATE FUNCTION refuse_read_audit() RETURNS trigger LANGUAGE plpgsql AS
         $$ BEGIN RAISE EXCEPTION 'audit refused'; END $$",
    )
    .execute(&c.db.pool)
    .await
    .unwrap();
    sqlx::query(
        "CREATE TRIGGER refuse_read_audit BEFORE INSERT ON audit_log
         FOR EACH ROW WHEN (NEW.action LIKE '%.read' OR NEW.action = 'participant.timeline_read'
                            OR NEW.action = 'worklist.read')
         EXECUTE FUNCTION refuse_read_audit()",
    )
    .execute(&c.db.pool)
    .await
    .unwrap();
    let p = s.participant;
    let (_, note) = (
        0,
        c.call(
            Method::POST,
            &format!("/v1/participants/{p}/notes"),
            Some(json!({"body": "classified words"})),
            &s.token,
        )
        .await
        .1,
    );
    let note_id = note["id"].as_str().unwrap().to_owned();
    for uri in [
        "/v1/sessions/S-1".to_owned(),
        "/v1/sessions/S-1/clips".to_owned(),
        "/v1/sessions/S-1/fhir".to_owned(),
        format!("/v1/participants/{p}"),
        format!("/v1/participants/{p}/timeline"),
        format!("/v1/participants/{p}/notes"),
        format!("/v1/notes/{note_id}"),
        format!("/v1/orgs/{}/worklist", c.org),
    ] {
        let (st, b) = get(&s, &uri).await;
        assert_eq!(st, StatusCode::INTERNAL_SERVER_ERROR, "{uri} {b}");
        let text = b.to_string();
        assert!(
            !text.contains("S-1") && !text.contains("classified") && !text.contains("P-1"),
            "{uri}: {text}"
        );
    }
    assert!(c.audit_rows("session.read").await.is_empty());
    s.c.db.cleanup().await;
}
