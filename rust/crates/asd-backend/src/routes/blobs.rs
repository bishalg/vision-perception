//! owner: WP-B.3
//!
//! Device blob uploads (CON-18, ADR-B3). `BLOB_MODE=presigned`: `POST /v1/sessions` hands
//! out absolute pre-signed PUT URLs (15 min, `Content-Type` bound, at most 50 MB) and this
//! router has no route. `BLOB_MODE=proxy`: the URLs are relative
//! `/v1/sessions/{id}/blobs/{frames|report}` and `PUT` here stores the bytes under the
//! device bearer. Keys: `sessions/{org}/{participant_token}/{session}/frames.jsonl.enc` and
//! `reports/{org}/{participant_token}/{session}/report.pdf`.
//!
//! The upload takes the WP-B.2 device bearer ([`DevicePrincipal`]); another org's session
//! is 404, a session of a participant who withdrew consent is `409 participant-withdrawn` (as ingest).
//!
//! ## Research frame log (WP-X.2, RFL-3)
//!
//! `PUT /v1/sessions/{id}/blobs/research_frames` (proxy mode only; presigned mode has no
//! route and the ack offers no URL, because the bytes must pass through the encryptor):
//! `Content-Type: application/x-ndjson`, at most 16 MiB, `Content-Digest` **required**
//! (`400 digest-required`, `400 digest-mismatch`), first line an `asd-raw-tracking/1`
//! [`RawLogHeader`] whose `sessionId` is the path (`400 validation-failed`), the session
//! research-consented (`rawPayload.normCollection.consented`, else `409
//! research-consent-required`) and its participant not withdrawn. The log is sealed under the
//! org data key ([`seal_research_log`]: AES-256-GCM, AAD
//! `sessions:research_frames:{org}:{session}`, the data key's id prefixed) before the blob
//! store sees it, stored at `.../research.jsonl.sealed`, and `research_frames_blob_key`,
//! `_bytes`, `_sha256` (of the plaintext) are recorded; audit `session.frames_upload` (device
//! actor, bytes and sha256). A re-PUT replaces the log. [`research_log_row`] and
//! [`read_research_log`] are the read side (`GET .../research-frames`, the analysis job).

use asd_core::types::tracking::{RawLogHeader, RAW_LOG_FORMAT};
use axum::body::Bytes;
use axum::extract::rejection::BytesRejection;
use axum::extract::{DefaultBodyLimit, Path, State};
use axum::routing::put;
use axum::Router;
use http::{header, HeaderMap, StatusCode};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use crate::app::AppState;
use crate::audit::AuditEvent;
use crate::auth::extractors::{DevicePrincipal, RequestMeta};
use crate::blob::{BlobStore, MAX_BLOB_BYTES};
use crate::config::BlobMode;
use crate::crypto::{Aad, ClinicalCrypto, Keyring};
use crate::error::{AppError, AppResult};
use crate::ingest::participant_withdrawn;
use crate::problem::InvalidParam;

pub const FRAMES_CONTENT_TYPE: &str = "application/octet-stream";
pub const REPORT_CONTENT_TYPE: &str = "application/pdf";
/// RFL-3: the research frame log is JSON Lines (`asd-raw-tracking/1`).
pub const RESEARCH_FRAMES_CONTENT_TYPE: &str = "application/x-ndjson";

/// The blobs a session may carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlobKind {
    Frames,
    Report,
    /// RFL-3. Accepted only by the sealed proxy upload ([`upload_research_log`]);
    /// [`BlobKind::parse`] does not return it, so the plain upload below never stores one.
    ResearchFrames,
}

impl BlobKind {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "frames" => Some(Self::Frames),
            "report" => Some(Self::Report),
            _ => None,
        }
    }

    pub fn content_type(self) -> &'static str {
        match self {
            Self::Frames => FRAMES_CONTENT_TYPE,
            Self::Report => REPORT_CONTENT_TYPE,
            Self::ResearchFrames => RESEARCH_FRAMES_CONTENT_TYPE,
        }
    }

    /// The ADR-B3 storage key. Ids were checked by `ingest::validate::is_safe_id`.
    pub fn key(self, org: Uuid, participant_token: &str, session_id: &str) -> String {
        match self {
            Self::Frames => {
                format!("sessions/{org}/{participant_token}/{session_id}/frames.jsonl.enc")
            }
            Self::Report => format!("reports/{org}/{participant_token}/{session_id}/report.pdf"),
            Self::ResearchFrames => {
                format!("sessions/{org}/{participant_token}/{session_id}/research.jsonl.sealed")
            }
        }
    }

    fn path_segment(self) -> &'static str {
        match self {
            Self::Frames => "frames",
            Self::Report => "report",
            Self::ResearchFrames => "research_frames",
        }
    }
}

