//! owner: WP-B.19
//!
//! Attachments (REC-6, REC-7, HIP-5): scanned paper forms, referrals and reports of a
//! participant.
//!
//! - `POST /v1/participants/{id}/attachments` (`AttachmentWrite`): metadata first; answers
//!   `201 AttachmentUpload` with `upload: {method: PUT, url: /v1/attachments/{id}/content}`.
//!   A media type other than PDF, JPEG or PNG is `400 attachment-type-not-allowed`; a size
//!   outside 1..10 MiB `400 attachment-too-large`. The file name is sanitised
//!   (`emr::sanitize_file_name`).
//! - `PUT /v1/attachments/{id}/content` (`AttachmentWrite`; always this proxy route, in
//!   either blob mode, because the bytes must pass through the encryptor): ≤ 10 MiB, `Content-Digest` verified over the plaintext
//!   when present (`400 digest-mismatch`), the bytes must look like the declared type, then
//!   they are encrypted with the org data key (AAD `attachments:blob:{org}:{id}`) and stored
//!   under `attachments/{org}/{participant}/{id}` (no file name in the key, trap 4). The
//!   content is written once; a second `PUT` is `409 conflict`.
//! - `GET /v1/attachments/{id}` (`ClinicalContentRead`): audits `attachment.read` first
//!   (HIP-1), then decrypts to memory and answers the bytes with `Cache-Control: no-store`.
//! - `GET /v1/participants/{id}/attachments` (`ParticipantRead`): metadata of the
//!   participant's attachments, newest first (no bytes; the timeline shows the same headers).
//! - `DELETE /v1/attachments/{id}` (`AttachmentWrite`): deletes the blob, then sets
//!   `deleted_at` (audited). A deleted attachment answers `404`.
//!
//! Writes about a participant who withdrew consent are `409 participant-withdrawn`.
//! [`load_content`] is the one decrypting reader; the case package (WP-I.13b) uses it too.

use asd_contract::emr::{
    sanitize_file_name, Attachment, AttachmentKind, AttachmentMediaType, AttachmentPage,
    AttachmentUpload, CreateAttachmentRequest, UploadTarget, MAX_ATTACHMENT_BYTES,
};
use asd_contract::problem::{ATTACHMENT_TOO_LARGE, ATTACHMENT_TYPE_NOT_ALLOWED, CONFLICT};
use axum::body::Bytes;
use axum::extract::rejection::{BytesRejection, QueryRejection};
use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use http::{header, HeaderMap, HeaderValue, StatusCode};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use crate::app::AppState;
use crate::audit::AuditEvent;
use crate::auth::extractors::{ApiJson, RequestMeta, Require};
use crate::auth::rbac::perm;
use crate::blob::BlobStore;
use crate::crypto::{Aad, ClinicalCrypto, Keyring};
use crate::error::{AppError, AppResult};
use crate::problem::InvalidParam;
use crate::read::cursor::{self, Cursor};
use crate::routes::participant_detail::{page_query, participant_org, PageQuery};
use crate::routes::sessions::path_uuid;

/// The stored object's content type: the bytes are ciphertext, whatever the file was.
const CIPHERTEXT_CONTENT_TYPE: &str = "application/octet-stream";

pub fn router() -> Router<AppState> {
    let limit = usize::try_from(MAX_ATTACHMENT_BYTES).unwrap_or(usize::MAX);
    Router::new()
        .route("/participants/{id}/attachments", post(create).get(list))
        .route("/attachments/{id}", get(download).delete(remove))
        .route(
            "/attachments/{id}/content",
            put(upload).layer(DefaultBodyLimit::max(limit)),
        )
}

fn invalid(name: &str, reason: &str) -> AppError {
    AppError::Validation(vec![InvalidParam {
        name: name.into(),
        reason: reason.into(),
    }])
}

fn too_large() -> AppError {
    AppError::BadRequest {
        slug: ATTACHMENT_TOO_LARGE,
        detail: format!("attachments are 1..{MAX_ATTACHMENT_BYTES} bytes"),
    }
}

