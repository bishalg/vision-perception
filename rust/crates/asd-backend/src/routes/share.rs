//! owner: WP-I.13
//!
//! Share links of case packages (SHR-8, SHR-9, HIP-8, HIP-9, ADR-0011 D5):
//!
//! - `POST /v1/exports/{id}/share-link` (`ShareLinkManage`, same org): a link to a ready
//!   `case_package` export; answers `201 CreatedShareLink`, the only time the passcode is
//!   ever returned (it is stored as an Argon2id hash with the AUTH-1 parameters);
//! - `GET /v1/exports/{id}/share-links` (`ShareLinkManage`): the export's links, never a
//!   passcode;
//! - `DELETE /v1/share-links/{id}` (`ShareLinkManage`): revoke (idempotent, `204`);
//! - public, no bearer (a nested router with its own `Cache-Control: no-store`,
//!   `X-Robots-Tag: noindex, nofollow` and `Referrer-Policy: no-referrer` layers and a
//!   rate limiter):
//!   - `POST /v1/share/{id}/open {passcode}` → `200 ShareLinkOpenResponse` with a
//!     five-minute download token; `404` unknown or revoked, `410 share-link-expired`,
//!     `410 share-link-exhausted`, `401 share-passcode-invalid`, `410 export-expired`,
//!     `429` + `Retry-After` past 10 opens per link or 30 per client IP per hour;
//!   - `GET /v1/share/{id}/file?t=<token>` streams the artifact and counts one download
//!     with `UPDATE ... WHERE downloads < max_downloads` (`410 share-link-exhausted`
//!     otherwise), so concurrent downloads never exceed the limit.
//!
//! Audit (HIP-8): `share_link.create` and `.revoke` name the user; `.open`,
//! `.open_failed` and `.download` are `system` rows carrying the client IP and user agent
//! (`RequestMeta`, `ASD_TRUST_PROXY_HEADERS` decides which IP), written in the request's
//! transaction before any byte is served; a failed audit is a `500`, never best-effort.
//! Neither the passcode nor the download token is ever logged or audited (the request
//! span records the route template only, OBS-1).
//!
//! The rate limits are per process (like every `RateLimiter`); a link expires at most
//! seven days after creation and never after its export's artifact (EXJ-4), and expired
//! or revoked links delete nothing.

use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

use asd_contract::emr::{
    CreateShareLinkRequest, CreatedShareLink, ShareLink, ShareLinkOpenRequest,
    ShareLinkOpenResponse, ShareLinkPage, SHARE_LINK_DEFAULT_DAYS, SHARE_LINK_DEFAULT_DOWNLOADS,
    SHARE_LINK_MAX_DAYS, SHARE_LINK_MAX_DOWNLOADS, SHARE_LINK_MIN_DOWNLOADS,
    SHARE_PASSCODE_ALPHABET, SHARE_PASSCODE_LEN,
};
use asd_contract::export::{ExportFormat, ExportMode};
use asd_contract::problem::{
    CONFLICT, SHARE_LINK_EXHAUSTED, SHARE_LINK_EXPIRED, SHARE_PASSCODE_INVALID,
};
use aws_lc_rs::hmac;
use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Extension, Json, Router};
use base64::Engine as _;
use chrono::{DateTime, Utc};
use http::{header, HeaderName, HeaderValue, StatusCode};
use serde::Deserialize;
use serde_json::json;
use tower_http::set_header::SetResponseHeaderLayer;
use uuid::Uuid;

use crate::app::AppState;
use crate::audit::AuditEvent;
use crate::auth::extractors::{ApiJson, RequestMeta, Require};
use crate::auth::rbac::{self, perm, Action};
use crate::auth::{password, random_bytes, RateLimiter};
use crate::error::{AppError, AppResult};
use crate::export_build::{self, ExportRow};
use crate::problem::InvalidParam;
use crate::read::cursor;
use crate::routes::exports::ensure_downloadable;

/// SHR-9: opens per link per hour.
pub const OPENS_PER_LINK_PER_HOUR: u32 = 10;
/// SHR-9: opens per client IP per hour.
pub const OPENS_PER_IP_PER_HOUR: u32 = 30;
/// SHR-9: lifetime of a download token (as EXJ-5's proxy tokens).
pub const DOWNLOAD_TOKEN_TTL_SECS: i64 = 300;

