//! owner: WP-B.4
//!
//! Config routes (CFG-1, CON-6, 02 ADR-B6).
//!
//! Device plane (bearer = device token):
//! - `GET  /v1/config/bundle` -> the signed `ConfigBundle` for the device's org (else the
//!   global default). `ETag: "<bundle_version>"`, `Cache-Control: no-cache`;
//!   `If-None-Match` hit -> 304. Records `bundle_adoption`. 404 when none exists.
//!
//! User plane (bearer = JWT, `auth::rbac` matrix):
//! - `GET  /v1/config/{kind}?status=&cursor=&limit=` (`ConfigRead`) -> `ConfigDocumentPage`
//! - `POST /v1/config/{kind}` (`ConfigGlobalManage`: superadmin only, the documents are
//!   global) `CreateConfigDocumentRequest` -> 201 draft
//! - `GET  /v1/config/{kind}/{version}` (`ConfigRead`) -> `ConfigDocument`
//! - `POST /v1/config/{kind}/{version}/approve|publish|retire` (`ConfigGlobalManage`) ->
//!   `ConfigDocument` (four-eyes rule, see `config_docs::lifecycle`)
//! - `POST /v1/config/bundles` (`ConfigPublish`) `{org_id?, feature_flags?}` -> 201
//!   `ConfigBundleSummary` + `ETag`
//! - `GET  /v1/config/bundles?cursor=&limit=` (`ConfigRead`) -> `{items, next_cursor, total}`
//! - `GET  /v1/config/bundles/{version}` (`ConfigRead`) -> the `ConfigBundle` as served
//!
//! `{kind}` is `stimulus_catalog | normative_baselines | scoring_profile`; anything else is
//! a 404.

use std::collections::BTreeMap;

use asd_contract::api_v1::{
    ConfigBundleSummary, ConfigDocument, ConfigDocumentKind, ConfigDocumentPage,
    CreateConfigDocumentRequest,
};
use axum::body::Bytes;
use axum::extract::rejection::{BytesRejection, QueryRejection};
use axum::extract::{Path, Query, State};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use http::{header, HeaderMap, HeaderValue, StatusCode};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::app::AppState;
use crate::auth::extractors::{json_or_default, ApiJson, DevicePrincipal, RequestMeta, Require};
use crate::auth::rbac::perm;
use crate::config_docs::bundle::{self, Served};
use crate::config_docs::{lifecycle, status_from_str, ConfigApiError, ConfigApiResult};
use crate::error::AppError;
use crate::problem::InvalidParam;
use crate::read::cursor::{self, Cursor};
use crate::routes::sessions::{body_or_problem, query_or_problem};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/config/bundle", get(device_bundle))
        .route("/config/bundles", get(list_bundles).post(compose_bundle))
        .route("/config/bundles/{version}", get(get_bundle))
        .route("/config/{kind}", get(list_documents).post(create_document))
        .route("/config/{kind}/{version}", get(get_document))
        .route("/config/{kind}/{version}/approve", post(approve_document))
        .route("/config/{kind}/{version}/publish", post(publish_document))
        .route("/config/{kind}/{version}/retire", post(retire_document))
}

fn invalid(name: &str, reason: &str) -> ConfigApiError {
    AppError::Validation(vec![InvalidParam {
        name: name.into(),
        reason: reason.into(),
    }])
    .into()
}

fn kind(raw: &str) -> ConfigApiResult<ConfigDocumentKind> {
    crate::config_docs::kind_from_str(raw).ok_or(ConfigApiError::App(AppError::NotFound))
}

#[derive(Debug, Default, Deserialize)]
struct PageQuery {
    status: Option<String>,
    cursor: Option<String>,
    limit: Option<String>,
}

fn page_params(q: &PageQuery) -> ConfigApiResult<(Option<Cursor>, i64)> {
    let limit = match q.limit.as_deref().filter(|s| !s.is_empty()) {
        Some(s) => Some(
            s.parse::<i64>()
                .map_err(|_| invalid("limit", "must be between 1 and 100"))?,
        ),
        None => None,
    };
    let limit = cursor::limit(limit)?;
    let after = match q.cursor.as_deref().filter(|s| !s.is_empty()) {
        Some(raw) => Some(Cursor::decode(raw)?),
        None => None,
    };
    Ok((after, limit))
}

