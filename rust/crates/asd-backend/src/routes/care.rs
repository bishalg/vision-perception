//! owner: WP-B.6
//!
//! Device-plane care routes (API-1, bearer = `dvc_` device token, org taken from the token):
//!
//! - `POST /v1/care/events`: idempotent batch, `201` / `200` replay.
//! - `GET /v1/participants/{token}/summary`: counts over the participant's care events. One
//!   route, two callers (the template cannot be registered twice): a `dvc_` device token
//!   names the participant by its token; a console user names it by participant id and needs
//!   `ClinicalContentRead` (owner, clinician; never superadmin, RBAC-3). The user read is
//!   audited (`participant.care_summary_read`) and uses the same builder.
//! - `POST /v1/participants/{token}/erasure`: stores an erasure request (`202`, or `200`
//!   with the open one) and withdraws consent; the deletion is the WP-B.9 job.
//!
//! The console's `GET /v1/orgs/{id}/participants` lives in `routes::participants`.

use asd_contract::care::{
    CareEventsRequest, CareEventsResponse, CareSummary, ErasureRequestResponse,
    IDEMPOTENCY_KEY_HEADER,
};
use axum::extract::{DefaultBodyLimit, FromRequestParts, Path, Request, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use http::{HeaderMap, StatusCode};

use crate::app::AppState;
use crate::audit::AuditEvent;
use crate::auth::device_token;
use crate::auth::extractors::{bearer, ApiJson, DevicePrincipal, RequestMeta, Require};
use crate::auth::rbac::perm;
use crate::care;
use crate::error::{AppError, AppResult};
use crate::ingest::validate::is_safe_id;
use crate::routes::participant_detail::participant_org;

/// Largest accepted `POST /v1/care/events` body (500 events of modest payloads).
pub const MAX_CARE_BODY_BYTES: usize = 1024 * 1024;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/care/events",
            post(ingest).layer(DefaultBodyLimit::max(MAX_CARE_BODY_BYTES)),
        )
        .route("/participants/{token}/summary", get(summary))
        .route("/participants/{token}/erasure", post(erasure))
}

/// A token that cannot be a stored one names no participant (404).
fn token_or_404(token: &str) -> AppResult<()> {
    if is_safe_id(token) {
        Ok(())
    } else {
        Err(AppError::NotFound)
    }
}

async fn ingest(
    device: DevicePrincipal,
    State(state): State<AppState>,
    meta: RequestMeta,
    headers: HeaderMap,
    ApiJson(req): ApiJson<CareEventsRequest>,
) -> AppResult<(StatusCode, Json<CareEventsResponse>)> {
    let key = headers
        .get(IDEMPOTENCY_KEY_HEADER)
        .and_then(|v| v.to_str().ok());
    let (status, body) = care::ingest(&state, device, &meta, key, &req).await?;
    Ok((status, Json(body)))
}

async fn summary(
    State(state): State<AppState>,
    meta: RequestMeta,
    Path(path_id): Path<String>,
    request: Request,
) -> AppResult<Json<CareSummary>> {
    let (mut parts, _) = request.into_parts();
    let is_device = bearer(&parts.headers).is_some_and(|t| t.starts_with(device_token::PREFIX));
    if is_device {
        let device = DevicePrincipal::from_request_parts(&mut parts, &state).await?;
        token_or_404(&path_id)?;
        return Ok(Json(
            care::summary(&state.db, device.org_id, &path_id).await?,
        ));
    }
    let auth = Require::<perm::ClinicalContentRead>::from_request_parts(&mut parts, &state).await?;
    let (id, org) = participant_org(&state.db, &path_id).await?;
    auth.ensure_org(org)?;
    let token = sqlx::query_scalar!("SELECT token FROM participants WHERE id = $1", id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;
    let body = care::summary(&state.db, org, &token).await?;
    let mut tx = state.db.begin().await?;
    AuditEvent::user(
        &auth.principal,
        "participant.care_summary_read",
        "participant",
        id,
    )
    .org(org)
    .meta(&meta)
    .record(&mut tx)
    .await?;
    tx.commit().await?;
    Ok(Json(body))
}

async fn erasure(
    device: DevicePrincipal,
    State(state): State<AppState>,
    meta: RequestMeta,
    Path(token): Path<String>,
) -> AppResult<(StatusCode, Json<ErasureRequestResponse>)> {
    token_or_404(&token)?;
    let (status, body) = care::request_erasure(&state, device, &meta, &token).await?;
    Ok((status, Json(body)))
}
