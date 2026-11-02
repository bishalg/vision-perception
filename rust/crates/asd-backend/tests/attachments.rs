//! owner: WP-B.19
//!
//! Attachments (REC-6, REC-7, HIP-5; acceptance 4): metadata, then the bytes through the
//! encrypting proxy PUT with an optional RFC 9530 `Content-Digest`; the blob store holds
//! ciphertext that decrypts to the upload; reads need `ClinicalContentRead` and are
//! audited; size and type limits; delete; erasure removes attachments and their blobs
//! unless the participant is held. Needs `DATABASE_URL`.

mod common;
mod participant_support;

use asd_backend::crypto::Aad;
use asd_backend::jobs::{runner, JobContext};
use asd_contract::api_v1::Role;
use axum::body::Body;
use base64::Engine as _;
use http::{HeaderMap, Method, Request, StatusCode};
use http_body_util::BodyExt;
use participant_support::*;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tower::ServiceExt;
use uuid::Uuid;

/// A small "JPEG": the magic number and a recognisable marker.
fn jpeg(len: usize) -> Vec<u8> {
    let mut v = vec![0xFF, 0xD8, 0xFF, 0xE0];
    v.extend_from_slice(b"SECRET-SCAN-BYTES");
    v.resize(len.max(v.len()), 0x42);
    v
}

fn digest(bytes: &[u8]) -> String {
    format!(
        "sha-256=:{}:",
        base64::engine::general_purpose::STANDARD.encode(Sha256::digest(bytes))
    )
}

async fn raw(c: &Ctx, req: Request<Body>) -> (StatusCode, HeaderMap, Vec<u8>) {
    let resp = c.app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let headers = resp.headers().clone();
    let bytes = resp
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes()
        .to_vec();
    (status, headers, bytes)
}

async fn put_content(
    c: &Ctx,
    url: &str,
    bytes: Vec<u8>,
    digest_header: Option<&str>,
    bearer: &str,
) -> (StatusCode, Value) {
    let mut b = Request::builder()
        .method(Method::PUT)
        .uri(url)
        .header("content-type", "image/jpeg")
        .header("authorization", format!("Bearer {bearer}"));
    if let Some(d) = digest_header {
        b = b.header("content-digest", d);
    }
    let (s, _, body) = raw(c, b.body(Body::from(bytes)).unwrap()).await;
    let v = if body.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&body).unwrap_or(Value::Null)
    };
    (s, v)
}

async fn create(c: &Ctx, p: Uuid, bearer: &str, body: Value) -> (StatusCode, Value) {
    c.call(
        Method::POST,
        &format!("/v1/participants/{p}/attachments"),
        Some(body),
        bearer,
    )
    .await
}

fn meta(size: usize) -> Value {
    json!({"kind": "paper_form", "file_name": "Consent form (Sita).jpg",
           "media_type": "image/jpeg", "byte_size": size})
}