fn type_not_allowed() -> AppError {
    AppError::BadRequest {
        slug: ATTACHMENT_TYPE_NOT_ALLOWED,
        detail: "attachments are PDF, JPEG or PNG".into(),
    }
}

/// The AAD of an attachment's stored bytes (KEY-3, REC-6).
fn blob_aad(org: Uuid, id: Uuid) -> Aad {
    Aad::new("attachments", "blob", org, id)
}

/// `attachments/{org}/{participant}/{id}`: ids only, never the file name (trap 4).
pub(crate) fn blob_key(org: Uuid, participant: Uuid, id: Uuid) -> String {
    format!("attachments/{org}/{participant}/{id}")
}

/// Whether `bytes` start like a file of `media_type` (magic numbers).
fn looks_like(media_type: AttachmentMediaType, bytes: &[u8]) -> bool {
    match media_type {
        AttachmentMediaType::ApplicationPdf => bytes.starts_with(b"%PDF-"),
        AttachmentMediaType::ImageJpeg => bytes.starts_with(&[0xFF, 0xD8, 0xFF]),
        AttachmentMediaType::ImagePng => bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
    }
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

// ---- the stored row ------------------------------------------------------------------------

pub(crate) struct AttachmentRow {
    pub id: Uuid,
    pub org_id: Uuid,
    pub participant_id: Uuid,
    pub encounter_id: Option<Uuid>,
    pub kind: String,
    pub file_name: String,
    pub media_type: String,
    pub byte_size: i32,
    pub sha256: Option<Vec<u8>>,
    pub blob_key: Option<String>,
    pub dek_key_id: Option<Uuid>,
    pub uploaded_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub withdrawn: bool,
}

impl AttachmentRow {
    pub(crate) fn media_type(&self) -> AppResult<AttachmentMediaType> {
        AttachmentMediaType::parse(&self.media_type)
            .ok_or_else(|| AppError::Internal("unknown attachment media type".into()))
    }

    pub(crate) fn dto(&self) -> AppResult<Attachment> {
        Ok(Attachment {
            id: self.id.to_string(),
            participant_id: self.participant_id.to_string(),
            encounter_id: self.encounter_id.map(|u| u.to_string()),
            kind: AttachmentKind::parse(&self.kind)
                .ok_or_else(|| AppError::Internal("unknown attachment kind".into()))?,
            file_name: self.file_name.clone(),
            media_type: self.media_type()?,
            byte_size: u64::try_from(self.byte_size).unwrap_or(0),
            sha256: self.sha256.as_deref().map(hex),
            uploaded_by: Some(self.uploaded_by.to_string()),
            created_at: cursor::ts(self.created_at),
        })
    }
}

/// The live (not deleted) attachment `id`, any org; the caller checks the org.
pub(crate) async fn attachment_by_id(db: &PgPool, id: Uuid) -> AppResult<Option<AttachmentRow>> {
    Ok(sqlx::query_as!(
        AttachmentRow,
        r#"SELECT a.id, a.org_id, a.participant_id, a.encounter_id, a.kind, a.file_name,
                  a.media_type, a.byte_size, a.sha256, a.blob_key, a.dek_key_id, a.uploaded_by,
                  a.created_at, p.withdrawn_at IS NOT NULL AS "withdrawn!"
           FROM attachments a JOIN participants p ON p.id = a.participant_id
           WHERE a.id = $1 AND a.deleted_at IS NULL"#,
        id
    )
    .fetch_optional(db)
    .await?)
}

/// An attachment's metadata and its decrypted bytes.
pub(crate) struct AttachmentContent {
    pub attachment: Attachment,
    pub bytes: Vec<u8>,
}

/// Loads and decrypts attachment `id` of `org` to memory (≤ 10 MiB, no temp files, trap 4).
/// `404` when it is unknown, of another org, deleted or has no content yet. The plaintext
/// is checked against the stored SHA-256; a mismatch or an AEAD failure (tampered or
/// copied ciphertext) is a `500`. The caller audits the read.
pub(crate) async fn load_content(
    db: &PgPool,
    blobs: &dyn BlobStore,
    keyring: &Keyring,
    org: Uuid,
    id: Uuid,
) -> AppResult<AttachmentContent> {
    let row = attachment_by_id(db, id)
        .await?
        .filter(|r| r.org_id == org)
        .ok_or(AppError::NotFound)?;
    let (Some(key), Some(dek), Some(sha)) = (&row.blob_key, row.dek_key_id, &row.sha256) else {
        return Err(AppError::NotFound);
    };
    let stored = blobs
        .get(key)
        .await?
        .ok_or_else(|| AppError::Internal("attachment blob missing".into()))?;
    let bytes = keyring
        .decrypt(org, &blob_aad(org, id), dek, &stored.bytes)
        .await?;
    if Sha256::digest(&bytes).as_slice() != sha.as_slice() {
        return Err(AppError::Internal(
            "attachment bytes do not match their digest".into(),
        ));
    }
    Ok(AttachmentContent {
        attachment: row.dto()?,
        bytes,
    })
}

// ---- POST /participants/{id}/attachments ----------------------------------------------------

/// The create body: the media type is checked before the typed parse so an unknown one is
/// `attachment-type-not-allowed`, not a generic validation error.
fn parse_create(body: Value) -> AppResult<CreateAttachmentRequest> {
    if let Some(media) = body.get("media_type").and_then(Value::as_str) {
        if AttachmentMediaType::parse(media).is_none() {
            return Err(type_not_allowed());
        }
    }
    if let Some(size) = body.get("byte_size") {
        match size.as_u64() {
            Some(n) if (1..=MAX_ATTACHMENT_BYTES).contains(&n) => {}
            Some(_) => return Err(too_large()),
            // Negative or zero-like numbers are out of range too; other types are malformed.
            None if size.as_i64().is_some() || size.as_f64().is_some() => return Err(too_large()),
            None => {}
        }
    }
    serde_json::from_value(body).map_err(|e| invalid("body", &e.to_string()))
}

async fn create(
    auth: Require<perm::AttachmentWrite>,
    State(state): State<AppState>,
    meta: RequestMeta,
    Path(raw_id): Path<String>,
    ApiJson(body): ApiJson<Value>,
) -> AppResult<(StatusCode, Json<AttachmentUpload>)> {
    let (participant, org) = participant_org(&state.db, &raw_id).await?;
    auth.ensure_org(org)?;
    // Without the KEK the bytes could not be stored; refuse before a row exists (KEY-2).
    ClinicalCrypto::from_state(&state)?;
    let req = parse_create(body)?;
    if req.file_name.trim().is_empty() {
        return Err(invalid("file_name", "must not be empty"));
    }
    let file_name = sanitize_file_name(req.file_name.trim());
    let byte_size = i32::try_from(req.byte_size).map_err(|_| too_large())?;
    let encounter = match req.encounter_id.as_deref().filter(|s| !s.is_empty()) {
        Some(raw) => {
            let id = Uuid::parse_str(raw).map_err(|_| invalid("encounter_id", "must be a UUID"))?;
            let ok = sqlx::query_scalar!(
                r#"SELECT EXISTS (SELECT 1 FROM encounters
                     WHERE id = $1 AND participant_id = $2 AND org_id = $3) AS "ok!""#,
                id,
                participant,
                org
            )
            .fetch_one(&state.db)
            .await?;
            if !ok {
                return Err(invalid(
                    "encounter_id",
                    "is not an encounter of this participant",
                ));
            }
            Some(id)
        }
        None => None,
    };

    let id = Uuid::new_v4();
    let mut tx = state.db.begin().await?;
    let withdrawn = sqlx::query_scalar!(
        r#"SELECT withdrawn_at IS NOT NULL AS "w!" FROM participants WHERE id = $1 FOR SHARE"#,
        participant
    )
    .fetch_one(&mut *tx)
    .await?;
    if withdrawn {
        return Err(crate::ingest::participant_withdrawn());
    }
    sqlx::query!(
        "INSERT INTO attachments
             (id, org_id, participant_id, encounter_id, kind, file_name, media_type, byte_size,
              uploaded_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
        id,
        org,
        participant,
        encounter,
        req.kind.as_str(),
        file_name,
        req.media_type.as_str(),
        byte_size,
        auth.principal.user_id,
    )
    .execute(&mut *tx)
    .await?;
    // The file name can carry PHI: the audit row names ids, kind, type and size only.
    AuditEvent::user(&auth.principal, "attachment.create", "attachment", id)
        .org(org)
        .after(json!({
            "participant_id": participant,
            "encounter_id": encounter,
            "kind": req.kind.as_str(),
            "media_type": req.media_type.as_str(),
            "byte_size": byte_size,
        }))
        .meta(&meta)
        .record(&mut tx)
        .await?;
    tx.commit().await?;

    let row = attachment_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::Internal("created attachment not found".into()))?;
    Ok((
        StatusCode::CREATED,
        Json(AttachmentUpload {
            attachment: row.dto()?,
            upload: UploadTarget {
                method: "PUT".into(),
                url: format!("/v1/attachments/{id}/content"),
            },
        }),
    ))
}

