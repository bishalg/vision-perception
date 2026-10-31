//! owner: WP-C.10
//!
//! Clinical review and sign-off (SIGN-1..10; logic in [`crate::clinical`]):
//!
//! - `POST /v1/sessions/{id}/clinical-review` (`ClinicalReviewWrite`): claim; `201` new,
//!   `200` existing. Body `ClaimClinicalReviewRequest`, may be empty.
//! - `GET /v1/sessions/{id}/clinical-review`, `GET /v1/clinical-reviews/{id}`
//!   (`WorklistRead`): the review; content only with `ClinicalContentRead` (RBAC-4).
//! - `PATCH /v1/clinical-reviews/{id}` (`ClinicalReviewWrite`, `If-Match: "<row_version>"`
//!   required, `412 stale-version` on mismatch): draft save.
//! - `POST /v1/clinical-reviews/{id}/sign` (`ClinicalSign`), `.../cosign`
//!   (`ClinicalCosign`): `SignRequest` with step-up re-authentication.
//! - `POST /v1/clinical-reviews/{id}/amend` (`ClinicalReviewWrite`): new draft version.
//!
//! Every mutation answers the updated `ClinicalReview` with `ETag: "<row_version>"`. Routes
//! that read or write clinical content take [`ClinicalCrypto`] (503 without the KEK).

use std::sync::Arc;

use asd_contract::clinical::{
    AmendClinicalReviewRequest, ClaimClinicalReviewRequest, ClinicalReview, SignRequest,
    UpdateClinicalReviewRequest,
};
use axum::body::Bytes;
use axum::extract::rejection::JsonRejection;
use axum::extract::{Path, State};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use http::{header, HeaderMap, HeaderValue, StatusCode};
use uuid::Uuid;

use crate::app::AppState;
use crate::auth::extractors::{RequestMeta, Require, UserPrincipal};
use crate::auth::rbac::perm;
use crate::clinical::reauth::StepUpLimiter;
use crate::clinical::{review, sign};
use crate::crypto::ClinicalCrypto;
use crate::error::{AppError, AppResult};
use crate::problem::InvalidParam;
use crate::routes::review::json_or_problem;
use crate::routes::sessions::path_uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/sessions/{id}/clinical-review",
            get(get_by_session).post(claim),
        )
        .route("/clinical-reviews/{id}", get(get_one).patch(update))
        .route("/clinical-reviews/{id}/sign", post(sign_review))
        .route("/clinical-reviews/{id}/cosign", post(cosign_review))
        .route("/clinical-reviews/{id}/amend", post(amend_review))
        .layer(Extension(StepUpLimiter::new()))
}

fn respond(status: StatusCode, review: ClinicalReview) -> Response {
    let etag = HeaderValue::from_str(&format!("\"{}\"", review.row_version))
        .expect("digits are a valid header value");
    (status, [(header::ETAG, etag)], Json(review)).into_response()
}

async fn reply(
    state: &AppState,
    principal: &UserPrincipal,
    status: StatusCode,
    id: Uuid,
) -> AppResult<Response> {
    Ok(respond(
        status,
        review::read_model(state, principal, id).await?,
    ))
}

/// `If-Match: "<n>"` (also `W/"<n>"` or a bare `n`) as a row version.
fn if_match(headers: &HeaderMap) -> AppResult<i32> {
    let bad = || {
        AppError::Validation(vec![InvalidParam {
            name: "If-Match".into(),
            reason: "required: the review's row_version as \"<n>\"".into(),
        }])
    };
    let raw = headers
        .get(header::IF_MATCH)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(bad)?
        .trim();
    let raw = raw.strip_prefix("W/").unwrap_or(raw);
    let raw = raw
        .strip_prefix('"')
        .and_then(|r| r.strip_suffix('"'))
        .unwrap_or(raw);
    raw.parse::<i32>().ok().filter(|n| *n >= 1).ok_or_else(bad)
}