/// `(frames_upload_url, report_upload_url)` for the configured blob mode. Called on every
/// accepted post, replays included, so URLs are never stale (CON-17).
pub async fn upload_urls(
    state: &AppState,
    org: Uuid,
    participant_token: &str,
    session_id: &str,
) -> AppResult<(Option<String>, Option<String>)> {
    let mut urls = [None, None];
    for (slot, kind) in urls.iter_mut().zip([BlobKind::Frames, BlobKind::Report]) {
        *slot = Some(match state.config.blob_mode {
            BlobMode::Proxy => format!("/v1/sessions/{session_id}/blobs/{}", kind.path_segment()),
            BlobMode::Presigned => {
                state
                    .blobs
                    .presign_put(
                        &kind.key(org, participant_token, session_id),
                        kind.content_type(),
                    )
                    .await?
                    .url
            }
        });
    }
    let [frames, report] = urls;
    Ok((frames, report))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/sessions/{id}/blobs/{kind}",
            put(upload).layer(DefaultBodyLimit::max(MAX_BLOB_BYTES)),
        )
        // A static segment wins over `{kind}`: the research log never reaches `upload`.
        .route(
            "/sessions/{id}/blobs/research_frames",
            put(upload_research_log).layer(DefaultBodyLimit::max(MAX_RESEARCH_LOG_BYTES)),
        )
}

// ---- research frame log (RFL-3) ------------------------------------------------------------

/// RFL-3 upload cap (16 MiB).
pub const MAX_RESEARCH_LOG_BYTES: usize = asd_contract::research::MAX_RESEARCH_LOG_BYTES;
/// What the blob store holds for a research log: ciphertext, never JSON.
pub const SEALED_CONTENT_TYPE: &str = "application/octet-stream";
/// Length of the data-key id prefixed to a sealed research log.
const KEY_ID_LEN: usize = 16;

/// The AAD of a session's research log (KEY-3).
fn research_aad(org: Uuid, session_id: &str) -> Aad {
    Aad::new("sessions", "research_frames", org, session_id)
}

/// Seals a research log under the org's active data key: `key_id (16 bytes) ||
/// version || nonce || ciphertext || tag`. The key id travels with the object because the
/// session row has no key-id column; the AAD binds it to the org and the session.
pub async fn seal_research_log(
    keyring: &Keyring,
    org: Uuid,
    session_id: &str,
    plaintext: &[u8],
) -> AppResult<Vec<u8>> {
    let sealed = keyring
        .encrypt(org, &research_aad(org, session_id), plaintext)
        .await?;
    let mut out = Vec::with_capacity(KEY_ID_LEN + sealed.ciphertext.len());
    out.extend_from_slice(sealed.key_id.as_bytes());
    out.extend_from_slice(&sealed.ciphertext);
    Ok(out)
}

/// Opens what [`seal_research_log`] produced. Tampered bytes, another session's or org's
/// object, or an unknown key are an error (500 to a client).
pub async fn open_research_log(
    keyring: &Keyring,
    org: Uuid,
    session_id: &str,
    sealed: &[u8],
) -> AppResult<Vec<u8>> {
    if sealed.len() <= KEY_ID_LEN {
        return Err(AppError::Internal(
            "sealed research log is truncated".into(),
        ));
    }
    let (id, ciphertext) = sealed.split_at(KEY_ID_LEN);
    let key_id = Uuid::from_slice(id)
        .map_err(|_| AppError::Internal("sealed research log has no key id".into()))?;
    Ok(keyring
        .decrypt(org, &research_aad(org, session_id), key_id, ciphertext)
        .await?)
}

