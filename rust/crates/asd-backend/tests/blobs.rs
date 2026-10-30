//! owner: WP-B.3
//!
//! CON-18 blob uploads. `BLOB_MODE=proxy`: `PUT /v1/sessions/{id}/blobs/{frames|report}`
//! under the device bearer stores the blob at the ADR-B3 key; `GET /v1/sessions/{id}/report.pdf`
//! (console) serves the device-uploaded PDF or 404. `BLOB_MODE=presigned`: the proxy route
//! does not exist (404) and `POST /v1/sessions` hands out absolute URLs (`sessions.rs`).

mod common;
mod ingest_support;

use std::sync::Arc;

use asd_backend::blob::{BlobStore, MemoryBlobStore, MAX_BLOB_BYTES};
use asd_backend::config::BlobMode;
use axum::body::Body;
use common::{send, TestDb};
use http::StatusCode;
use ingest_support::*;
use uuid::Uuid;

fn put(device: Option<&str>, uri: &str, content_type: &str, body: Vec<u8>) -> http::Request<Body> {
    let mut b = http::Request::builder()
        .method("PUT")
        .uri(uri)
        .header("content-type", content_type);
    if let Some(d) = device {
        b = b.header(http::header::AUTHORIZATION, bearer(d));
    }
    b.body(Body::from(body)).unwrap()
}

struct Fixture {
    db: TestDb,
    org: Uuid,
    token: String,
    store: Arc<MemoryBlobStore>,
    router: axum::Router,
}

impl Fixture {
    async fn new(mode: BlobMode) -> Option<Self> {
        let db = TestDb::migrated().await?;
        let org = seed_quiet_org(&db.pool).await;
        let (_, token) = seed_device(&db.pool, org).await;
        let store = Arc::new(MemoryBlobStore::new());
        let mut cfg = config(false);
        cfg.blob_mode = mode;
        let router = app(state_with(db.pool.clone(), store.clone(), cfg));
        Some(Self {
            db,
            org,
            token,
            store,
            router,
        })
    }
    /// The device's bearer token.
    fn dev(&self) -> String {
        self.token.clone()
    }
}