// ---- PUT /attachments/{id}/content ---------------------------------------------------------

async fn upload(
    auth: Require<perm::AttachmentWrite>,
    State(state): State<AppState>,
    meta: RequestMeta,
    Path(raw_id): Path<String>,
    headers: HeaderMap,
    body: Result<Bytes, BytesRejection>,
) -> AppResult<Json<Attachment>> {
    let id = path_uuid(&raw_id)?;
    let row = attachment_by_id(&state.db, id)
        .await?
        .ok_or(AppError::NotFound)?;
    auth.ensure_org(row.org_id)?;
    let crypto = ClinicalCrypto::from_state(&state)?;
    let body = body.map_err(|e| {
        if e.status() == StatusCode::PAYLOAD_TOO_LARGE {
            too_large()
        } else {
            invalid("body", "could not read the request body")
        }
    })?;
    if row.withdrawn {
        return Err(crate::ingest::participant_withdrawn());
    }
    if row.sha256.is_some() {
        return Err(AppError::Conflict {
            slug: CONFLICT,
            detail: "the attachment's content was already uploaded".into(),
        });
    }
    if body.is_empty() || body.len() as u64 > MAX_ATTACHMENT_BYTES {
        return Err(too_large());
    }
    // SYNC-6 / REC-7: over the plaintext the client sent, before anything is stored.
    crate::blob::verify_content_digest(&headers, &body)?;
    let media_type = row.media_type()?;
    if !looks_like(media_type, &body) {
        return Err(type_not_allowed());
    }

    let (org, participant) = (row.org_id, row.participant_id);
    let sha = Sha256::digest(&body).to_vec();
    let size = i32::try_from(body.len()).map_err(|_| too_large())?;
    // HIP-5: encrypted before the store ever sees the bytes.
    let sealed = crypto.encrypt(org, &blob_aad(org, id), &body).await?;
    drop(body);
    let key = blob_key(org, participant, id);
    state
        .blobs
        .put(
            &key,
            CIPHERTEXT_CONTENT_TYPE,
            Bytes::from(sealed.ciphertext),
        )
        .await?;

    let mut tx = state.db.begin().await?;
    let n = sqlx::query!(
        "UPDATE attachments
         SET sha256 = $2, blob_key = $3, dek_key_id = $4, byte_size = $5, content_at = now()
         WHERE id = $1 AND sha256 IS NULL AND deleted_at IS NULL",
        id,
        sha,
        key,
        sealed.key_id,
        size,
    )
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if n == 0 {
        // A concurrent upload or delete won; this request's object is the same key.
        return Err(AppError::Conflict {
            slug: CONFLICT,
            detail: "the attachment changed while uploading".into(),
        });
    }
    AuditEvent::user(&auth.principal, "attachment.upload", "attachment", id)
        .org(org)
        .after(json!({
            "participant_id": participant,
            "byte_size": size,
            "sha256": hex(&sha),
            "digest_header": headers.contains_key("content-digest"),
        }))
        .meta(&meta)
        .record(&mut tx)
        .await?;
    tx.commit().await?;

    let row = attachment_by_id(&state.db, id)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(Json(row.dto()?))
}

