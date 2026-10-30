//! owner: WP-B.3
//!
//! Session routes.
//!
//! Device plane (API-1): `POST /v1/sessions` (ING-1..3, CON-13..18, CON-21).
//! User plane (API-2): `GET /v1/orgs/{id}/sessions`, `GET /v1/sessions/{id}`,
//! `GET /v1/sessions/{id}/clips` (CON-19), `GET /v1/sessions/{id}/report.pdf` (the
//! device-uploaded PDF, or 404 until one exists; server rendering is WP-B.8).
//!
//! Auth (WP-B.2): the device plane takes a [`DevicePrincipal`] (`dvc_` bearer) and is
//! limited to [`SESSION_POSTS_PER_MINUTE`] per device (AUTH-3, `429` + `Retry-After`); the
//! console routes take `Require<perm::SessionRead>` and 404 another org's session (AUTH-4).

use std::sync::Arc;

use asd_contract::api_v1::{ClipList, SessionDetail, SessionPage};
use asd_contract::session_v1_6::{
    SessionIngestResponse, IDEMPOTENCY_KEY_HEADER, SCHEMA_VERSION_HEADER,
};
use axum::body::{Body, Bytes};
use axum::extract::rejection::{BytesRejection, QueryRejection};
use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use http::{header, HeaderMap, HeaderValue, StatusCode};
use uuid::Uuid;

use crate::app::AppState;
use crate::auth::extractors::{DevicePrincipal, RequestMeta, Require};
use crate::auth::rbac::perm;
use crate::auth::RateLimiter;
use crate::config::BlobMode;
use crate::error::{AppError, AppResult};
use crate::ingest::{self, validate};
use crate::problem::InvalidParam;
use crate::read;
use crate::read::sessions::SessionQuery;

/// Largest accepted `POST /v1/sessions` body (a full comprehensive battery is far smaller).
pub const MAX_SESSION_BODY_BYTES: usize = 4 * 1024 * 1024;

/// AUTH-3: `POST /v1/sessions` attempts per device per minute.
pub const SESSION_POSTS_PER_MINUTE: u32 = 60;

/// Per-device limiter for `POST /v1/sessions` (one per router, shared by its clones).
#[derive(Debug)]
pub struct SessionPostLimiter(pub RateLimiter<Uuid>);

pub fn router() -> Router<AppState> {
    let limiter = Arc::new(SessionPostLimiter(RateLimiter::per_minute(
        SESSION_POSTS_PER_MINUTE,
    )));
    Router::new()
        .route(
            "/sessions",
            post(create).layer(DefaultBodyLimit::max(MAX_SESSION_BODY_BYTES)),
        )
        .route("/orgs/{id}/sessions", get(list))
        .route("/sessions/{id}", get(detail))
        .route("/sessions/{id}/clips", get(clips))
        .route("/sessions/{id}/report.pdf", get(report_pdf))
        .layer(Extension(limiter))
}

fn header_str<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name).and_then(|v| v.to_str().ok())
}

/// Maps a body rejection to problem+json (413 or 400).
pub(crate) fn body_or_problem(body: Result<Bytes, BytesRejection>) -> AppResult<Bytes> {
    body.map_err(|e| {
        if e.status() == StatusCode::PAYLOAD_TOO_LARGE {
            AppError::PayloadTooLarge
        } else {
            AppError::Validation(vec![InvalidParam {
                name: "body".into(),
                reason: "could not read the request body".into(),
            }])
        }
    })
}

pub(crate) fn query_or_problem<T>(q: Result<Query<T>, QueryRejection>) -> AppResult<T> {
    q.map(|Query(t)| t).map_err(|_| {
        AppError::Validation(vec![InvalidParam {
            name: "query".into(),
            reason: "malformed query string".into(),
        }])
    })
}

/// A path UUID; anything else is an unknown resource (404).
pub(crate) fn path_uuid(raw: &str) -> AppResult<Uuid> {
    Uuid::parse_str(raw).map_err(|_| AppError::NotFound)
}

/// `POST /v1/sessions`: 201 new, 200 replay (`duplicate: true`), 409 key reused or
/// participant withdrawn, 429 over the per-device rate.
async fn create(
    device: DevicePrincipal,
    State(state): State<AppState>,
    Extension(limiter): Extension<Arc<SessionPostLimiter>>,
    meta: RequestMeta,
    headers: HeaderMap,
    body: Result<Bytes, BytesRejection>,
) -> AppResult<(StatusCode, Json<SessionIngestResponse>)> {
    // Every authenticated attempt counts, valid or not.
    limiter
        .0
        .check(&device.device_id)
        .map_err(|retry_after_secs| AppError::RateLimited { retry_after_secs })?;
    let body = body_or_problem(body)?;
    let validated = validate::validate(
        &body,
        header_str(&headers, SCHEMA_VERSION_HEADER),
        header_str(&headers, IDEMPOTENCY_KEY_HEADER),
        state.config.allow_synthetic,
    )?;
    let done = ingest::ingest(&state, device, &meta, validated).await?;
    Ok((done.status, Json(done.response)))
}

async fn list(
    auth: Require<perm::SessionRead>,
    State(state): State<AppState>,
    query: Result<Query<SessionQuery>, QueryRejection>,
) -> AppResult<Json<SessionPage>> {
    let org = auth.org();
    let filters = query_or_problem(query)?.parse()?;
    Ok(Json(read::sessions::list(&state.db, org, &filters).await?))
}

async fn detail(
    auth: Require<perm::SessionRead>,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<SessionDetail>> {
    let presigned = state.config.blob_mode == BlobMode::Presigned;
    let (org, detail) = read::sessions::detail(&state.db, &id, presigned)
        .await?
        .ok_or(AppError::NotFound)?;
    auth.ensure_org(org)?;
    Ok(Json(detail))
}

/// The org of a session, or 404.
async fn session_org(state: &AppState, id: &str) -> AppResult<Uuid> {
    sqlx::query_scalar!("SELECT org_id FROM sessions WHERE id = $1", id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)
}

async fn clips(
    auth: Require<perm::SessionRead>,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<ClipList>> {
    auth.ensure_org(session_org(&state, &id).await?)?;
    Ok(Json(read::clips::list(&state.db, &id).await?))
}

async fn report_pdf(
    auth: Require<perm::SessionRead>,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Response> {
    let (org, key) = read::sessions::report_key(&state.db, &id)
        .await?
        .ok_or(AppError::NotFound)?;
    auth.ensure_org(org)?;
    let blob = state.blobs.get(&key).await?.ok_or(AppError::NotFound)?;
    let mut response = Body::from(blob.bytes).into_response();
    let h = response.headers_mut();
    h.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(crate::routes::blobs::REPORT_CONTENT_TYPE),
    );
    h.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    h.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static("inline; filename=\"report.pdf\""),
    );
    Ok(response)
}
