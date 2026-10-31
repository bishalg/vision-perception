//! owner: WP-B.1
//!
//! [`BlobStore`] on S3 (AWS) or MinIO (compose). Credentials come from the default AWS
//! provider chain; nothing secret is held in [`crate::config::Config`].

use async_trait::async_trait;
use aws_sdk_s3::config::{BehaviorVersion, Credentials, Region};
use aws_sdk_s3::error::{DisplayErrorContext, SdkError};
use std::time::Duration;

use aws_sdk_s3::presigning::PresigningConfig;
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::types::{CompletedMultipartUpload, CompletedPart};
use aws_sdk_s3::Client;
use bytes::Bytes;
use chrono::Utc;

use super::{
    check_large_size, check_presign_get, check_size, validate_key, BlobError, BlobStore,
    PresignedGet, PresignedPut, StoredBlob, PRESIGN_TTL,
};
use crate::config::S3Settings;

#[derive(Debug, Clone)]
pub struct S3BlobStore {
    client: Client,
    bucket: String,
}

impl S3BlobStore {
    /// Builds a client from the default AWS chain (env, profile, ECS/EC2 role).
    pub async fn from_env(settings: &S3Settings) -> Self {
        let shared = aws_config::defaults(BehaviorVersion::latest())
            .region(Region::new(settings.region.clone()))
            .load()
            .await;
        let mut builder =
            aws_sdk_s3::config::Builder::from(&shared).force_path_style(settings.force_path_style);
        if let Some(endpoint) = &settings.endpoint_url {
            builder = builder.endpoint_url(endpoint);
        }
        Self::from_client(Client::from_conf(builder.build()), &settings.bucket)
    }

    /// Builds a client with fixed credentials (tests, tooling).
    pub fn with_static_credentials(
        settings: &S3Settings,
        access_key: &str,
        secret_key: &str,
    ) -> Self {
        let mut builder = aws_sdk_s3::config::Builder::new()
            .behavior_version(BehaviorVersion::latest())
            .region(Region::new(settings.region.clone()))
            .credentials_provider(Credentials::new(
                access_key, secret_key, None, None, "static",
            ))
            .force_path_style(settings.force_path_style);
        if let Some(endpoint) = &settings.endpoint_url {
            builder = builder.endpoint_url(endpoint);
        }
        Self::from_client(Client::from_conf(builder.build()), &settings.bucket)
    }

    pub fn from_client(client: Client, bucket: &str) -> Self {
        Self {
            client,
            bucket: bucket.to_owned(),
        }
    }
}

fn sdk_error<E, R>(err: SdkError<E, R>) -> BlobError
where
    E: std::error::Error + 'static,
    R: std::fmt::Debug,
{
    let message = DisplayErrorContext(&err).to_string();
    match err {
        SdkError::DispatchFailure(_) | SdkError::TimeoutError(_) => BlobError::Unavailable(message),
        _ => BlobError::Other(message),
    }
}

#[async_trait]
impl BlobStore for S3BlobStore {
    async fn presign_put(&self, key: &str, content_type: &str) -> Result<PresignedPut, BlobError> {
        validate_key(key)?;
        let presigning = PresigningConfig::expires_in(PRESIGN_TTL)
            .map_err(|e| BlobError::Other(e.to_string()))?;
        let expires_at = Utc::now()
            + chrono::Duration::from_std(PRESIGN_TTL).expect("15 minutes fits in chrono");
        let request = self
            .client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .content_type(content_type)
            .presigned(presigning)
            .await
            .map_err(sdk_error)?;
        Ok(PresignedPut {
            url: request.uri().to_owned(),
            method: "PUT",
            headers: request
                .headers()
                .map(|(k, v)| (k.to_ascii_lowercase(), v.to_owned()))
                .collect(),
            expires_at,
        })
    }

    async fn put(&self, key: &str, content_type: &str, body: Bytes) -> Result<(), BlobError> {
        validate_key(key)?;
        check_size(body.len())?;
        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .content_type(content_type)
            .body(ByteStream::from(body))
            .send()
            .await
            .map_err(sdk_error)?;
        Ok(())
    }

    async fn get(&self, key: &str) -> Result<Option<StoredBlob>, BlobError> {
        validate_key(key)?;
        let output = match self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
        {
            Ok(output) => output,
            Err(err) if err.as_service_error().is_some_and(|e| e.is_no_such_key()) => {
                return Ok(None)
            }
            Err(err) => return Err(sdk_error(err)),
        };
        let content_type = output
            .content_type()
            .unwrap_or("application/octet-stream")
            .to_owned();
        let bytes = output
            .body
            .collect()
            .await
            .map_err(|e| BlobError::Other(e.to_string()))?
            .into_bytes();
        Ok(Some(StoredBlob {
            bytes,
            content_type,
        }))
    }

    async fn ready(&self) -> Result<(), BlobError> {
        self.client
            .head_bucket()
            .bucket(&self.bucket)
            .send()
            .await
            .map_err(sdk_error)?;
        Ok(())
    }

    async fn presign_get(
        &self,
        key: &str,
        ttl: Duration,
        file_name: &str,
    ) -> Result<PresignedGet, BlobError> {
        check_presign_get(key, ttl, file_name)?;
        let presigning =
            PresigningConfig::expires_in(ttl).map_err(|e| BlobError::Other(e.to_string()))?;
        let expires_at = Utc::now() + chrono::Duration::from_std(ttl).expect("ttl is capped");
        let request = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .response_content_disposition(format!("attachment; filename=\"{file_name}\""))
            .response_cache_control("no-store")
            .presigned(presigning)
            .await
            .map_err(sdk_error)?;
        Ok(PresignedGet {
            url: request.uri().to_owned(),
            expires_at,
        })
    }