/// Creates and uploads a JPEG of `len` bytes; returns the attachment id.
async fn upload(c: &Ctx, p: Uuid, bearer: &str, len: usize) -> String {
    let (s, made) = create(c, p, bearer, meta(len)).await;
    assert_eq!(s, StatusCode::CREATED, "{made}");
    let bytes = jpeg(len);
    let (s, b) = put_content(
        c,
        made["upload"]["url"].as_str().unwrap(),
        bytes.clone(),
        Some(&digest(&bytes)),
        bearer,
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{b}");
    made["attachment"]["id"].as_str().unwrap().to_owned()
}

#[tokio::test]
async fn upload_is_encrypted_at_rest_and_reads_are_audited() {
    let Some(c) = Ctx::new().await else { return };
    let p = c.participant(c.org, "child-dddddddddddddddddddd").await;
    let (_, admin) = c.token(Role::Admin).await;
    let (_, clin) = c.token(Role::Clinician).await;
    let size = 2 * 1024 * 1024;

    // Acceptance 4: front desk uploads a 2 MB scan with Content-Digest.
    let (s, made) = create(&c, p, &admin, meta(size)).await;
    assert_eq!(s, StatusCode::CREATED, "{made}");
    let att = &made["attachment"];
    assert_eq!(att["file_name"], "Consent_form__Sita_.jpg", "sanitised");
    assert_eq!(att["sha256"], Value::Null);
    let id: Uuid = att["id"].as_str().unwrap().parse().unwrap();
    assert_eq!(made["upload"]["method"], "PUT");
    let url = made["upload"]["url"].as_str().unwrap().to_owned();
    assert_eq!(url, format!("/v1/attachments/{id}/content"));
    assert!(
        !c.audit_dump().await.contains("Sita"),
        "no file name in audit"
    );

    let bytes = jpeg(size);
    // Tampered body (one byte differs from what the digest covers): digest-mismatch.
    let mut tampered = bytes.clone();
    tampered[100] ^= 0x01;
    let (s, b) = put_content(&c, &url, tampered, Some(&digest(&bytes)), &admin).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{b}");
    assert_eq!(slug_of(&b), "digest-mismatch");
    // Malformed digest (no colons): validation-failed, through the shared helper.
    let (s, b) = put_content(&c, &url, bytes.clone(), Some("sha-256=abc"), &admin).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{b}");
    assert_eq!(slug_of(&b), "validation-failed");
    // Nothing stored yet.
    let key = format!("attachments/{}/{p}/{id}", c.org);
    assert!(c.state.blobs.get(&key).await.unwrap().is_none());

    let (s, done) = put_content(&c, &url, bytes.clone(), Some(&digest(&bytes)), &admin).await;
    assert_eq!(s, StatusCode::OK, "{done}");
    assert_eq!(
        done["sha256"],
        Sha256::digest(&bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    );
    assert_eq!(done["byte_size"], size);
    // The content is written once.
    let (s, b) = put_content(&c, &url, bytes.clone(), None, &admin).await;
    assert_eq!(s, StatusCode::CONFLICT, "{b}");

    // At rest: ciphertext under an id-only key, which decrypts to the upload.
    let (stored_key, dek): (String, Uuid) =
        sqlx::query_as("SELECT blob_key, dek_key_id FROM attachments WHERE id = $1")
            .bind(id)
            .fetch_one(&c.db.pool)
            .await
            .unwrap();
    assert_eq!(stored_key, key);
    let stored = c.state.blobs.get(&key).await.unwrap().unwrap();
    assert_ne!(stored.bytes.as_ref(), bytes.as_slice());
    assert!(!stored.bytes.starts_with(&[0xFF, 0xD8, 0xFF]));
    assert!(!String::from_utf8_lossy(&stored.bytes).contains("SECRET-SCAN"));
    let opened = c
        .state
        .keyring
        .clone()
        .unwrap()
        .decrypt(
            c.org,
            &Aad::new("attachments", "blob", c.org, id),
            dek,
            &stored.bytes,
        )
        .await
        .unwrap();
    assert_eq!(opened, bytes);

    // Reads: clinician gets the image, audited; admin and superadmin 403.
    let get = |t: &str| {
        Request::builder()
            .uri(format!("/v1/attachments/{id}"))
            .header("authorization", format!("Bearer {t}"))
            .body(Body::empty())
            .unwrap()
    };
    let (s, h, body) = raw(&c, get(&clin)).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(body, bytes);
    assert_eq!(h["content-type"], "image/jpeg");
    assert_eq!(h["cache-control"], "no-store");
    assert!(h["content-disposition"]
        .to_str()
        .unwrap()
        .contains("Consent_form__Sita_.jpg"));
    let reads = c.audit_rows("attachment.read").await;
    assert_eq!(reads.len(), 1);
    assert_eq!(reads[0]["target_id"], id.to_string());
    let (_, root) = c.token(Role::Superadmin).await;
    for t in [&admin, &root] {
        let (s, _, _) = raw(&c, get(t)).await;
        assert_eq!(s, StatusCode::FORBIDDEN);
    }
    assert_eq!(c.audit_rows("attachment.read").await.len(), 1);

    // Metadata list (any reader) and the timeline header.
    let (_, viewer) = c.token(Role::Viewer).await;
    let (s, list) = c
        .call(
            Method::GET,
            &format!("/v1/participants/{p}/attachments"),
            None,
            &viewer,
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{list}");
    assert_eq!(list["total"], 1);
    assert_eq!(list["items"][0]["id"], id.to_string());
    let (s, tl) = c
        .call(
            Method::GET,
            &format!("/v1/participants/{p}/timeline?kind=attachment"),
            None,
            &viewer,
        )
        .await;
    assert_eq!(s, StatusCode::OK, "{tl}");
    assert_eq!(tl["items"][0]["kind"], "attachment");
    assert_eq!(tl["items"][0]["summary"]["attachment_kind"], "paper_form");
    assert_eq!(tl["items"][0]["summary"]["byte_size"], size);

    // Delete: the blob goes, reads are 404, the list is empty.
    let (s, _) = c
        .call(
            Method::DELETE,
            &format!("/v1/attachments/{id}"),
            None,
            &admin,
        )
        .await;
    assert_eq!(s, StatusCode::NO_CONTENT);
    assert!(c.state.blobs.get(&key).await.unwrap().is_none());
    let (s, _, _) = raw(&c, get(&clin)).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    assert_eq!(c.audit_rows("attachment.delete").await.len(), 1);
    let (_, list) = c
        .call(
            Method::GET,
            &format!("/v1/participants/{p}/attachments"),
            None,
            &viewer,
        )
        .await;
    assert_eq!(list["total"], 0);
    c.db.cleanup().await;
}

#[tokio::test]
async fn limits_types_and_roles() {
    let Some(c) = Ctx::new().await else { return };
    let p = c.participant(c.org, "child-eeeeeeeeeeeeeeeeeeee").await;
    let (_, clin) = c.token(Role::Clinician).await;

    // Metadata: 12 MB, 0 bytes and GIF are refused with their slugs.
    for (body, slug) in [
        (meta(12 * 1024 * 1024), "attachment-too-large"),
        (meta(0), "attachment-too-large"),
        (
            json!({"kind": "report", "file_name": "x.gif", "media_type": "image/gif",
                   "byte_size": 10}),
            "attachment-type-not-allowed",
        ),
    ] {
        let (s, b) = create(&c, p, &clin, body).await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{b}");
        assert_eq!(slug_of(&b), slug);
    }
    // Content: over 10 MiB is attachment-too-large; bytes that are not a JPEG are refused.
    let (_, made) = create(&c, p, &clin, meta(1000)).await;
    let url = made["upload"]["url"].as_str().unwrap().to_owned();
    let (s, b) = put_content(&c, &url, jpeg(10 * 1024 * 1024 + 1), None, &clin).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{b}");
    assert_eq!(slug_of(&b), "attachment-too-large");
    let (s, b) = put_content(&c, &url, b"%PDF-1.7 not a jpeg".to_vec(), None, &clin).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{b}");
    assert_eq!(slug_of(&b), "attachment-type-not-allowed");
    let (s, b) = put_content(&c, &url, Vec::new(), None, &clin).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{b}");
    assert_eq!(slug_of(&b), "attachment-too-large");
    // No digest (older clients): accepted.
    let (s, b) = put_content(&c, &url, jpeg(1000), None, &clin).await;
    assert_eq!(s, StatusCode::OK, "{b}");

    // Writes: viewer, researcher, reviewer and superadmin are 403 (RBAC-3 for superadmin).
    for role in [
        Role::Viewer,
        Role::Researcher,
        Role::Reviewer,
        Role::Superadmin,
    ] {
        let (_, t) = c.token(role).await;
        let (s, _) = create(&c, p, &t, meta(10)).await;
        assert_eq!(s, StatusCode::FORBIDDEN, "{role:?}");
        let (s, _) = put_content(&c, &url, jpeg(10), None, &t).await;
        assert_eq!(s, StatusCode::FORBIDDEN, "{role:?}");
    }
    // Owner and admin may write.
    for role in [Role::Owner, Role::Admin] {
        let (_, t) = c.token(role).await;
        let (s, b) = create(&c, p, &t, meta(10)).await;
        assert_eq!(s, StatusCode::CREATED, "{role:?}: {b}");
    }
    // Another org: 404 on the participant and the attachment.
    let other = common::auth::seed_org(&c.db.pool, "Clinic B").await;
    let (_, b_clin) = c.token_in(Some(other), Role::Clinician).await;
    let (s, _) = create(&c, p, &b_clin, meta(10)).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let id = made["attachment"]["id"].as_str().unwrap();
    let (s, _) = c
        .call(Method::GET, &format!("/v1/attachments/{id}"), None, &b_clin)
        .await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    // An encounter of another participant cannot be named.
    let q = c.participant(c.org, "child-ffffffffffffffffffff").await;
    let (_, enc) = c
        .call(
            Method::POST,
            &format!("/v1/participants/{q}/encounters"),
            Some(json!({"kind": "other", "occurred_at": "2026-11-01T00:00:00Z"})),
            &clin,
        )
        .await;
    let mut wrong = meta(10);
    wrong["encounter_id"] = enc["id"].clone();
    let (s, b) = create(&c, p, &clin, wrong).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{b}");
    // Withdrawn: no new attachments.
    sqlx::query("UPDATE participants SET withdrawn_at = now() WHERE id = $1")
        .bind(p)
        .execute(&c.db.pool)
        .await
        .unwrap();
    let (s, b) = create(&c, p, &clin, meta(10)).await;
    assert_eq!(s, StatusCode::CONFLICT, "{b}");
    assert_eq!(slug_of(&b), "participant-withdrawn");
    c.db.cleanup().await;
}

#[tokio::test]
async fn tampered_ciphertext_is_a_500_without_plaintext() {
    let Some(c) = Ctx::new().await else { return };
    let p = c.participant(c.org, "child-abababababababababab").await;
    let (_, clin) = c.token(Role::Clinician).await;
    let a = upload(&c, p, &clin, 500).await;
    let b = upload(&c, p, &clin, 600).await;
    // Copy a's ciphertext over b's object: the AAD binds it to a's id.
    let key = |id: &str| format!("attachments/{}/{p}/{id}", c.org);
    let a_bytes = c.state.blobs.get(&key(&a)).await.unwrap().unwrap().bytes;
    c.state
        .blobs
        .put(&key(&b), "application/octet-stream", a_bytes)
        .await
        .unwrap();
    let (s, body) = c
        .call(Method::GET, &format!("/v1/attachments/{b}"), None, &clin)
        .await;
    assert_eq!(s, StatusCode::INTERNAL_SERVER_ERROR, "{body}");
    assert!(!body.to_string().contains("SECRET"));
    c.db.cleanup().await;
}

async fn run_erasure(c: &Ctx, p: Uuid, bearer: &str) -> Value {
    let (s, b) = c
        .call(
            Method::POST,
            &format!("/v1/orgs/{}/participants/{p}/erasure", c.org),
            None,
            bearer,
        )
        .await;
    assert_eq!(s, StatusCode::ACCEPTED, "{b}");
    runner::run_pending_in(&JobContext::from_state(&c.state), "test-worker")
        .await
        .unwrap();
    sqlx::query_scalar(
        "SELECT to_jsonb(r) FROM erasure_requests r WHERE participant_id = $1
         ORDER BY requested_at DESC LIMIT 1",
    )
    .bind(p)
    .fetch_one(&c.db.pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn erasure_removes_attachments_and_blobs_unless_held() {
    let Some(c) = Ctx::new().await else { return };
    let (_, admin) = c.token(Role::Admin).await;
    let (clin_id, clin) = c.token(Role::Clinician).await;

    // Erased: two uploaded attachments (one deleted already), one metadata only, and an
    // encounter without a summary.
    let p = c.participant(c.org, "child-0a0a0a0a0a0a0a0a0a0a").await;
    let a1 = upload(&c, p, &admin, 100).await;
    let a2 = upload(&c, p, &admin, 200).await;
    let (_, _) = create(&c, p, &admin, meta(300)).await;
    c.call(
        Method::DELETE,
        &format!("/v1/attachments/{a2}"),
        None,
        &admin,
    )
    .await;
    c.call(
        Method::POST,
        &format!("/v1/participants/{p}/encounters"),
        Some(json!({"kind": "other", "occurred_at": "2026-11-01T00:00:00Z"})),
        &admin,
    )
    .await;
    let key1 = format!("attachments/{}/{p}/{a1}", c.org);
    assert!(c.state.blobs.get(&key1).await.unwrap().is_some());

    let req = run_erasure(&c, p, &admin).await;
    assert_eq!(req["status"], "completed", "{req}");
    assert_eq!(req["outcome"]["attachments_deleted"], 3);
    assert!(c.state.blobs.get(&key1).await.unwrap().is_none());
    let left: i64 = sqlx::query_scalar(
        "SELECT (SELECT count(*) FROM attachments WHERE participant_id = $1)
              + (SELECT count(*) FROM encounters WHERE participant_id = $1)",
    )
    .bind(p)
    .fetch_one(&c.db.pool)
    .await
    .unwrap();
    assert_eq!(left, 0);

    // Held: a clinical note keeps the record, the attachments and their blobs.
    let h = c.participant(c.org, "child-0b0b0b0b0b0b0b0b0b0b").await;
    let kept = upload(&c, h, &admin, 100).await;
    let (s, b) = c
        .call(
            Method::POST,
            &format!("/v1/participants/{h}/notes"),
            Some(json!({"body": "clinical observation"})),
            &clin,
        )
        .await;
    assert_eq!(s, StatusCode::CREATED, "{b}");
    let req = run_erasure(&c, h, &admin).await;
    assert_eq!(req["status"], "held", "{req}");
    assert_eq!(req["outcome"]["attachments_deleted"], 0);
    let kept_key = format!("attachments/{}/{h}/{kept}", c.org);
    assert!(c.state.blobs.get(&kept_key).await.unwrap().is_some());

    // Held too: an encounter with a clinician's summary (the same kind of record).
    let e = c.participant(c.org, "child-0c0c0c0c0c0c0c0c0c0c").await;
    let kept_e = upload(&c, e, &admin, 100).await;
    let (s, b) = c
        .call(
            Method::POST,
            &format!("/v1/participants/{e}/encounters"),
            Some(
                json!({"kind": "other", "occurred_at": "2026-11-01T00:00:00Z",
                        "summary": "clinical text", "clinician_id": clin_id.to_string()}),
            ),
            &clin,
        )
        .await;
    assert_eq!(s, StatusCode::CREATED, "{b}");
    let req = run_erasure(&c, e, &admin).await;
    assert_eq!(req["status"], "held", "{req}");
    let kept_key = format!("attachments/{}/{e}/{kept_e}", c.org);
    assert!(c.state.blobs.get(&kept_key).await.unwrap().is_some());
    c.db.cleanup().await;
}