/// What the database says about a session's research log.
#[derive(Debug, Clone, PartialEq)]
pub struct ResearchLogRow {
    pub session_id: String,
    pub org_id: Uuid,
    pub blob_key: Option<String>,
    pub bytes: Option<i64>,
    pub sha256: Option<Vec<u8>>,
    pub purged: bool,
}

/// Whether a session's research log can be read (RFL-4, RFL-6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResearchLogState {
    Available,
    /// Never uploaded: `404 frames-not-available`.
    NotAvailable,
    /// Deleted by the retention sweep or an erasure: `410 frames-purged`.
    Purged,
}

impl ResearchLogRow {
    pub fn state(&self) -> ResearchLogState {
        if self.blob_key.is_some() {
            ResearchLogState::Available
        } else if self.purged {
            ResearchLogState::Purged
        } else {
            ResearchLogState::NotAvailable
        }
    }

    /// The problem a reader gets when the log cannot be read; `None` when it can.
    pub fn unavailable_problem(&self) -> Option<crate::problem::Problem> {
        use crate::problem::Problem;
        match self.state() {
            ResearchLogState::Available => None,
            ResearchLogState::NotAvailable => Some(
                Problem::new(
                    StatusCode::NOT_FOUND,
                    asd_contract::problem::FRAMES_NOT_AVAILABLE,
                )
                .with_detail("no research frame log was uploaded for this session"),
            ),
            ResearchLogState::Purged => Some(
                Problem::new(StatusCode::GONE, asd_contract::problem::FRAMES_PURGED)
                    .with_detail("the research frame log of this session was deleted"),
            ),
        }
    }
}

fn frames_not_available() -> AppError {
    AppError::Internal("research log read without a stored log".into())
}

/// The research-log columns of session `id` (any org; the caller checks the org).
pub async fn research_log_row(db: &PgPool, id: &str) -> AppResult<Option<ResearchLogRow>> {
    Ok(sqlx::query!(
        r#"SELECT id, org_id, research_frames_blob_key, research_frames_bytes,
                  research_frames_sha256,
                  research_frames_purged_at IS NOT NULL AS "purged!"
           FROM sessions WHERE id = $1"#,
        id
    )
    .fetch_optional(db)
    .await?
    .map(|r| ResearchLogRow {
        session_id: r.id,
        org_id: r.org_id,
        blob_key: r.research_frames_blob_key,
        bytes: r.research_frames_bytes,
        sha256: r.research_frames_sha256,
        purged: r.purged,
    }))
}

/// Fetches and opens the log of `row` (to memory, ≤ 16 MiB) and checks it against the stored
/// SHA-256. The caller has checked the org, availability and written the read audit.
pub async fn read_research_log(
    blobs: &dyn BlobStore,
    keyring: &Keyring,
    row: &ResearchLogRow,
) -> AppResult<Vec<u8>> {
    let key = row.blob_key.as_deref().ok_or_else(frames_not_available)?;
    let stored = blobs
        .get(key)
        .await?
        .ok_or_else(|| AppError::Internal("research log blob missing".into()))?;
    let bytes = open_research_log(keyring, row.org_id, &row.session_id, &stored.bytes).await?;
    if row
        .sha256
        .as_deref()
        .is_some_and(|sha| Sha256::digest(&bytes).as_slice() != sha)
    {
        return Err(AppError::Internal(
            "research log bytes do not match their digest".into(),
        ));
    }
    Ok(bytes)
}

fn invalid(name: &str, reason: impl Into<String>) -> AppError {
    AppError::Validation(vec![InvalidParam {
        name: name.into(),
        reason: reason.into(),
    }])
}

/// RFL-3: the first line must be an `asd-raw-tracking/1` header naming this session.
pub fn check_research_header(body: &[u8], session_id: &str) -> AppResult<()> {
    let first = body.split(|b| *b == b'\n').next().unwrap_or_default();
    let header: RawLogHeader = serde_json::from_slice(first).map_err(|_| {
        invalid(
            "body",
            "the first line must be an asd-raw-tracking/1 log header",
        )
    })?;
    if header.format != RAW_LOG_FORMAT {
        return Err(invalid(
            "body",
            format!("the log format must be {RAW_LOG_FORMAT}"),
        ));
    }
    if header.session_id.as_deref() != Some(session_id) {
        return Err(invalid(
            "body",
            "the log header's sessionId must be the session of the upload path",
        ));
    }
    Ok(())
}