// ---- GET /attachments/{id} -------------------------------------------------------------------

async fn download(
    auth: Require<perm::ClinicalContentRead>,
    crypto: ClinicalCrypto,
    State(state): State<AppState>,
    meta: RequestMeta,
    Path(raw_id): Path<String>,
) -> AppResult<Response> {
    let id = path_uuid(&raw_id)?;
    let row = attachment_by_id(&state.db, id)
        .await?
        .ok_or(AppError::NotFound)?;
    auth.ensure_org(row.org_id)?;
    if row.sha256.is_none() {
        return Err(AppError::NotFound);
    }
    // HIP-1: the audit row exists before any byte is decrypted or sent (trap 10).
    crate::audit::record_read(
        &state.db,
        &auth.principal,
        &meta,
        row.org_id,
        "attachment.read",
        "attachment",
        &id.to_string(),
        Some(json!({ "participant_id": row.participant_id })),
    )
    .await?;
    let content = load_content(&state.db, &*state.blobs, &crypto, row.org_id, id).await?;
    let disposition = format!("attachment; filename=\"{}\"", content.attachment.file_name);
    let headers = [
        (
            header::CONTENT_TYPE,
            HeaderValue::from_static(content.attachment.media_type.as_str()),
        ),
        (
            header::CONTENT_DISPOSITION,
            HeaderValue::from_str(&disposition)
                .map_err(|_| AppError::Internal("file name is not a header value".into()))?,
        ),
        (header::CACHE_CONTROL, HeaderValue::from_static("no-store")),
        (
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        ),
    ];
    Ok((StatusCode::OK, headers, content.bytes).into_response())
}

