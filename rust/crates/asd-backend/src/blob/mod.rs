//! owner: WP-B.1
//!
//! Blob storage for encrypted frame logs and device-generated report PDFs (02 ADR-B3).
//! [`S3BlobStore`] talks to AWS S3 or MinIO; [`MemoryBlobStore`] backs tests and quick
//! local runs. Keys follow `sessions/{org_id}/{participant_token}/{session_id}/frames.jsonl.enc`
//! and `reports/...`; callers build them, this module only checks they are well formed.

mod memory;
mod s3;

use std::time::Duration;

use async_trait::async_trait;
use bytes::Bytes;
use chrono::{DateTime, Utc};

pub use memory::MemoryBlobStore;
pub use s3::S3BlobStore;

/// Pre-signed PUT lifetime (02 §7: 15 minutes).
pub const PRESIGN_TTL: Duration = Duration::from_secs(15 * 60);
/// Largest blob accepted (02 §7: 50 MB).
pub const MAX_BLOB_BYTES: usize = 50 * 1024 * 1024;

/// An absolute upload URL the device PUTs to directly (CON-18 `presigned` mode).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresignedPut {
    pub url: String,
    pub method: &'static str,
    /// Headers the client must send unchanged (lower-case names), e.g. `content-type`.
    pub headers: Vec<(String, String)>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredBlob {
    pub bytes: Bytes,
    pub content_type: String,
}

#[derive(Debug, thiserror::Error)]
pub enum BlobError {
    #[error("invalid blob key")]
    InvalidKey,
    #[error("blob is {size} bytes, limit is {limit}")]
    TooLarge { size: usize, limit: usize },
    /// The store cannot be reached; the message is for logs only.
    #[error("blob store unavailable: {0}")]
    Unavailable(String),
    #[error("blob store error: {0}")]
    Other(String),
}

#[async_trait]
pub trait BlobStore: Send + Sync + 'static {
    /// Pre-signs a PUT for `key` bound to `content_type`, valid for [`PRESIGN_TTL`].
    async fn presign_put(&self, key: &str, content_type: &str) -> Result<PresignedPut, BlobError>;
    /// Stores `body` under `key` (proxy upload mode); at most [`MAX_BLOB_BYTES`].
    async fn put(&self, key: &str, content_type: &str, body: Bytes) -> Result<(), BlobError>;
    /// Reads a blob; `Ok(None)` when absent.
    async fn get(&self, key: &str) -> Result<Option<StoredBlob>, BlobError>;
    /// Cheap reachability check for `GET /health/ready`.
    async fn ready(&self) -> Result<(), BlobError>;
}

/// Keys are relative, `/`-separated, without empty, `.` or `..` segments.
pub fn validate_key(key: &str) -> Result<(), BlobError> {
    let ok = !key.is_empty()
        && key.len() <= 1024
        && !key.starts_with('/')
        && key
            .split('/')
            .all(|seg| !seg.is_empty() && seg != "." && seg != "..")
        && !key.chars().any(|c| c.is_control() || c == '\\');
    if ok {
        Ok(())
    } else {
        Err(BlobError::InvalidKey)
    }
}

pub(crate) fn check_size(len: usize) -> Result<(), BlobError> {
    if len > MAX_BLOB_BYTES {
        Err(BlobError::TooLarge {
            size: len,
            limit: MAX_BLOB_BYTES,
        })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_validation() {
        for good in [
            "a",
            "sessions/o/child-1/S-1/frames.jsonl.enc",
            "reports/x/report.pdf",
        ] {
            assert!(validate_key(good).is_ok(), "{good}");
        }
        for bad in ["", "/abs", "a//b", "a/../b", "./a", "a/", "a\\b", "a\nb"] {
            assert!(validate_key(bad).is_err(), "{bad:?}");
        }
        assert!(validate_key(&"k".repeat(1025)).is_err());
    }

    #[test]
    fn size_cap() {
        assert!(check_size(MAX_BLOB_BYTES).is_ok());
        assert!(matches!(
            check_size(MAX_BLOB_BYTES + 1),
            Err(BlobError::TooLarge { .. })
        ));
    }
}