const X_ROBOTS_TAG: HeaderName = HeaderName::from_static("x-robots-tag");

/// The `open` limits (SHR-9), keyed by link and by client IP; one instance per router.
#[derive(Debug)]
pub struct ShareLimiter {
    by_link: RateLimiter<Uuid>,
    by_ip: RateLimiter<Option<IpAddr>>,
}

impl ShareLimiter {
    pub fn new() -> Self {
        let hour = Duration::from_secs(3600);
        Self {
            by_link: RateLimiter::new(OPENS_PER_LINK_PER_HOUR, hour),
            by_ip: RateLimiter::new(OPENS_PER_IP_PER_HOUR, hour),
        }
    }
}

impl Default for ShareLimiter {
    fn default() -> Self {
        Self::new()
    }
}

pub fn router() -> Router<AppState> {
    // HIP-9: nothing a share route answers may be cached or indexed, errors included.
    let public = Router::new()
        .route("/share/{id}/open", post(open))
        .route("/share/{id}/file", get(file))
        .layer(Extension(Arc::new(ShareLimiter::new())))
        .layer(SetResponseHeaderLayer::overriding(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-store"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            X_ROBOTS_TAG,
            HeaderValue::from_static("noindex, nofollow"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::REFERRER_POLICY,
            HeaderValue::from_static("no-referrer"),
        ));
    Router::new()
        .route("/exports/{id}/share-link", post(create))
        .route("/exports/{id}/share-links", get(list))
        .route("/share-links/{id}", delete(revoke))
        .merge(public)
}

// ---- passcodes and download tokens --------------------------------------------------------

/// A fresh 12-character passcode from `[A-HJ-NP-Z2-9]` (SHR-8). The alphabet has 32
/// symbols, so `byte & 31` is unbiased: 60 bits of entropy.
pub fn new_passcode() -> String {
    random_bytes::<SHARE_PASSCODE_LEN>()
        .iter()
        .map(|b| char::from(SHARE_PASSCODE_ALPHABET[usize::from(b & 31)]))
        .collect()
}

/// What people may type: case-insensitive, spaces and dashes ignored.
pub fn normalize_passcode(typed: &str) -> String {
    typed
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .flat_map(char::to_uppercase)
        .collect()
}

fn token_message(link: Uuid, passcode_hash: &str, exp: i64) -> String {
    format!("share|{link}|{passcode_hash}|{exp}")
}

/// `t` of `/v1/share/{id}/file?t=...`: `{exp}.{base64url(HMAC-SHA256)}` over
/// `(link id, passcode hash, exp)`, keyed with the export's own `download_secret` (EXJ-5).
pub fn download_token(secret: &[u8], link: Uuid, passcode_hash: &str, exp: i64) -> String {
    let key = hmac::Key::new(hmac::HMAC_SHA256, secret);
    let tag = hmac::sign(&key, token_message(link, passcode_hash, exp).as_bytes());
    format!(
        "{exp}.{}",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(tag.as_ref())
    )
}

/// Whether `token` is genuine for this link and passcode and not expired at `now`
/// (constant-time tag comparison).
pub fn verify_download_token(
    secret: &[u8],
    link: Uuid,
    passcode_hash: &str,
    token: &str,
    now: i64,
) -> bool {
    let Some((exp, tag)) = token.split_once('.') else {
        return false;
    };
    let Ok(exp) = exp.parse::<i64>() else {
        return false;
    };
    let Ok(tag) = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(tag) else {
        return false;
    };
    let key = hmac::Key::new(hmac::HMAC_SHA256, secret);
    hmac::verify(
        &key,
        token_message(link, passcode_hash, exp).as_bytes(),
        &tag,
    )
    .is_ok()
        && exp > now
}

// ---- rows ---------------------------------------------------------------------------------

struct LinkRow {
    id: Uuid,
    org_id: Uuid,
    export_id: Uuid,
    passcode_hash: String,
    created_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
    max_downloads: i32,
    downloads: i32,
    opens: i32,
    revoked_at: Option<DateTime<Utc>>,
    last_opened_at: Option<DateTime<Utc>>,
}

impl std::fmt::Debug for LinkRow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LinkRow")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

impl LinkRow {
    fn to_dto(&self, console_origin: Option<&str>) -> ShareLink {
        ShareLink {
            id: self.id.to_string(),
            export_id: self.export_id.to_string(),
            url: link_url(console_origin, self.id),
            expires_at: cursor::ts(self.expires_at),
            max_downloads: self.max_downloads,
            downloads: self.downloads,
            opens: self.opens,
            revoked_at: self.revoked_at.map(cursor::ts),
            created_at: cursor::ts(self.created_at),
            last_opened_at: self.last_opened_at.map(cursor::ts),
        }
    }
}

/// The console's public page for a link: `{ASD_CONSOLE_ORIGIN}/share/{id}`, relative when
/// no origin is configured.
pub fn link_url(console_origin: Option<&str>, id: Uuid) -> String {
    format!("{}/share/{id}", console_origin.unwrap_or(""))
}

async fn load_link(db: &sqlx::PgPool, id: Uuid) -> AppResult<Option<LinkRow>> {
    Ok(sqlx::query_as!(
        LinkRow,
        r#"SELECT id, org_id, export_id, passcode_hash, created_at, expires_at, max_downloads,
                  downloads, opens, revoked_at, last_opened_at
           FROM share_links WHERE id = $1"#,
        id
    )
    .fetch_optional(db)
    .await?)
}

fn path_id(raw: &str) -> AppResult<Uuid> {
    Uuid::parse_str(raw).map_err(|_| AppError::NotFound)
}

fn bad(name: &str, reason: impl Into<String>) -> InvalidParam {
    InvalidParam {
        name: name.into(),
        reason: reason.into(),
    }
}

fn no_store(mut response: Response) -> Response {
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

/// An audit row that must exist before the response does (HIP-8): its own transaction,
/// any failure a `500`.
async fn audit_now(state: &AppState, event: AuditEvent) -> AppResult<()> {
    let fail = |e: &dyn std::fmt::Display| AppError::Internal(format!("share audit failed: {e}"));
    let mut tx = state.db.begin().await.map_err(|e| fail(&e))?;
    event.record(&mut tx).await.map_err(|e| fail(&e))?;
    tx.commit().await.map_err(|e| fail(&e))
}

// ---- authenticated: create, list, revoke ----------------------------------------------------

async fn case_export_in_org(
    state: &AppState,
    auth: &Require<perm::ShareLinkManage>,
    raw: &str,
) -> AppResult<ExportRow> {
    let row = export_build::load_export(&state.db, path_id(raw)?)
        .await?
        .ok_or(AppError::NotFound)?;
    auth.ensure_org(row.org_id)?;
    Ok(row)
}

async fn create(
    auth: Require<perm::ShareLinkManage>,
    State(state): State<AppState>,
    meta: RequestMeta,
    Path(raw): Path<String>,
    ApiJson(req): ApiJson<CreateShareLinkRequest>,
) -> AppResult<Response> {
    let p = auth.principal;
    let export = case_export_in_org(&state, &auth, &raw).await?;
    // RBAC-3: the vendor never hands out clinic data (superadmin lacks the action anyway).
    if p.is_superadmin() {
        return Err(AppError::Forbidden);
    }
    // As the download URL (EXJ-5): an identified artifact goes out only through its
    // requester or someone who may export identified data.
    if export.mode == ExportMode::Identified
        && export.requested_by != Some(p.user_id)
        && !rbac::allows(p.role, Action::ExportIdentified)
    {
        return Err(AppError::Forbidden);
    }
    let mut errors = Vec::new();
    let days = req.expires_in_days.unwrap_or(SHARE_LINK_DEFAULT_DAYS);
    if !(1..=SHARE_LINK_MAX_DAYS).contains(&days) {
        errors.push(bad(
            "expires_in_days",
            format!("must be 1..{SHARE_LINK_MAX_DAYS}"),
        ));
    }
    let max_downloads = req.max_downloads.unwrap_or(SHARE_LINK_DEFAULT_DOWNLOADS);
    if !(SHARE_LINK_MIN_DOWNLOADS..=SHARE_LINK_MAX_DOWNLOADS).contains(&max_downloads) {
        errors.push(bad(
            "max_downloads",
            format!("must be {SHARE_LINK_MIN_DOWNLOADS}..{SHARE_LINK_MAX_DOWNLOADS}"),
        ));
    }
    if !errors.is_empty() {
        return Err(AppError::Validation(errors));
    }
    if export.format != ExportFormat::CasePackage {
        return Err(AppError::Conflict {
            slug: CONFLICT,
            detail: "share links exist for case_package exports only".into(),
        });
    }
    // 410 export-expired for an expired artifact, 409 while it is not ready.
    ensure_downloadable(&export)?;

    let passcode = new_passcode();
    let passcode_hash = password::hash_blocking(passcode.clone()).await?;
    let id = Uuid::new_v4();
    let days = i32::try_from(days).expect("at most 7");
    let mut tx = state.db.begin().await?;
    // A link never outlives its artifact (EXJ-4): `least` ignores a NULL expiry.
    let row = sqlx::query_as!(
        LinkRow,
        r#"INSERT INTO share_links
             (id, org_id, export_id, passcode_hash, created_by, created_at, expires_at,
              max_downloads)
           VALUES ($1, $2, $3, $4, $5, now(),
                   least(now() + make_interval(days => $6), $7), $8)
           RETURNING id, org_id, export_id, passcode_hash, created_at,
                     expires_at AS "expires_at!", max_downloads, downloads, opens,
                     revoked_at, last_opened_at"#,
        id,
        export.org_id,
        export.id,
        passcode_hash,
        p.user_id,
        days,
        export.expires_at,
        max_downloads,
    )
    .fetch_one(&mut *tx)
    .await?;
    AuditEvent::user(&p, "share_link.create", "share_link", id)
        .org(export.org_id)
        .after(json!({
            "export_id": export.id,
            "expires_at": cursor::ts(row.expires_at),
            "max_downloads": max_downloads,
        }))
        .meta(&meta)
        .record(&mut tx)
        .await?;
    tx.commit().await?;
    let body = CreatedShareLink {
        link: row.to_dto(state.config.console_origin.as_deref()),
        passcode,
    };
    // The passcode is in this body: never cached anywhere.
    Ok(no_store((StatusCode::CREATED, Json(body)).into_response()))
}

async fn list(
    auth: Require<perm::ShareLinkManage>,
    State(state): State<AppState>,
    Path(raw): Path<String>,
) -> AppResult<Json<ShareLinkPage>> {
    let export = case_export_in_org(&state, &auth, &raw).await?;
    let rows = sqlx::query_as!(
        LinkRow,
        r#"SELECT id, org_id, export_id, passcode_hash, created_at, expires_at, max_downloads,
                  downloads, opens, revoked_at, last_opened_at
           FROM share_links WHERE export_id = $1
           ORDER BY created_at DESC, id DESC"#,
        export.id
    )
    .fetch_all(&state.db)
    .await?;
    let origin = state.config.console_origin.as_deref();
    Ok(Json(ShareLinkPage {
        items: rows.iter().map(|r| r.to_dto(origin)).collect(),
        next_cursor: None,
    }))
}

async fn revoke(
    auth: Require<perm::ShareLinkManage>,
    State(state): State<AppState>,
    meta: RequestMeta,
    Path(raw): Path<String>,
) -> AppResult<StatusCode> {
    let link = load_link(&state.db, path_id(&raw)?)
        .await?
        .ok_or(AppError::NotFound)?;
    auth.ensure_org(link.org_id)?;
    let mut tx = state.db.begin().await?;
    let revoked = sqlx::query!(
        "UPDATE share_links SET revoked_at = now(), revoked_by = $2
         WHERE id = $1 AND revoked_at IS NULL",
        link.id,
        auth.principal.user_id,
    )
    .execute(&mut *tx)
    .await?;
    if revoked.rows_affected() == 1 {
        AuditEvent::user(&auth.principal, "share_link.revoke", "share_link", link.id)
            .org(link.org_id)
            .after(json!({"export_id": link.export_id}))
            .meta(&meta)
            .record(&mut tx)
            .await?;
    }
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

// ---- public: open, file ---------------------------------------------------------------------

fn link_expired() -> AppError {
    AppError::Gone {
        slug: SHARE_LINK_EXPIRED,
        detail: "this share link has expired; ask the sender for a new one".into(),
    }
}

fn link_exhausted() -> AppError {
    AppError::Gone {
        slug: SHARE_LINK_EXHAUSTED,
        detail: "this share link has no downloads left; ask the sender for a new one".into(),
    }
}

/// An unrevoked link, else `404` (unknown and revoked look the same).
async fn live_link(state: &AppState, raw: &str) -> AppResult<LinkRow> {
    load_link(&state.db, path_id(raw)?)
        .await?
        .filter(|l| l.revoked_at.is_none())
        .ok_or(AppError::NotFound)
}

async fn open(
    State(state): State<AppState>,
    Extension(limiter): Extension<Arc<ShareLimiter>>,
    meta: RequestMeta,
    Path(raw): Path<String>,
    ApiJson(req): ApiJson<ShareLinkOpenRequest>,
) -> AppResult<Json<ShareLinkOpenResponse>> {
    let id = path_id(&raw)?;
    let now = state.clock.now();
    let limited = |retry_after_secs| AppError::RateLimited { retry_after_secs };
    limiter.by_ip.check_at(&meta.ip, now).map_err(limited)?;
    limiter.by_link.check_at(&id, now).map_err(limited)?;

    let link = live_link(&state, &raw).await?;
    if link.expires_at <= Utc::now() {
        return Err(link_expired());
    }
    if link.downloads >= link.max_downloads {
        return Err(link_exhausted());
    }
    // A wrong length can never match; it costs no Argon2 (bounds hashing work).
    let typed = normalize_passcode(&req.passcode);
    drop(req);
    let ok = typed.chars().count() == SHARE_PASSCODE_LEN
        && password::verify_or_dummy_blocking(Some(link.passcode_hash.clone()), typed).await;
    if !ok {
        audit_now(
            &state,
            AuditEvent::system("share_link.open_failed", "share_link", link.id)
                .org(link.org_id)
                .after(json!({"export_id": link.export_id}))
                .meta(&meta),
        )
        .await?;
        return Err(AppError::UnauthorizedWith {
            slug: SHARE_PASSCODE_INVALID,
            detail: "the passcode does not match this link".into(),
        });
    }
    let export = export_build::load_export(&state.db, link.export_id)
        .await?
        .ok_or_else(|| AppError::Internal("share link without its export".into()))?;
    ensure_downloadable(&export)?;

    let ip = meta.ip.map(|ip| ip.to_string());
    let fail = |e: &dyn std::fmt::Display| AppError::Internal(format!("share audit failed: {e}"));
    let mut tx = state.db.begin().await.map_err(|e| fail(&e))?;
    sqlx::query!(
        "UPDATE share_links
         SET opens = opens + 1, last_opened_at = now(), last_opened_ip = CAST($2::text AS inet)
         WHERE id = $1",
        link.id,
        ip,
    )
    .execute(&mut *tx)
    .await?;
    AuditEvent::system("share_link.open", "share_link", link.id)
        .org(link.org_id)
        .after(json!({"export_id": link.export_id}))
        .meta(&meta)
        .record(&mut tx)
        .await
        .map_err(|e| fail(&e))?;
    tx.commit().await.map_err(|e| fail(&e))?;

    let exp = Utc::now().timestamp() + DOWNLOAD_TOKEN_TTL_SECS;
    let expires_at = match export.expires_at {
        Some(artifact) if artifact < link.expires_at => artifact,
        _ => link.expires_at,
    };
    Ok(Json(ShareLinkOpenResponse {
        export_id: export.id.to_string(),
        file_name: export
            .file_name
            .clone()
            .unwrap_or_else(|| "case.zip".into()),
        bytes: export
            .byte_size
            .and_then(|b| u64::try_from(b).ok())
            .unwrap_or(0),
        expires_at: cursor::ts(expires_at),
        downloads_left: link.max_downloads - link.downloads,
        download_token: download_token(&export.download_secret, link.id, &link.passcode_hash, exp),
    }))
}

#[derive(Deserialize)]
struct FileQuery {
    t: Option<String>,
}

/// Streams the artifact and counts one download. A missing, bad or expired token is
/// `403`; the query string is never logged (OBS-1).
async fn file(
    State(state): State<AppState>,
    meta: RequestMeta,
    Path(raw): Path<String>,
    Query(q): Query<FileQuery>,
) -> AppResult<Response> {
    let link = live_link(&state, &raw).await?;
    let export = export_build::load_export(&state.db, link.export_id)
        .await?
        .ok_or_else(|| AppError::Internal("share link without its export".into()))?;
    let genuine = q.t.as_deref().is_some_and(|t| {
        verify_download_token(
            &export.download_secret,
            link.id,
            &link.passcode_hash,
            t,
            Utc::now().timestamp(),
        )
    });
    if !genuine {
        return Err(AppError::Forbidden);
    }
    if link.expires_at <= Utc::now() {
        return Err(link_expired());
    }
    let key = ensure_downloadable(&export)?;
    // Read before counting: a lost blob must not consume a download.
    let blob = state.blobs.get(&key).await?.ok_or_else(|| {
        AppError::Internal(format!(
            "export {} artifact missing from the blob store",
            export.id
        ))
    })?;

    let fail = |e: &dyn std::fmt::Display| AppError::Internal(format!("share audit failed: {e}"));
    let mut tx = state.db.begin().await.map_err(|e| fail(&e))?;
    let counted = sqlx::query_scalar!(
        "UPDATE share_links SET downloads = downloads + 1
         WHERE id = $1 AND revoked_at IS NULL AND expires_at > now()
           AND downloads < max_downloads
         RETURNING downloads",
        link.id,
    )
    .fetch_optional(&mut *tx)
    .await?;
    let Some(downloads) = counted else {
        tx.rollback().await.ok();
        return Err(link_exhausted());
    };
    AuditEvent::system("share_link.download", "share_link", link.id)
        .org(link.org_id)
        .after(json!({"export_id": link.export_id, "downloads": downloads}))
        .meta(&meta)
        .record(&mut tx)
        .await
        .map_err(|e| fail(&e))?;
    tx.commit().await.map_err(|e| fail(&e))?;

    let file_name = export
        .file_name
        .clone()
        .map(|n| asd_contract::emr::sanitize_file_name(&n))
        .unwrap_or_else(|| "case.zip".into());
    let mut response = Response::new(Body::from(blob.bytes));
    let h = response.headers_mut();
    h.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&blob.content_type)
            .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream")),
    );
    if let Ok(v) = HeaderValue::from_str(&format!("attachment; filename=\"{file_name}\"")) {
        h.insert(header::CONTENT_DISPOSITION, v);
    }
    h.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passcodes_use_the_unambiguous_alphabet() {
        let mut seen = std::collections::HashSet::new();
        for _ in 0..200 {
            let p = new_passcode();
            assert_eq!(p.len(), SHARE_PASSCODE_LEN);
            assert!(
                p.bytes().all(|b| SHARE_PASSCODE_ALPHABET.contains(&b)),
                "{p}"
            );
            assert!(!p.contains(['I', 'O', '0', '1']));
            assert!(seen.insert(p));
        }
        assert_eq!(normalize_passcode(" abcd-efgh jkmn "), "ABCDEFGHJKMN");
    }

    #[test]
    fn download_tokens_bind_link_passcode_and_expiry() {
        let secret = [3u8; 32];
        let link = Uuid::new_v4();
        let t = download_token(&secret, link, "$argon2id$a", 1_000);
        assert!(verify_download_token(&secret, link, "$argon2id$a", &t, 999));
        assert!(!verify_download_token(
            &secret,
            link,
            "$argon2id$a",
            &t,
            1_000
        ));
        assert!(!verify_download_token(
            &secret,
            Uuid::new_v4(),
            "$argon2id$a",
            &t,
            0
        ));
        assert!(!verify_download_token(&secret, link, "$argon2id$b", &t, 0));
        assert!(!verify_download_token(
            &[4u8; 32],
            link,
            "$argon2id$a",
            &t,
            0
        ));
        let later = t.replacen("1000", "9999", 1);
        assert!(!verify_download_token(
            &secret,
            link,
            "$argon2id$a",
            &later,
            0
        ));
        assert!(!verify_download_token(
            &secret,
            link,
            "$argon2id$a",
            "garbage",
            0
        ));
        assert!(!verify_download_token(
            &secret,
            link,
            "$argon2id$a",
            "1.%%%",
            0
        ));
    }

    #[test]
    fn urls_follow_the_console_origin() {
        let id = Uuid::nil();
        assert_eq!(
            link_url(Some("https://console.example.org"), id),
            format!("https://console.example.org/share/{id}")
        );
        assert_eq!(link_url(None, id), format!("/share/{id}"));
    }
}