    async fn delete(&self, key: &str) -> Result<(), BlobError> {
        validate_key(key)?;
        // S3 answers 204 for an absent key too.
        self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .map_err(sdk_error)?;
        Ok(())
    }

    /// One `PutObject` up to [`MULTIPART_THRESHOLD`], a multipart upload above it (parts of
    /// [`PART_SIZE`]); a failed multipart upload is aborted so no parts are left billed.
    async fn put_object_large(
        &self,
        key: &str,
        content_type: &str,
        body: Bytes,
    ) -> Result<(), BlobError> {
        validate_key(key)?;
        check_large_size(body.len())?;
        if body.len() <= MULTIPART_THRESHOLD {
            self.client
                .put_object()
                .bucket(&self.bucket)
                .key(key)
                .content_type(content_type)
                .body(ByteStream::from(body))
                .send()
                .await
                .map_err(sdk_error)?;
            return Ok(());
        }
        let upload = self
            .client
            .create_multipart_upload()
            .bucket(&self.bucket)
            .key(key)
            .content_type(content_type)
            .send()
            .await
            .map_err(sdk_error)?;
        let upload_id = upload
            .upload_id()
            .ok_or_else(|| BlobError::Other("multipart upload without an id".into()))?
            .to_owned();
        match self.upload_parts(key, &upload_id, &body).await {
            Ok(parts) => {
                self.client
                    .complete_multipart_upload()
                    .bucket(&self.bucket)
                    .key(key)
                    .upload_id(&upload_id)
                    .multipart_upload(
                        CompletedMultipartUpload::builder()
                            .set_parts(Some(parts))
                            .build(),
                    )
                    .send()
                    .await
                    .map_err(sdk_error)?;
                Ok(())
            }
            Err(e) => {
                let _ = self
                    .client
                    .abort_multipart_upload()
                    .bucket(&self.bucket)
                    .key(key)
                    .upload_id(&upload_id)
                    .send()
                    .await;
                Err(e)
            }
        }
    }
}

/// Above this size `put_object_large` switches to a multipart upload.
const MULTIPART_THRESHOLD: usize = 32 * 1024 * 1024;
/// Multipart part size (S3 minimum is 5 MiB except for the last part).
const PART_SIZE: usize = 16 * 1024 * 1024;

impl S3BlobStore {
    async fn upload_parts(
        &self,
        key: &str,
        upload_id: &str,
        body: &Bytes,
    ) -> Result<Vec<CompletedPart>, BlobError> {
        let mut parts = Vec::new();
        for (i, start) in (0..body.len()).step_by(PART_SIZE).enumerate() {
            let end = (start + PART_SIZE).min(body.len());
            let number = i32::try_from(i + 1).expect("at most 32 parts of 16 MiB in 500 MB");
            let part = self
                .client
                .upload_part()
                .bucket(&self.bucket)
                .key(key)
                .upload_id(upload_id)
                .part_number(number)
                .body(ByteStream::from(body.slice(start..end)))
                .send()
                .await
                .map_err(sdk_error)?;
            parts.push(
                CompletedPart::builder()
                    .part_number(number)
                    .set_e_tag(part.e_tag().map(str::to_owned))
                    .build(),
            );
        }
        Ok(parts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minio() -> S3BlobStore {
        S3BlobStore::with_static_credentials(
            &S3Settings {
                bucket: "curv-asd-sessions".into(),
                region: "us-east-1".into(),
                endpoint_url: Some("https://minio.example.org".into()),
                force_path_style: true,
            },
            "AKIDEXAMPLE",
            "secret",
        )
    }

    // Presigning is local computation; no network is touched.
    #[tokio::test]
    async fn presigned_put_is_bound_to_key_content_type_and_ttl() {
        let put = minio()
            .presign_put(
                "sessions/o/child-1/S-1/frames.jsonl.enc",
                "application/octet-stream",
            )
            .await
            .unwrap();
        assert!(
            put.url.starts_with(
                "https://minio.example.org/curv-asd-sessions/sessions/o/child-1/S-1/frames.jsonl.enc?"
            ),
            "{}",
            put.url
        );
        assert!(put.url.contains("X-Amz-Expires=900"), "{}", put.url);
        assert!(
            put.url.contains("content-type"),
            "content-type is signed: {}",
            put.url
        );
        assert!(!put.url.contains("secret"));
        assert_eq!(put.method, "PUT");
        assert!(put
            .headers
            .iter()
            .any(|(k, v)| k == "content-type" && v == "application/octet-stream"));
    }

    #[tokio::test]
    async fn presigned_get_is_short_lived_and_an_attachment() {
        let get = minio()
            .presign_get(
                "exports/o/e/export-e-csv.zip",
                Duration::from_secs(300),
                "export-e-csv.zip",
            )
            .await
            .unwrap();
        assert!(
            get.url.starts_with(
                "https://minio.example.org/curv-asd-sessions/exports/o/e/export-e-csv.zip?"
            ),
            "{:?}",
            get.url
        );
        assert!(get.url.contains("X-Amz-Expires=300"));
        assert!(get.url.contains("response-content-disposition=attachment"));
        assert!(!get.url.contains("secret"));
        let ttl = (get.expires_at - Utc::now()).num_seconds();
        assert!((295..=300).contains(&ttl), "{ttl}");
        assert!(minio()
            .presign_get("k", Duration::from_secs(3600), "a.zip")
            .await
            .is_err());
    }

    #[tokio::test]
    async fn presign_rejects_bad_keys_before_signing() {
        assert!(matches!(
            minio().presign_put("../etc", "text/plain").await,
            Err(BlobError::InvalidKey)
        ));
    }
}