async fn claim(
    auth: Require<perm::ClinicalReviewWrite>,
    meta: RequestMeta,
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    body: Bytes,
) -> AppResult<Response> {
    let req: ClaimClinicalReviewRequest = if body.iter().all(u8::is_ascii_whitespace) {
        ClaimClinicalReviewRequest::default()
    } else {
        serde_json::from_slice(&body).map_err(|e| {
            AppError::Validation(vec![InvalidParam {
                name: "body".into(),
                reason: e.to_string(),
            }])
        })?
    };
    let (created, id) = review::claim(&state, &auth.principal, &meta, &session_id, req).await?;
    let status = if created {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };
    reply(&state, &auth.principal, status, id).await
}

async fn get_by_session(
    auth: Require<perm::WorklistRead>,
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> AppResult<Response> {
    let row = review::find_review_by_session(&state.db, &session_id)
        .await?
        .ok_or(AppError::NotFound)?;
    auth.ensure_org(row.org_id)?;
    reply(&state, &auth.principal, StatusCode::OK, row.id).await
}

async fn get_one(
    auth: Require<perm::WorklistRead>,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Response> {
    let id = path_uuid(&id)?;
    let row = review::find_review(&state.db, id)
        .await?
        .ok_or(AppError::NotFound)?;
    auth.ensure_org(row.org_id)?;
    reply(&state, &auth.principal, StatusCode::OK, id).await
}

async fn update(
    auth: Require<perm::ClinicalReviewWrite>,
    crypto: ClinicalCrypto,
    meta: RequestMeta,
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    body: Result<Json<UpdateClinicalReviewRequest>, JsonRejection>,
) -> AppResult<Response> {
    let id = path_uuid(&id)?;
    let req = json_or_problem(body)?;
    let version = if_match(&headers)?;
    review::save_draft(&state, &crypto, &auth.principal, &meta, id, version, req).await?;
    reply(&state, &auth.principal, StatusCode::OK, id).await
}

async fn sign_review(
    auth: Require<perm::ClinicalSign>,
    crypto: ClinicalCrypto,
    meta: RequestMeta,
    State(state): State<AppState>,
    Extension(limiter): Extension<Arc<StepUpLimiter>>,
    Path(id): Path<String>,
    body: Result<Json<SignRequest>, JsonRejection>,
) -> AppResult<Response> {
    let id = path_uuid(&id)?;
    let req = json_or_problem(body)?;
    sign::sign(&state, &crypto, &limiter, &auth.principal, &meta, id, req).await?;
    reply(&state, &auth.principal, StatusCode::OK, id).await
}

async fn cosign_review(
    auth: Require<perm::ClinicalCosign>,
    crypto: ClinicalCrypto,
    meta: RequestMeta,
    State(state): State<AppState>,
    Extension(limiter): Extension<Arc<StepUpLimiter>>,
    Path(id): Path<String>,
    body: Result<Json<SignRequest>, JsonRejection>,
) -> AppResult<Response> {
    let id = path_uuid(&id)?;
    let req = json_or_problem(body)?;
    sign::cosign(&state, &crypto, &limiter, &auth.principal, &meta, id, req).await?;
    reply(&state, &auth.principal, StatusCode::OK, id).await
}

async fn amend_review(
    auth: Require<perm::ClinicalReviewWrite>,
    crypto: ClinicalCrypto,
    meta: RequestMeta,
    State(state): State<AppState>,
    Path(id): Path<String>,
    body: Result<Json<AmendClinicalReviewRequest>, JsonRejection>,
) -> AppResult<Response> {
    let id = path_uuid(&id)?;
    let req = json_or_problem(body)?;
    sign::amend(&state, &crypto, &auth.principal, &meta, id, req).await?;
    reply(&state, &auth.principal, StatusCode::OK, id).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn if_match_forms() {
        let h = |v: &str| {
            let mut m = HeaderMap::new();
            m.insert(header::IF_MATCH, HeaderValue::from_str(v).unwrap());
            m
        };
        assert_eq!(if_match(&h("\"3\"")).unwrap(), 3);
        assert_eq!(if_match(&h("W/\"3\"")).unwrap(), 3);
        assert_eq!(if_match(&h("3")).unwrap(), 3);
        assert!(if_match(&h("\"x\"")).is_err());
        assert!(if_match(&h("\"0\"")).is_err());
        assert!(if_match(&HeaderMap::new()).is_err());
    }
}
