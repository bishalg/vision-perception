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
//! is 404, a session of a participant who withdrew consent is `409 conflict` (as ingest).

use axum::body::Bytes;
use axum::extract::rejection::BytesRejection;
use axum::extract::{DefaultBodyLimit, Path, State};
use axum::routing::put;
use axum::Router;
use http::{header, HeaderMap, StatusCode};
use uuid::Uuid;

use crate::app::AppState;
use crate::audit::AuditEvent;
use crate::auth::extractors::{DevicePrincipal, RequestMeta};
use crate::blob::MAX_BLOB_BYTES;
use crate::config::BlobMode;
use crate::error::{AppError, AppResult};
use crate::ingest::participant_withdrawn;
use crate::problem::InvalidParam;

pub const FRAMES_CONTENT_TYPE: &str = "application/octet-stream";
pub const REPORT_CONTENT_TYPE: &str = "application/pdf";

/// The two blobs a session may carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlobKind {
    Frames,
    Report,
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
        }
    }

    /// The ADR-B3 storage key. Ids were checked by `ingest::validate::is_safe_id`.
    pub fn key(self, org: Uuid, participant_token: &str, session_id: &str) -> String {
        match self {
            Self::Frames => {
                format!("sessions/{org}/{participant_token}/{session_id}/frames.jsonl.enc")
            }
            Self::Report => format!("reports/{org}/{participant_token}/{session_id}/report.pdf"),
        }
    }

    fn path_segment(self) -> &'static str {
        match self {
            Self::Frames => "frames",
            Self::Report => "report",
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
    Router::new().route(
        "/sessions/{id}/blobs/{kind}",
        put(upload).layer(DefaultBodyLimit::max(MAX_BLOB_BYTES)),
    )
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
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|v| {
            v.split(';')
                .next()
                .unwrap_or("")
                .trim()
                .to_ascii_lowercase()
        });
    if content_type.as_deref() != Some(kind.content_type()) {
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
    }
}
