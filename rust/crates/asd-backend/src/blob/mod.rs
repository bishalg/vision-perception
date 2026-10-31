//! owner: WP-B.1
//!
//! Blob storage for encrypted frame logs and device-generated report PDFs (02 ADR-B3).
//! [`S3BlobStore`] talks to AWS S3 or MinIO; [`MemoryBlobStore`] backs tests and quick
//! local runs. Keys follow `sessions/{org_id}/{participant_token}/{session_id}/frames.jsonl.enc`
//! and `reports/...`; callers build them, this module only checks they are well formed.
//!
//! WP-I.6 (G4, EXJ-5, EXJ-9) adds [`BlobStore::presign_get`] (five-minute download URLs),
//! [`BlobStore::delete`] (artifact expiry, EXJ-4) and [`BlobStore::put_object_large`] for
//! export artifacts up to [`MAX_LARGE_BLOB_BYTES`]; `put` keeps its 50 MB cap, which also
//! guards device uploads (trap 17). The new methods have refusing defaults so stores that
//! predate them (test doubles) still compile; both real stores implement them.

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
/// Largest export artifact (EXJ-9: 500 MB), through [`BlobStore::put_object_large`] only.
pub const MAX_LARGE_BLOB_BYTES: usize = 500 * 1024 * 1024;
/// Longest pre-signed GET a caller may ask for (EXJ-5 uses 5 minutes, webhooks 15).
pub const MAX_PRESIGN_GET_TTL: Duration = Duration::from_secs(15 * 60);

/// An absolute upload URL the device PUTs to directly (CON-18 `presigned` mode).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresignedPut {
    pub url: String,
    pub method: &'static str,
    /// Headers the client must send unchanged (lower-case names), e.g. `content-type`.
    pub headers: Vec<(String, String)>,
    pub expires_at: DateTime<Utc>,
}

/// An absolute download URL (EXJ-5 `presigned` mode). The URL itself is the credential:
/// never log it.
#[derive(Clone, PartialEq, Eq)]
pub struct PresignedGet {
    pub url: String,
    pub expires_at: DateTime<Utc>,
}

impl std::fmt::Debug for PresignedGet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PresignedGet")
            .field("url", &"<redacted>")
            .field("expires_at", &self.expires_at)
            .finish()
    }
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

    /// Pre-signs a GET for `key`, valid for `ttl` (at most [`MAX_PRESIGN_GET_TTL`]). The
    /// response is served as an attachment named `file_name` (`[A-Za-z0-9._-]`).
    async fn presign_get(
        &self,
        key: &str,
        ttl: Duration,
        file_name: &str,
    ) -> Result<PresignedGet, BlobError> {
        let _ = (key, ttl, file_name);
        Err(BlobError::Other(
            "this blob store cannot pre-sign downloads".into(),
        ))
    }

    /// Deletes `key`; deleting an absent key succeeds (expiry sweeps are re-runnable).
    async fn delete(&self, key: &str) -> Result<(), BlobError> {
        let _ = key;
        Err(BlobError::Other("this blob store cannot delete".into()))
    }

    /// Stores an export artifact of up to [`MAX_LARGE_BLOB_BYTES`] (S3: multipart upload).
    /// Overwrites, so a re-run job with the same key replaces its artifact (EXJ-3).
    async fn put_object_large(
        &self,
        key: &str,
        content_type: &str,
        body: Bytes,
    ) -> Result<(), BlobError> {
        self.put(key, content_type, body).await
    }
}

/// `presign_get` arguments every store checks the same way.
pub(crate) fn check_presign_get(
    key: &str,
    ttl: Duration,
    file_name: &str,
) -> Result<(), BlobError> {
    validate_key(key)?;
    if ttl.is_zero() || ttl > MAX_PRESIGN_GET_TTL {
        return Err(BlobError::Other(format!(
            "presign_get ttl must be 1 s..{} s",
            MAX_PRESIGN_GET_TTL.as_secs()
        )));
    }
    let name_ok = !file_name.is_empty()
        && file_name.len() <= 200
        && file_name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
    if name_ok {
        Ok(())
    } else {
        Err(BlobError::Other(
            "download file names are [A-Za-z0-9._-]{1,200}".into(),
        ))
    }
}

pub(crate) fn check_large_size(len: usize) -> Result<(), BlobError> {
    if len > MAX_LARGE_BLOB_BYTES {
        Err(BlobError::TooLarge {
            size: len,
            limit: MAX_LARGE_BLOB_BYTES,
        })
    } else {
        Ok(())
    }
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
    fn presign_get_arguments() {
        let ttl = Duration::from_secs(300);
        assert!(check_presign_get("exports/o/e/a.zip", ttl, "export-1.zip").is_ok());
        assert!(check_presign_get("../x", ttl, "a.zip").is_err());
        assert!(check_presign_get("k", Duration::ZERO, "a.zip").is_err());
        let too_long = MAX_PRESIGN_GET_TTL + Duration::from_secs(1);
        assert!(check_presign_get("k", too_long, "a.zip").is_err());
        for bad in ["", "a b.zip", "a\"b", "../a", "a/b", "a;b"] {
            assert!(check_presign_get("k", ttl, bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn large_size_cap() {
        assert!(check_large_size(MAX_LARGE_BLOB_BYTES).is_ok());
        assert!(check_large_size(MAX_BLOB_BYTES + 1).is_ok());
        assert!(matches!(
            check_large_size(MAX_LARGE_BLOB_BYTES + 1),
            Err(BlobError::TooLarge { .. })
        ));
    }

    #[test]
    fn presigned_get_debug_hides_the_url() {
        let g = PresignedGet {
            url: "https://s3/x?X-Amz-Signature=abc".into(),
            expires_at: Utc::now(),
        };
        assert!(!format!("{g:?}").contains("Signature"));
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