// ---- GET /participants/{id}/attachments ------------------------------------------------------

async fn list(
    auth: Require<perm::ParticipantRead>,
    State(state): State<AppState>,
    Path(raw_id): Path<String>,
    query: Result<Query<PageQuery>, QueryRejection>,
) -> AppResult<Json<AttachmentPage>> {
    let (participant, org) = participant_org(&state.db, &raw_id).await?;
    auth.ensure_org(org)?;
    let (limit, after) = page_query(query)?.parse()?;
    let (after_at, after_id) = match after {
        Some(c) => {
            let id = Uuid::parse_str(&c.id)
                .map_err(|_| invalid("cursor", "not a cursor from this API"))?;
            (Some(c.at), Some(id))
        }
        None => (None, None),
    };
    let rows = sqlx::query_as!(
        AttachmentRow,
        r#"SELECT a.id, a.org_id, a.participant_id, a.encounter_id, a.kind, a.file_name,
                  a.media_type, a.byte_size, a.sha256, a.blob_key, a.dek_key_id, a.uploaded_by,
                  a.created_at, p.withdrawn_at IS NOT NULL AS "withdrawn!"
           FROM attachments a JOIN participants p ON p.id = a.participant_id
           WHERE a.participant_id = $1 AND a.org_id = $2 AND a.deleted_at IS NULL
             AND ($3::timestamptz IS NULL OR (a.created_at, a.id) < ($3, $4::uuid))
           ORDER BY a.created_at DESC, a.id DESC
           LIMIT $5"#,
        participant,
        org,
        after_at,
        after_id,
        limit + 1,
    )
    .fetch_all(&state.db)
    .await?;
    let total = sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!" FROM attachments
           WHERE participant_id = $1 AND org_id = $2 AND deleted_at IS NULL"#,
        participant,
        org
    )
    .fetch_one(&state.db)
    .await?;
    let limit = usize::try_from(limit).unwrap_or(usize::MAX);
    let has_more = rows.len() > limit;
    let page: Vec<&AttachmentRow> = rows.iter().take(limit).collect();
    let next_cursor = if has_more {
        page.last()
            .map(|r| Cursor::new(r.created_at, r.id.to_string()).encode())
    } else {
        None
    };
    Ok(Json(AttachmentPage {
        items: page
            .iter()
            .map(|r| r.dto())
            .collect::<AppResult<Vec<_>>>()?,
        next_cursor,
        total: Some(u64::try_from(total).unwrap_or(0)),
    }))
}