fn served(s: Served, status: StatusCode) -> Response {
    let mut r = (status, s.json).into_response();
    let h = r.headers_mut();
    h.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    if let Ok(tag) = HeaderValue::from_str(&bundle::etag(&s.bundle_version)) {
        h.insert(header::ETAG, tag);
    }
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    r
}

// ---- device plane -----------------------------------------------------------------------

async fn device_bundle(
    device: DevicePrincipal,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ConfigApiResult<Response> {
    let mut conn = state.db.acquire().await?;
    let signer = state.config.bundle_signing.as_ref();
    let s = bundle::latest_for_org(&mut conn, signer, device.org_id)
        .await?
        .ok_or(AppError::NotFound)?;
    bundle::record_adoption(&mut conn, device.device_id, &s.bundle_version).await?;
    let tag = bundle::etag(&s.bundle_version);
    let hit = headers
        .get_all(header::IF_NONE_MATCH)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .any(|v| bundle::if_none_match_hits(v, &tag));
    if hit {
        let mut r = StatusCode::NOT_MODIFIED.into_response();
        if let Ok(tag) = HeaderValue::from_str(&tag) {
            r.headers_mut().insert(header::ETAG, tag);
        }
        r.headers_mut()
            .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
        return Ok(r);
    }
    Ok(served(s, StatusCode::OK))
}

// ---- documents --------------------------------------------------------------------------

async fn list_documents(
    _auth: Require<perm::ConfigRead>,
    State(state): State<AppState>,
    Path(raw_kind): Path<String>,
    query: Result<Query<PageQuery>, QueryRejection>,
) -> ConfigApiResult<Json<ConfigDocumentPage>> {
    let kind = kind(&raw_kind)?;
    let q = query_or_problem(query)?;
    let status =
        match q.status.as_deref().filter(|s| !s.is_empty()) {
            Some(s) => Some(status_from_str(s).ok_or_else(|| {
                invalid("status", "must be draft, candidate, published or retired")
            })?),
            None => None,
        };
    let (after, limit) = page_params(&q)?;
    let mut conn = state.db.acquire().await?;
    let (rows, next_cursor) =
        lifecycle::list(&mut conn, kind, status, after.as_ref(), limit).await?;
    let items = rows
        .iter()
        .map(lifecycle::DocRow::to_dto)
        .collect::<ConfigApiResult<Vec<_>>>()?;
    Ok(Json(ConfigDocumentPage {
        items,
        next_cursor,
        total: None,
    }))
}

async fn create_document(
    auth: Require<perm::ConfigGlobalManage>,
    meta: RequestMeta,
    State(state): State<AppState>,
    Path(raw_kind): Path<String>,
    ApiJson(req): ApiJson<CreateConfigDocumentRequest>,
) -> ConfigApiResult<(StatusCode, Json<ConfigDocument>)> {
    let kind = kind(&raw_kind)?;
    let mut tx = state.db.begin().await?;
    let row = lifecycle::create(
        &mut tx,
        &auth.principal,
        &meta,
        kind,
        &req.version,
        &req.body,
        req.notes.as_deref(),
    )
    .await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(row.to_dto()?)))
}

async fn get_document(
    _auth: Require<perm::ConfigRead>,
    State(state): State<AppState>,
    Path((raw_kind, version)): Path<(String, String)>,
) -> ConfigApiResult<Json<ConfigDocument>> {
    let kind = kind(&raw_kind)?;
    let mut conn = state.db.acquire().await?;
    let row = lifecycle::get(&mut conn, kind, &version).await?;
    Ok(Json(row.to_dto()?))
}

#[derive(Clone, Copy)]
enum Transition {
    Approve,
    Publish,
    Retire,
}

async fn transition(
    auth: Require<perm::ConfigGlobalManage>,
    meta: RequestMeta,
    state: AppState,
    raw_kind: String,
    version: String,
    step: Transition,
) -> ConfigApiResult<Json<ConfigDocument>> {
    let kind = kind(&raw_kind)?;
    let mut tx = state.db.begin().await?;
    let p = &auth.principal;
    let row = match step {
        Transition::Approve => lifecycle::approve(&mut tx, p, &meta, kind, &version).await,
        Transition::Publish => lifecycle::publish(&mut tx, p, &meta, kind, &version).await,
        Transition::Retire => lifecycle::retire(&mut tx, p, &meta, kind, &version).await,
    }?;
    tx.commit().await?;
    Ok(Json(row.to_dto()?))
}

