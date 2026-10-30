//! owner: WP-B.1
//!
//! CON-22: `GET /health` (liveness, no dependencies) and `GET /health/ready` (database,
//! migrations, blob store). No `/v1/health`. Errors are RFC 7807 problem+json.

mod common;

use std::sync::Arc;

use asd_backend::blob::{BlobError, BlobStore, MemoryBlobStore, PresignedPut, StoredBlob};
use async_trait::async_trait;
use bytes::Bytes;
use common::*;
use http::StatusCode;

const PROBLEM_BASE: &str = "https://schemas.curv.org/asd/problems/";

#[tokio::test]
async fn health_liveness_is_ok_without_database() {
    let (status, headers, body) = send(router(state_without_db()), get("/health")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(content_type(&headers).starts_with("application/json"));
    assert_eq!(body["status"], "ok");
    assert_eq!(body["service"], "asd-backend");
    assert_eq!(body["version"], env!("CARGO_PKG_VERSION"));
}

#[tokio::test]
async fn health_ready_is_503_problem_when_database_unreachable() {
    let (status, headers, body) = send(router(state_without_db()), get("/health/ready")).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(content_type(&headers), "application/problem+json");
    assert_eq!(body["type"], format!("{PROBLEM_BASE}not-ready"));
    assert_eq!(body["status"], 503);
    assert_eq!(body["checks"]["database"], "unavailable");
    assert_eq!(body["checks"]["blob_store"], "ok");
}

struct DownBlobStore;

#[async_trait]
impl BlobStore for DownBlobStore {
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
async fn health_ready_reports_blob_store_failure() {
    let state = state_with_blobs_without_db(Arc::new(DownBlobStore));
    let (status, _, body) = send(router(state), get("/health/ready")).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["checks"]["blob_store"], "unavailable");
    // The failure detail is not echoed to the caller.
    assert!(!body.to_string().contains("down"));
}

#[tokio::test]
async fn v1_health_is_not_routed() {
    let (status, headers, body) = send(router(state_without_db()), get("/v1/health")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(content_type(&headers), "application/problem+json");
    assert_eq!(body["type"], format!("{PROBLEM_BASE}not-found"));
}

#[tokio::test]
async fn unknown_route_is_problem_json_404() {
    let (status, headers, body) = send(router(state_without_db()), get("/nope")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(content_type(&headers), "application/problem+json");
    assert_eq!(body["status"], 404);
    assert_eq!(body["title"], "Not Found");
}

#[tokio::test]
async fn wrong_method_is_problem_json_405() {
    let request = http::Request::post("/health")
        .body(axum::body::Body::empty())
        .unwrap();
    let (status, headers, body) = send(router(state_without_db()), request).await;
    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(content_type(&headers), "application/problem+json");
    assert_eq!(body["type"], format!("{PROBLEM_BASE}method-not-allowed"));
}

#[tokio::test]
async fn responses_carry_a_request_id() {
    let (_, headers, _) = send(router(state_without_db()), get("/health")).await;
    let id = headers.get("x-request-id").expect("x-request-id header");
    assert!(uuid::Uuid::parse_str(id.to_str().unwrap()).is_ok());
}

#[tokio::test]
async fn incoming_request_id_is_propagated() {
    let request = http::Request::get("/health")
        .header("x-request-id", "abc-123")
        .body(axum::body::Body::empty())
        .unwrap();
    let (_, headers, _) = send(router(state_without_db()), request).await;
    assert_eq!(headers.get("x-request-id").unwrap(), "abc-123");
}

#[tokio::test]
async fn health_ready_is_ok_with_migrated_database() {
    let Some(db) = TestDb::migrated().await else {
        return;
    };
    let (status, headers, body) = send(
        router(state_with_pool(db.pool.clone())),
        get("/health/ready"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(content_type(&headers).starts_with("application/json"));
    assert_eq!(body["status"], "ready");
    assert_eq!(body["checks"]["database"], "ok");
    assert_eq!(body["checks"]["migrations"], "ok");
    assert_eq!(body["checks"]["blob_store"], "ok");
    db.cleanup().await;
}

#[tokio::test]
async fn health_ready_is_503_when_migrations_have_not_run() {
    let Some(db) = TestDb::empty().await else {
        return;
    };
    let (status, _, body) = send(
        router(state_with_pool(db.pool.clone())),
        get("/health/ready"),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["checks"]["database"], "ok");
    assert_eq!(body["checks"]["migrations"], "pending");
    db.cleanup().await;
}

#[tokio::test]
async fn memory_blob_store_round_trips_and_presigns_https() {
    let store = MemoryBlobStore::new();
    store
        .put(
            "sessions/o/p/s/frames.jsonl.enc",
            "application/octet-stream",
            Bytes::from_static(b"abc"),
        )
        .await
        .unwrap();
    let blob = store
        .get("sessions/o/p/s/frames.jsonl.enc")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(blob.bytes, Bytes::from_static(b"abc"));
    assert_eq!(blob.content_type, "application/octet-stream");
    assert!(store.get("missing").await.unwrap().is_none());

    let put = store
        .presign_put("k/frames", "application/octet-stream")
        .await
        .unwrap();
    assert!(put.url.starts_with("https://"), "{}", put.url);
    assert_eq!(put.method, "PUT");
    assert!(put
        .headers
        .iter()
        .any(|(k, v)| k == "content-type" && v == "application/octet-stream"));
}

#[tokio::test]
async fn blob_put_over_the_size_cap_is_rejected() {
    let store = MemoryBlobStore::new();
    let too_big = Bytes::from(vec![0u8; asd_backend::blob::MAX_BLOB_BYTES + 1]);
    let err = store
        .put("k", "application/pdf", too_big)
        .await
        .unwrap_err();
    assert!(matches!(err, BlobError::TooLarge { .. }), "{err:?}");
}
