//! owner: WP-B.6
//!
//! Device-plane care routes (API-1, bearer = `dvc_` device token, org taken from the token):
//!
//! - `POST /v1/care/events`: idempotent batch, `201` / `200` replay.
//! - `GET /v1/participants/{token}/summary`: counts over the participant's care events.
//! - `POST /v1/participants/{token}/erasure`: stores an erasure request (`202`, or `200`
//!   with the open one) and withdraws consent; the deletion is the WP-B.9 job.
//!
//! The console's `GET /v1/orgs/{id}/participants` lives in `routes::participants`.

use asd_contract::care::{
    CareEventsRequest, CareEventsResponse, CareSummary, ErasureRequestResponse,
    IDEMPOTENCY_KEY_HEADER,
};
use axum::extract::{DefaultBodyLimit, Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use http::{HeaderMap, StatusCode};

use crate::app::AppState;
use crate::auth::extractors::{ApiJson, DevicePrincipal, RequestMeta};
use crate::care;
use crate::error::{AppError, AppResult};
use crate::ingest::validate::is_safe_id;

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
    device: DevicePrincipal,
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> AppResult<Json<CareSummary>> {
    token_or_404(&token)?;
    Ok(Json(care::summary(&state.db, device.org_id, &token).await?))
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