async fn approve_document(
    auth: Require<perm::ConfigGlobalManage>,
    meta: RequestMeta,
    State(state): State<AppState>,
    Path((kind, version)): Path<(String, String)>,
) -> ConfigApiResult<Json<ConfigDocument>> {
    transition(auth, meta, state, kind, version, Transition::Approve).await
}

async fn publish_document(
    auth: Require<perm::ConfigGlobalManage>,
    meta: RequestMeta,
    State(state): State<AppState>,
    Path((kind, version)): Path<(String, String)>,
) -> ConfigApiResult<Json<ConfigDocument>> {
    transition(auth, meta, state, kind, version, Transition::Publish).await
}

async fn retire_document(
    auth: Require<perm::ConfigGlobalManage>,
    meta: RequestMeta,
    State(state): State<AppState>,
    Path((kind, version)): Path<(String, String)>,
) -> ConfigApiResult<Json<ConfigDocument>> {
    transition(auth, meta, state, kind, version, Transition::Retire).await
}

// ---- bundles ----------------------------------------------------------------------------

/// `POST /v1/config/bundles` body; empty = `{}` (the caller's org, no feature flags).
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct ComposeBundleRequest {
    /// Superadmin only: the org to compose for; omitted = the global default. For an org
    /// user it may only name the user's own org.
    #[serde(default)]
    org_id: Option<String>,
    #[serde(default)]
    feature_flags: BTreeMap<String, bool>,
}

/// `GET /v1/config/bundles` envelope (same shape as every `/v1` page).
#[derive(Debug, Serialize)]
struct ConfigBundlePage {
    items: Vec<ConfigBundleSummary>,
    next_cursor: Option<String>,
    total: Option<u64>,
}

async fn compose_bundle(
    auth: Require<perm::ConfigPublish>,
    meta: RequestMeta,
    State(state): State<AppState>,
    body: Result<Bytes, BytesRejection>,
) -> ConfigApiResult<Response> {
    let bytes = body_or_problem(body)?;
    let req: ComposeBundleRequest = json_or_default(&bytes)?;
    let p = &auth.principal;
    let requested = match req.org_id.as_deref() {
        Some(raw) => Some(Uuid::parse_str(raw).map_err(|_| AppError::NotFound)?),
        None => None,
    };
    let org = if p.is_superadmin() {
        requested
    } else {
        let own = p.org_id.ok_or(AppError::Forbidden)?;
        if requested.is_some_and(|r| r != own) {
            return Err(AppError::NotFound.into());
        }
        Some(own)
    };
    let mut tx = state.db.begin().await?;
    if let Some(org) = org {
        let exists = sqlx::query_scalar!(
            r#"SELECT EXISTS (SELECT 1 FROM organizations WHERE id = $1) AS "e!""#,
            org
        )
        .fetch_one(&mut *tx)
        .await?;
        if !exists {
            return Err(AppError::NotFound.into());
        }
    }
    let summary = bundle::compose(
        &mut tx,
        state.config.bundle_signing.as_ref(),
        p,
        &meta,
        org,
        req.feature_flags,
    )
    .await?;
    tx.commit().await?;
    let mut r = (StatusCode::CREATED, Json(&summary)).into_response();
    if let Ok(tag) = HeaderValue::from_str(&bundle::etag(&summary.bundle_version)) {
        r.headers_mut().insert(header::ETAG, tag);
    }
    Ok(r)
}

async fn list_bundles(
    auth: Require<perm::ConfigRead>,
    State(state): State<AppState>,
    query: Result<Query<PageQuery>, QueryRejection>,
) -> ConfigApiResult<Json<ConfigBundlePage>> {
    let q = query_or_problem(query)?;
    let (after, limit) = page_params(&q)?;
    let mut conn = state.db.acquire().await?;
    let (items, next_cursor) =
        bundle::list(&mut conn, &auth.principal, after.as_ref(), limit).await?;
    Ok(Json(ConfigBundlePage {
        items,
        next_cursor,
        total: None,
    }))
}

async fn get_bundle(
    auth: Require<perm::ConfigRead>,
    State(state): State<AppState>,
    Path(version): Path<String>,
) -> ConfigApiResult<Response> {
    let mut conn = state.db.acquire().await?;
    let s = bundle::by_version(
        &mut conn,
        state.config.bundle_signing.as_ref(),
        &auth.principal,
        &version,
    )
    .await?;
    Ok(served(s, StatusCode::OK))
}
