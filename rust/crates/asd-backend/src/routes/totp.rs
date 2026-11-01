//! owner: WP-B.11
//!
//! Self-service TOTP enrolment (AUTH-6..8, ADR-0009) for any signed-in console user:
//!
//! - `POST /v1/users/me/totp/enrol` `TotpEnrolRequest {password}` -> `200
//!   TotpEnrolResponse {secret, otpauth_url, expires_at}`: a new pending seed, valid 10
//!   minutes (a second call replaces it). `401 invalid-credentials` (audited
//!   `auth.password_failed`), `409 totp-already-enrolled`, `503 clinical-crypto-unavailable`
//!   without `ASD_DATA_KEK`;
//! - `POST /v1/users/me/totp/confirm` `TotpConfirmRequest {totp_code}` -> `200 TotpStatus`:
//!   the pending seed becomes the user's seed. `401 totp-invalid` (audited
//!   `auth.totp_enrol_failed`), `410 totp-enrolment-expired` (an expired pending seed is
//!   cleared), `409 totp-already-enrolled` once confirmed (a repeated or concurrent confirm);
//! - `DELETE /v1/users/me/totp` `TotpDisableRequest {password, totp_code}` -> `200
//!   TotpStatus`: both factors required. `401 invalid-credentials`, `401 totp-required`,
//!   `401 totp-invalid` (audited `auth.totp_disable_failed`), `404 totp-not-enrolled`.
//!
//! The three routes share one limiter: 5 requests per user per minute (`429`). Passwords,
//! seeds and codes are never logged; the code field is named `totp_code`, which the log
//! formatter redacts (OBS-1). The seed is in clear only in the enrol response. Storage and
//! the state machine are in [`crate::auth::totp`].

use std::sync::Arc;

use asd_contract::api_v1::{
    TotpConfirmRequest, TotpDisableRequest, TotpEnrolRequest, TotpEnrolResponse, TotpStatus,
};
use asd_contract::problem::{
    INVALID_CREDENTIALS, TOTP_ENROLMENT_EXPIRED, TOTP_INVALID, TOTP_NOT_ENROLLED, TOTP_REQUIRED,
};
use axum::extract::State;
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, post};
use axum::{Extension, Json, Router};
use http::StatusCode;
use uuid::Uuid;
use zeroize::Zeroizing;

use super::orgs::listing::rfc3339;
use crate::app::AppState;
use crate::audit::AuditEvent;
use crate::auth::extractors::{ApiJson, RequestMeta, UserPrincipal};
use crate::auth::totp::{self, ConfirmOutcome, DisableOutcome};
use crate::auth::{password, RateLimiter};
use crate::crypto::CryptoError;
use crate::error::{AppError, AppResult};
use crate::problem::Problem;

/// AUTH-6/7: requests per user per minute, across enrol, confirm and disable.
pub const TOTP_REQUESTS_PER_MINUTE: u32 = 5;

/// The per-user limiter of the TOTP routes (one per router, shared by its clones).
#[derive(Debug)]
pub struct TotpLimiter(pub RateLimiter<Uuid>);

pub fn router() -> Router<AppState> {
    let limiter = Arc::new(TotpLimiter(RateLimiter::per_minute(
        TOTP_REQUESTS_PER_MINUTE,
    )));
    Router::new()
        .route("/users/me/totp/enrol", post(enrol))
        .route("/users/me/totp/confirm", post(confirm))
        .route("/users/me/totp", delete(disable))
        .layer(Extension(limiter))
}

fn limit(state: &AppState, limiter: &TotpLimiter, user_id: Uuid) -> AppResult<()> {
    limiter
        .0
        .check_at(&user_id, state.clock.now())
        .map_err(|retry_after_secs| AppError::RateLimited { retry_after_secs })
}

/// The caller's password, verified like login (Argon2id, one verification even for a
/// vanished or disabled user). A mismatch is audited and answers `401 invalid-credentials`.
async fn check_password(
    state: &AppState,
    principal: &UserPrincipal,
    meta: &RequestMeta,
    password_plain: String,
) -> AppResult<()> {
    let pw_hash = sqlx::query_scalar!(
        "SELECT pw_hash FROM users WHERE id = $1 AND status = 'active'",
        principal.user_id
    )
    .fetch_optional(&state.db)
    .await?;
    let active = pw_hash.is_some();
    if password::verify_or_dummy_blocking(pw_hash, password_plain).await && active {
        return Ok(());
    }
    let mut tx = state.db.begin().await?;
    AuditEvent::user(principal, "auth.password_failed", "user", principal.user_id)
        .meta(meta)
        .record(&mut tx)
        .await?;
    tx.commit().await?;
    Err(AppError::UnauthorizedWith {
        slug: INVALID_CREDENTIALS,
        detail: "the password is not correct".into(),
    })
}