/// `PUT /v1/sessions/{id}/blobs/research_frames` (RFL-3, see the module docs). 204.
async fn upload_research_log(
    device: DevicePrincipal,
    meta: RequestMeta,
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    headers: HeaderMap,
    body: Result<Bytes, BytesRejection>,
) -> AppResult<StatusCode> {
    if state.config.blob_mode != BlobMode::Proxy {
        return Err(AppError::NotFound);
    }
    let crypto = ClinicalCrypto::from_state(&state)?;
    let body = crate::routes::sessions::body_or_problem(body)?;
    let kind = BlobKind::ResearchFrames;
    if content_type(&headers).as_deref() != Some(kind.content_type()) {
        return Err(invalid(
            "Content-Type",
            format!("must be {}", kind.content_type()),
        ));
    }
    if body.is_empty() || body.len() > MAX_RESEARCH_LOG_BYTES {
        return Err(AppError::PayloadTooLarge);
    }
    if !headers.contains_key("content-digest") {
        return Err(AppError::BadRequest {
            slug: asd_contract::problem::DIGEST_REQUIRED,
            detail: "a research frame log upload needs a Content-Digest (RFC 9530)".into(),
        });
    }
    crate::blob::verify_content_digest(&headers, &body)?;
    check_research_header(&body, &session_id)?;

    let owner = sqlx::query!(
        r#"SELECT s.participant_token, p.withdrawn_at IS NOT NULL AS "withdrawn!",
                  COALESCE(s.summary -> 'normCollection' -> 'consented' = 'true'::jsonb, false)
                      AS "consented!"
           FROM sessions s JOIN participants p ON p.id = s.participant_id
           WHERE s.id = $1 AND s.org_id = $2"#,
        session_id,
        device.org_id,
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;
    if owner.withdrawn {
        return Err(participant_withdrawn());
    }
    if !owner.consented {
        return Err(AppError::Conflict {
            slug: asd_contract::problem::RESEARCH_CONSENT_REQUIRED,
            detail: "this session has no research consent; its frame log is not accepted".into(),
        });
    }

    let org = device.org_id;
    let sha = Sha256::digest(&body).to_vec();
    let size = i64::try_from(body.len()).map_err(|_| AppError::PayloadTooLarge)?;
    // Sealed before the store sees it (RFL-3); stored outside a transaction like `upload`.
    let sealed = seal_research_log(&crypto, org, &session_id, &body).await?;
    drop(body);
    let key = kind.key(org, &owner.participant_token, &session_id);
    state
        .blobs
        .put(&key, SEALED_CONTENT_TYPE, Bytes::from(sealed))
        .await?;
    let mut tx = state.db.begin().await?;
    sqlx::query!(
        "UPDATE sessions
         SET research_frames_blob_key = $2, research_frames_bytes = $3,
             research_frames_sha256 = $4, research_frames_purged_at = NULL
         WHERE id = $1",
        session_id,
        key,
        size,
        sha,
    )
    .execute(&mut *tx)
    .await?;
    AuditEvent::device(&device, "session.frames_upload", "session", &session_id)
        .after(serde_json::json!({"bytes": size, "sha256": hex(&sha)}))
        .meta(&meta)
        .record(&mut tx)
        .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

/// The media type of a request, lower-cased, without parameters.
fn content_type(headers: &HeaderMap) -> Option<String> {
    headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|v| {
            v.split(';')
                .next()
                .unwrap_or("")
                .trim()
                .to_ascii_lowercase()
        })
}

/// `PUT /v1/sessions/{id}/blobs/{frames|report}` (proxy mode only). 204 on success.
async fn upload(
    device: DevicePrincipal,
    meta: RequestMeta,
    State(state): State<AppState>,
    Path((session_id, kind)): Path<(String, String)>,
    headers: HeaderMap,
    body: Result<Bytes, BytesRejection>,
) -> AppResult<StatusCode> {
    if state.config.blob_mode != BlobMode::Proxy {
        return Err(AppError::NotFound);
    }
    let kind = BlobKind::parse(&kind).ok_or(AppError::NotFound)?;
    let body = crate::routes::sessions::body_or_problem(body)?;
    if content_type(&headers).as_deref() != Some(kind.content_type()) {
        return Err(AppError::Validation(vec![InvalidParam {
            name: "Content-Type".into(),
            reason: format!("must be {}", kind.content_type()),
        }]));
    }

    let owner = sqlx::query!(
        r#"SELECT s.participant_token, p.withdrawn_at IS NOT NULL AS "withdrawn!"
           FROM sessions s JOIN participants p ON p.id = s.participant_id
           WHERE s.id = $1 AND s.org_id = $2"#,
        session_id,
        device.org_id,
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;
    if owner.withdrawn {
        return Err(participant_withdrawn());
    }
    // SYNC-6 / HIP-4: reject a corrupted upload before anything is stored.
    crate::blob::verify_content_digest(&headers, &body)?;
    let token = owner.participant_token;

    // Store first, outside a transaction (no connection held during a 50 MB upload); a
    // failure after this leaves at most an unreferenced blob that the next PUT overwrites.
    let key = kind.key(device.org_id, &token, &session_id);
    state.blobs.put(&key, kind.content_type(), body).await?;
    let mut tx = state.db.begin().await?;
    match kind {
        BlobKind::Frames => {
            sqlx::query!(
                "UPDATE sessions SET frames_blob_key = $2 WHERE id = $1",
                session_id,
                key
            )
            .execute(&mut *tx)
            .await?;
        }
        BlobKind::Report => {
            sqlx::query!(
                "UPDATE sessions SET report_pdf_key = $2 WHERE id = $1",
                session_id,
                key
            )
            .execute(&mut *tx)
            .await?;
        }
        // Never parsed here (see `BlobKind::ResearchFrames`); WP-X.2 owns its upload.
        BlobKind::ResearchFrames => return Err(AppError::NotFound),
    }
    AuditEvent::device(&device, "session.blob_upload", "session", &session_id)
        .after(serde_json::json!({"blob": kind.path_segment()}))
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
    fn keys_follow_adr_b3_and_validate() {
        let org = Uuid::nil();
        let f = BlobKind::Frames.key(org, "child-1", "S-1");
        assert_eq!(
            f,
            "sessions/00000000-0000-0000-0000-000000000000/child-1/S-1/frames.jsonl.enc"
        );
        let r = BlobKind::Report.key(org, "child-1", "S-1");
        assert_eq!(
            r,
            "reports/00000000-0000-0000-0000-000000000000/child-1/S-1/report.pdf"
        );
        assert!(crate::blob::validate_key(&f).is_ok());
        assert!(crate::blob::validate_key(&r).is_ok());
        assert_eq!(BlobKind::parse("video"), None);
        // RFL-3: the research log key; the plain upload does not accept the kind.
        let k = BlobKind::ResearchFrames.key(org, "child-1", "S-1");
        assert_eq!(
            k,
            "sessions/00000000-0000-0000-0000-000000000000/child-1/S-1/research.jsonl.sealed"
        );
        assert!(crate::blob::validate_key(&k).is_ok());
        assert_eq!(BlobKind::parse("research_frames"), None);
        assert_eq!(
            BlobKind::ResearchFrames.content_type(),
            "application/x-ndjson"
        );
    }

    /// RFL-3: the first line is an `asd-raw-tracking/1` header naming the path's session.
    #[test]
    fn research_header_must_name_the_session() {
        let raw = include_str!("../../../asd-core/tests/fixtures/raw_tracking_sample.jsonl");
        let (first, rest) = raw.split_once('\n').unwrap();
        let with = |header: serde_json::Value| format!("{header}\n{rest}").into_bytes();
        let mut header: serde_json::Value = serde_json::from_str(first).unwrap();
        // A bench log without `sessionId` is not a research log.
        assert!(check_research_header(raw.as_bytes(), "S-1").is_err());
        header["sessionId"] = "S-1".into();
        assert!(check_research_header(&with(header.clone()), "S-1").is_ok());
        assert!(check_research_header(&with(header.clone()), "S-2").is_err());
        header["format"] = "asd-raw-tracking/2".into();
        assert!(check_research_header(&with(header), "S-1").is_err());
        assert!(check_research_header(b"not json\n", "S-1").is_err());
        assert!(check_research_header(b"", "S-1").is_err());
    }
}