// ---- DELETE /attachments/{id} -----------------------------------------------------------------

async fn remove(
    auth: Require<perm::AttachmentWrite>,
    State(state): State<AppState>,
    meta: RequestMeta,
    Path(raw_id): Path<String>,
) -> AppResult<StatusCode> {
    let id = path_uuid(&raw_id)?;
    let row = attachment_by_id(&state.db, id)
        .await?
        .ok_or(AppError::NotFound)?;
    auth.ensure_org(row.org_id)?;
    // The blob first: a failure leaves the row live and the DELETE can be retried; deleting
    // an absent key succeeds, so a retry after the row update failed is harmless too.
    let key = row
        .blob_key
        .clone()
        .unwrap_or_else(|| blob_key(row.org_id, row.participant_id, id));
    state.blobs.delete(&key).await?;
    let mut tx = state.db.begin().await?;
    let n = sqlx::query!(
        "UPDATE attachments SET deleted_at = now() WHERE id = $1 AND deleted_at IS NULL",
        id
    )
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if n == 0 {
        return Err(AppError::NotFound);
    }
    AuditEvent::user(&auth.principal, "attachment.delete", "attachment", id)
        .org(row.org_id)
        .after(json!({
            "participant_id": row.participant_id,
            "had_content": row.sha256.is_some(),
        }))
        .meta(&meta)
        .record(&mut tx)
        .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn magic_numbers() {
        use AttachmentMediaType::*;
        assert!(looks_like(ApplicationPdf, b"%PDF-1.7\n"));
        assert!(looks_like(ImageJpeg, &[0xFF, 0xD8, 0xFF, 0xE0]));
        assert!(looks_like(ImagePng, b"\x89PNG\r\n\x1a\n...."));
        assert!(!looks_like(ImagePng, b"%PDF-1.7"));
        assert!(!looks_like(ApplicationPdf, b""));
    }

    #[test]
    fn create_body_checks_type_and_size_first() {
        let ok = json!({"kind": "referral", "file_name": "a.pdf",
                        "media_type": "application/pdf", "byte_size": 10});
        assert!(parse_create(ok).is_ok());
        let gif = json!({"kind": "referral", "file_name": "a.gif",
                         "media_type": "image/gif", "byte_size": 10});
        assert!(
            matches!(parse_create(gif), Err(AppError::BadRequest { slug, .. })
            if slug == ATTACHMENT_TYPE_NOT_ALLOWED)
        );
        for size in [json!(0), json!(-1), json!(MAX_ATTACHMENT_BYTES + 1)] {
            let big = json!({"kind": "referral", "file_name": "a.pdf",
                             "media_type": "application/pdf", "byte_size": size});
            assert!(
                matches!(parse_create(big), Err(AppError::BadRequest { slug, .. })
                if slug == ATTACHMENT_TOO_LARGE)
            );
        }
    }

    #[test]
    fn blob_key_has_ids_only() {
        let (o, p, a) = (Uuid::nil(), Uuid::nil(), Uuid::nil());
        let k = blob_key(o, p, a);
        assert!(k.starts_with("attachments/"));
        assert!(crate::blob::validate_key(&k).is_ok());
    }
}