#[tokio::test]
async fn upload_without_device_identity_is_401() {
    let (status, _, body) = send(
        app(common::state_without_db()),
        put(
            None,
            "/v1/sessions/S/blobs/frames",
            "application/octet-stream",
            vec![1],
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(problem_slug(&body), "unauthorized");
}

#[tokio::test]
async fn proxy_upload_stores_frames_at_the_adr_b3_key() {
    let Some(f) = Fixture::new(BlobMode::Proxy).await else {
        return;
    };
    ingest_ok(&f.router, &f.dev(), &golden()).await;
    let (status, _, _) = send_raw(
        f.router.clone(),
        put(
            Some(&f.dev()),
            "/v1/sessions/SESS-GOLDEN-0001/blobs/frames",
            "application/octet-stream",
            b"encrypted-frames".to_vec(),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let key = format!(
        "sessions/{}/child-SESS-GOL/SESS-GOLDEN-0001/frames.jsonl.enc",
        f.org
    );
    let blob = f.store.get(&key).await.unwrap().expect("stored");
    assert_eq!(&blob.bytes[..], b"encrypted-frames");
    assert_eq!(blob.content_type, "application/octet-stream");
    let stored: Option<String> = sqlx::query_scalar("SELECT frames_blob_key FROM sessions")
        .fetch_one(&f.db.pool)
        .await
        .unwrap();
    assert_eq!(stored, Some(key));
    f.db.cleanup().await;
}

#[tokio::test]
async fn proxy_report_upload_is_served_to_the_console() {
    let Some(f) = Fixture::new(BlobMode::Proxy).await else {
        return;
    };
    ingest_ok(&f.router, &f.dev(), &golden()).await;
    let (_, uh) = seed_user(&f.db.pool, Some(f.org), "clinician").await;

    // Before the upload: 404 and report_pdf_available false.
    let (status, _, body) = send(
        f.router.clone(),
        user_get(&uh, "/v1/sessions/SESS-GOLDEN-0001/report.pdf"),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let (_, _, detail) = send(
        f.router.clone(),
        user_get(&uh, "/v1/sessions/SESS-GOLDEN-0001"),
    )
    .await;
    assert_eq!(detail["report_pdf_available"], false);

    let pdf = b"%PDF-1.7 test".to_vec();
    let (status, _, _) = send_raw(
        f.router.clone(),
        put(
            Some(&f.dev()),
            "/v1/sessions/SESS-GOLDEN-0001/blobs/report",
            "application/pdf",
            pdf.clone(),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, headers, bytes) = send_raw(
        f.router.clone(),
        user_get(&uh, "/v1/sessions/SESS-GOLDEN-0001/report.pdf"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(common::content_type(&headers), "application/pdf");
    assert_eq!(bytes, pdf);
    let (_, _, detail) = send(
        f.router.clone(),
        user_get(&uh, "/v1/sessions/SESS-GOLDEN-0001"),
    )
    .await;
    assert_eq!(detail["report_pdf_available"], true);

    // Another org's user gets 404, not the PDF.
    let other_org = seed_quiet_org(&f.db.pool).await;
    let (_, other) = seed_user(&f.db.pool, Some(other_org), "owner").await;
    let (status, _, _) = send_raw(
        f.router.clone(),
        user_get(&other, "/v1/sessions/SESS-GOLDEN-0001/report.pdf"),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    f.db.cleanup().await;
}

#[tokio::test]
async fn report_upload_must_be_a_pdf() {
    let Some(f) = Fixture::new(BlobMode::Proxy).await else {
        return;
    };
    ingest_ok(&f.router, &f.dev(), &golden()).await;
    let (status, _, body) = send(
        f.router.clone(),
        put(
            Some(&f.dev()),
            "/v1/sessions/SESS-GOLDEN-0001/blobs/report",
            "text/html",
            b"<html>".to_vec(),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(problem_slug(&body), "validation-failed");
    assert!(f.store.is_empty());
    f.db.cleanup().await;
}

#[tokio::test]
async fn upload_for_unknown_kind_or_session_or_other_org_is_404() {
    let Some(f) = Fixture::new(BlobMode::Proxy).await else {
        return;
    };
    ingest_ok(&f.router, &f.dev(), &golden()).await;
    let other_org = seed_quiet_org(&f.db.pool).await;
    let (_, other_dev) = seed_device(&f.db.pool, other_org).await;
    for (dev, uri) in [
        (f.dev(), "/v1/sessions/SESS-GOLDEN-0001/blobs/video"),
        (f.dev(), "/v1/sessions/NOPE/blobs/frames"),
        (other_dev, "/v1/sessions/SESS-GOLDEN-0001/blobs/frames"),
    ] {
        let (status, _, body) = send(
            f.router.clone(),
            put(Some(&dev), uri, "application/octet-stream", vec![1, 2, 3]),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{uri}: {body}");
        assert_eq!(problem_slug(&body), "not-found");
    }
    assert!(f.store.is_empty());
    f.db.cleanup().await;
}

#[tokio::test]
async fn upload_over_the_limit_is_413_problem() {
    let Some(f) = Fixture::new(BlobMode::Proxy).await else {
        return;
    };
    ingest_ok(&f.router, &f.dev(), &golden()).await;
    let (status, headers, body) = send(
        f.router.clone(),
        put(
            Some(&f.dev()),
            "/v1/sessions/SESS-GOLDEN-0001/blobs/frames",
            "application/octet-stream",
            vec![0u8; MAX_BLOB_BYTES + 1],
        ),
    )
    .await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(common::content_type(&headers), "application/problem+json");
    assert_eq!(problem_slug(&body), "payload-too-large");
    assert!(f.store.is_empty());
    f.db.cleanup().await;
}

#[tokio::test]
async fn proxy_route_does_not_exist_in_presigned_mode() {
    let Some(f) = Fixture::new(BlobMode::Presigned).await else {
        return;
    };
    ingest_ok(&f.router, &f.dev(), &golden()).await;
    let (status, _, body) = send(
        f.router.clone(),
        put(
            Some(&f.dev()),
            "/v1/sessions/SESS-GOLDEN-0001/blobs/frames",
            "application/octet-stream",
            vec![1],
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert!(f.store.is_empty());
    f.db.cleanup().await;
}

#[tokio::test]
async fn presigned_report_is_served_once_the_device_put_it() {
    let Some(f) = Fixture::new(BlobMode::Presigned).await else {
        return;
    };
    ingest_ok(&f.router, &f.dev(), &golden()).await;
    let (_, uh) = seed_user(&f.db.pool, Some(f.org), "viewer").await;
    // Upload completion is not observed in presigned mode: availability is unknown (null).
    let (_, _, detail) = send(
        f.router.clone(),
        user_get(&uh, "/v1/sessions/SESS-GOLDEN-0001"),
    )
    .await;
    assert_eq!(detail["report_pdf_available"], serde_json::Value::Null);

    let key = format!(
        "reports/{}/child-SESS-GOL/SESS-GOLDEN-0001/report.pdf",
        f.org
    );
    f.store
        .put(&key, "application/pdf", bytes::Bytes::from_static(b"%PDF"))
        .await
        .unwrap();
    let (status, _, bytes) = send_raw(
        f.router.clone(),
        user_get(&uh, "/v1/sessions/SESS-GOLDEN-0001/report.pdf"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(bytes, b"%PDF");
    f.db.cleanup().await;
}
