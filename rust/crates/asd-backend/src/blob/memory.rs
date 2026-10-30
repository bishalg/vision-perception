//! owner: WP-B.1
//!
//! In-process [`BlobStore`] for tests (QA layer 2) and `ASD_BLOB_BACKEND=memory`.

use std::collections::HashMap;
use std::sync::Mutex;

use async_trait::async_trait;
use bytes::Bytes;
use chrono::Utc;

use super::{
    check_size, validate_key, BlobError, BlobStore, PresignedPut, StoredBlob, PRESIGN_TTL,
};

/// Host used in pre-signed URLs; `.test` is reserved (RFC 2606), so nothing resolves it.
pub const MEMORY_PRESIGN_BASE: &str = "https://blobs.memory.test/";

#[derive(Debug, Default)]
pub struct MemoryBlobStore {
    blobs: Mutex<HashMap<String, StoredBlob>>,
}

impl MemoryBlobStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of stored blobs (test helper).
    pub fn len(&self) -> usize {
        self.blobs.lock().expect("blob map poisoned").len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[async_trait]
impl BlobStore for MemoryBlobStore {
    async fn presign_put(&self, key: &str, content_type: &str) -> Result<PresignedPut, BlobError> {
        validate_key(key)?;
        let expires_at = Utc::now()
            + chrono::Duration::from_std(PRESIGN_TTL).expect("15 minutes fits in chrono");
        Ok(PresignedPut {
            url: format!(
                "{MEMORY_PRESIGN_BASE}{key}?expires={}",
                expires_at.timestamp()
            ),
            method: "PUT",
            headers: vec![("content-type".to_owned(), content_type.to_owned())],
            expires_at,
        })
    }

    async fn put(&self, key: &str, content_type: &str, body: Bytes) -> Result<(), BlobError> {
        validate_key(key)?;
        check_size(body.len())?;
        self.blobs.lock().expect("blob map poisoned").insert(
            key.to_owned(),
            StoredBlob {
                bytes: body,
                content_type: content_type.to_owned(),
            },
        );
        Ok(())
    }

    async fn get(&self, key: &str) -> Result<Option<StoredBlob>, BlobError> {
        validate_key(key)?;
        Ok(self
            .blobs
            .lock()
            .expect("blob map poisoned")
            .get(key)
            .cloned())
    }

    async fn ready(&self) -> Result<(), BlobError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn put_overwrites_and_rejects_bad_keys() {
        let s = MemoryBlobStore::new();
        s.put("a/b", "text/plain", Bytes::from_static(b"1"))
            .await
            .unwrap();
        s.put("a/b", "text/plain", Bytes::from_static(b"2"))
            .await
            .unwrap();
        assert_eq!(s.len(), 1);
        assert_eq!(
            s.get("a/b").await.unwrap().unwrap().bytes,
            Bytes::from_static(b"2")
        );
        assert!(matches!(
            s.put("../x", "text/plain", Bytes::new()).await,
            Err(BlobError::InvalidKey)
        ));
        assert!(matches!(
            s.presign_put("/x", "text/plain").await,
            Err(BlobError::InvalidKey)
        ));
    }

    #[tokio::test]
    async fn presigned_url_expires_in_fifteen_minutes() {
        let put = MemoryBlobStore::new()
            .presign_put("k", "application/pdf")
            .await
            .unwrap();
        let ttl = (put.expires_at - Utc::now()).num_seconds();
        assert!((895..=900).contains(&ttl), "{ttl}");
        assert!(put.url.starts_with(MEMORY_PRESIGN_BASE));
    }
}