async fn audit_failure(
    state: &AppState,
    principal: &UserPrincipal,
    meta: &RequestMeta,
    action: &str,
) -> AppResult<()> {
    let mut tx = state.db.begin().await?;
    AuditEvent::user(principal, action, "user", principal.user_id)
        .meta(meta)
        .record(&mut tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

fn invalid_code() -> AppError {
    AppError::UnauthorizedWith {
        slug: TOTP_INVALID,
        detail: "the authenticator code is not correct or was already used".into(),
    }
}

/// `POST /v1/users/me/totp/enrol` (AUTH-6).
async fn enrol(
    State(state): State<AppState>,
    Extension(limiter): Extension<Arc<TotpLimiter>>,
    principal: UserPrincipal,
    meta: RequestMeta,
    ApiJson(req): ApiJson<TotpEnrolRequest>,
) -> AppResult<Json<TotpEnrolResponse>> {
    limit(&state, &limiter, principal.user_id)?;
    let keyring = state
        .keyring
        .clone()
        .ok_or(AppError::from(CryptoError::Unavailable))?;
    check_password(&state, &principal, &meta, req.password).await?;
    let audit = AuditEvent::user(
        &principal,
        "user.totp_enrol_started",
        "user",
        principal.user_id,
    )
    .meta(&meta);
    let pending = totp::start_enrolment(&state.db, &keyring, principal.user_id, audit).await?;
    Ok(Json(TotpEnrolResponse {
        secret: pending.secret.as_str().to_owned(),
        otpauth_url: pending.otpauth_url,
        expires_at: rfc3339(pending.expires_at),
    }))
}

/// `POST /v1/users/me/totp/confirm` (AUTH-7).
async fn confirm(
    State(state): State<AppState>,
    Extension(limiter): Extension<Arc<TotpLimiter>>,
    principal: UserPrincipal,
    meta: RequestMeta,
    ApiJson(req): ApiJson<TotpConfirmRequest>,
) -> AppResult<Json<TotpStatus>> {
    let code = Zeroizing::new(req.totp_code);
    limit(&state, &limiter, principal.user_id)?;
    let keyring = state
        .keyring
        .clone()
        .ok_or(AppError::from(CryptoError::Unavailable))?;
    let audit =
        AuditEvent::user(&principal, "user.totp_enrolled", "user", principal.user_id).meta(&meta);
    match totp::confirm_enrolment(&state.db, &keyring, principal.user_id, &code, audit).await? {
        ConfirmOutcome::Enrolled => Ok(Json(TotpStatus {
            totp_enrolled: true,
        })),
        ConfirmOutcome::InvalidCode => {
            audit_failure(&state, &principal, &meta, "auth.totp_enrol_failed").await?;
            Err(invalid_code())
        }
        ConfirmOutcome::Expired => Err(AppError::Gone {
            slug: TOTP_ENROLMENT_EXPIRED,
            detail: "no enrolment is pending or it expired; start again".into(),
        }),
    }
}

/// `DELETE /v1/users/me/totp` (AUTH-8).
async fn disable(
    State(state): State<AppState>,
    Extension(limiter): Extension<Arc<TotpLimiter>>,
    principal: UserPrincipal,
    meta: RequestMeta,
    ApiJson(req): ApiJson<TotpDisableRequest>,
) -> AppResult<Response> {
    let code = req.totp_code.map(Zeroizing::new);
    limit(&state, &limiter, principal.user_id)?;
    check_password(&state, &principal, &meta, req.password).await?;
    let audit =
        AuditEvent::user(&principal, "user.totp_disabled", "user", principal.user_id).meta(&meta);
    let outcome = totp::disable(
        &state.db,
        state.keyring.as_deref(),
        principal.user_id,
        code.as_deref().map(String::as_str),
        audit,
    )
    .await?;
    match outcome {
        DisableOutcome::Disabled => Ok(Json(TotpStatus {
            totp_enrolled: false,
        })
        .into_response()),
        DisableOutcome::NotEnrolled => Ok(Problem::new(StatusCode::NOT_FOUND, TOTP_NOT_ENROLLED)
            .with_detail("TOTP is not enabled for this account")
            .into_response()),
        DisableOutcome::CodeRequired => Err(AppError::UnauthorizedWith {
            slug: TOTP_REQUIRED,
            detail: "enter the code from your authenticator app".into(),
        }),
        DisableOutcome::InvalidCode => {
            audit_failure(&state, &principal, &meta, "auth.totp_disable_failed").await?;
            Err(invalid_code())
        }
    }
}
