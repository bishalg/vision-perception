//! owner: WP-I.6
//!
//! Blob store additions (G4, EXJ-5, EXJ-9, trap 17) through the public trait: pre-signed
//! GETs are short-lived attachments, delete is idempotent, large artifacts bypass the
//! 50 MB device cap but not the 500 MB export cap. Stores that predate the methods refuse
//! them instead of misbehaving. No database needed.

use std::time::Duration;

use asd_backend::blob::{
    BlobError, BlobStore, MemoryBlobStore, PresignedPut, S3BlobStore, StoredBlob, MAX_BLOB_BYTES,
    MAX_LARGE_BLOB_BYTES, MAX_PRESIGN_GET_TTL,
};
use asd_backend::config::S3Settings;
use async_trait::async_trait;
use bytes::Bytes;
use chrono::Utc;

#[tokio::test]
async fn memory_store_round_trip() {
    let s: Box<dyn BlobStore> = Box::new(MemoryBlobStore::new());
    let body = Bytes::from(vec![1u8; MAX_BLOB_BYTES + 10]);
    assert!(matches!(
        s.put("exports/o/e/a.zip", "application/zip", body.clone())
            .await,
        Err(BlobError::TooLarge { .. })
    ));
    s.put_object_large("exports/o/e/a.zip", "application/zip", body.clone())
        .await
        .unwrap();
    assert_eq!(
        s.get("exports/o/e/a.zip").await.unwrap().unwrap().bytes,
        body
    );
    let url = s
        .presign_get("exports/o/e/a.zip", Duration::from_secs(300), "a.zip")
        .await
        .unwrap();
    assert!((url.expires_at - Utc::now()).num_seconds() <= 300);
    assert!(s
        .presign_get("exports/o/e/a.zip", MAX_PRESIGN_GET_TTL * 2, "a.zip")
        .await
        .is_err());
    assert!(s
        .presign_get("exports/o/e/a.zip", Duration::from_secs(300), "../a.zip")
        .await
        .is_err());
    s.delete("exports/o/e/a.zip").await.unwrap();
    s.delete("exports/o/e/a.zip").await.unwrap();
    assert!(s.get("exports/o/e/a.zip").await.unwrap().is_none());
    const { assert!(MAX_LARGE_BLOB_BYTES > MAX_BLOB_BYTES) };
}

#[tokio::test]
async fn s3_presigned_get_is_signed_locally() {
    let s3 = S3BlobStore::with_static_credentials(
        &S3Settings {
            bucket: "curv-asd-sessions".into(),
            region: "us-east-1".into(),
            endpoint_url: Some("https://minio.example.org".into()),
            force_path_style: true,
        },
        "AKIDEXAMPLE",
        "secret",
    );
    let url = s3
        .presign_get("exports/o/e/a.zip", Duration::from_secs(300), "a.zip")
        .await
        .unwrap();
    assert!(url.url.contains("X-Amz-Expires=300"), "{:?}", url);
    assert!(url.url.contains("X-Amz-Signature="));
}

struct OldStore;

#[async_trait]
impl BlobStore for OldStore {
    async fn presign_put(&self, _: &str, _: &str) -> Result<PresignedPut, BlobError> {
        Err(BlobError::Other("n/a".into()))
    }
    async fn put(&self, _: &str, _: &str, _: Bytes) -> Result<(), BlobError> {
        Ok(())
    }
    async fn get(&self, _: &str) -> Result<Option<StoredBlob>, BlobError> {
        Ok(None)
    }
    async fn ready(&self) -> Result<(), BlobError> {
        Ok(())
    }
}

#[tokio::test]
async fn stores_without_the_new_methods_refuse_them() {
    let s = OldStore;
    assert!(s
        .presign_get("k", Duration::from_secs(60), "a")
        .await
        .is_err());
    assert!(s.delete("k").await.is_err());
    // `put_object_large` falls back to `put` (and its cap).
    s.put_object_large("k", "text/plain", Bytes::new())
        .await
        .unwrap();
}
