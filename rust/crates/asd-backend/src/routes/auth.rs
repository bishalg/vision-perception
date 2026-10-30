//! owner: WP-B.2
//!
//! User-plane session endpoints (AUTH-1, AUTH-2), shaped for the console
//! (`apps/curv_console/src/lib/api/live.ts`):
//!
//! - `POST /v1/auth/login` `{email, password}` -> `200 TokenPair`; `401` for any wrong
//!   credential or a disabled account (one answer, constant time); `429` + `Retry-After`
//!   beyond 10 attempts per minute per client IP.
//! - `POST /v1/auth/refresh` `{refresh_token}` -> `200 TokenPair` with a rotated refresh
//!   token; reuse of a rotated-out token revokes the family and answers `401`.
//! - `POST /v1/auth/logout` `{refresh_token}` -> `204`, revoking the token's family.

use std::net::IpAddr;
use std::sync::Arc;

use asd_contract::api_v1::{LoginRequest, RefreshRequest, Role, TokenPair, User};
use axum::extract::State;
use axum::routing::post;
use axum::{Extension, Json, Router};
use http::StatusCode;
use serde_json::json;
use uuid::Uuid;

use crate::app::AppState;
use crate::audit::AuditEvent;
use crate::auth::extractors::{ApiJson, RequestMeta};
use crate::auth::jwt::{JwtKeys, ACCESS_TOKEN_TTL_SECS};
use crate::auth::refresh::{self, Rotation};
use crate::auth::{password, rbac, RateLimiter};
use crate::error::{AppError, AppResult};
use crate::problem::InvalidParam;

/// AUTH-1: login attempts per client IP per minute.
pub const LOGIN_ATTEMPTS_PER_MINUTE: u32 = 10;

/// Per-IP login limiter (one per router, shared by its clones).
#[derive(Debug)]
pub struct LoginLimiter(pub RateLimiter<Option<IpAddr>>);

pub fn router() -> Router<AppState> {
    let limiter = Arc::new(LoginLimiter(RateLimiter::per_minute(
        LOGIN_ATTEMPTS_PER_MINUTE,
    )));
    Router::new()
        .route("/auth/login", post(login))
        .route("/auth/refresh", post(refresh_tokens))
        .route("/auth/logout", post(logout))
        .layer(Extension(limiter))
}

fn keys(state: &AppState) -> AppResult<&JwtKeys> {
    state.config.auth.jwt.as_ref().ok_or_else(|| {
        tracing::error!("ASD_JWT_SIGNING_KEY is not configured; user plane unavailable");
        AppError::Unavailable
    })
}

/// Lower-cased, trimmed; `users_email_key` is unique on `lower(email)`.
pub fn normalize_email(email: &str) -> String {
    email.trim().to_lowercase()
}

fn token_pair(
    keys: &JwtKeys,
    user_id: Uuid,
    org_id: Option<Uuid>,
    role: Role,
    email: String,
    refresh_token: String,
) -> AppResult<TokenPair> {
    let (access_token, _) = keys.issue(user_id, org_id, role)?;
    Ok(TokenPair {
        access_token,
        refresh_token,
        expires_in: ACCESS_TOKEN_TTL_SECS,
        user: User {
            id: user_id.to_string(),
            email,
            role,
            // `null` for the global superadmin, who has no org.
            org_id: org_id.map(|o| o.to_string()),
        },
    })
}

async fn login(
    State(state): State<AppState>,
    Extension(limiter): Extension<Arc<LoginLimiter>>,
    meta: RequestMeta,
    ApiJson(req): ApiJson<LoginRequest>,
) -> AppResult<Json<TokenPair>> {
    limiter
        .0
        .check(&meta.ip)
        .map_err(|retry_after_secs| AppError::RateLimited { retry_after_secs })?;
    let keys = keys(&state)?;
    let email = normalize_email(&req.email);
    if email.is_empty() || req.password.is_empty() {
        return Err(AppError::Validation(vec![InvalidParam {
            name: if email.is_empty() {
                "email"
            } else {
                "password"
            }
            .into(),
            reason: "is required".into(),
        }]));
    }

    let row = sqlx::query!(
        "SELECT id, org_id, email, pw_hash, role, status FROM users WHERE lower(email) = $1",
        email
    )
    .fetch_optional(&state.db)
    .await?;
    // Always one Argon2 verification, against a dummy hash when there is no such user.
    let verified =
        password::verify_or_dummy_blocking(row.as_ref().map(|r| r.pw_hash.clone()), req.password)
            .await;
    let user = match row {
        Some(r) if verified && r.status == "active" => r,
        _ => return Err(AppError::Unauthorized),
    };
    let role = rbac::role_from_str(&user.role).ok_or(AppError::Unauthorized)?;

    let mut tx = state.db.begin().await?;
    sqlx::query!(
        "UPDATE users SET last_login_at = now() WHERE id = $1",
        user.id
    )
    .execute(&mut *tx)
    .await?;
    let family = Uuid::new_v4();
    let refresh_token = refresh::issue(&mut tx, user.id, family).await?;
    AuditEvent::user_id(user.id, user.org_id, "auth.login", "user", user.id)
        .after(json!({"refresh_family": family}))
        .meta(&meta)
        .record(&mut tx)
        .await?;
    tx.commit().await?;
    Ok(Json(token_pair(
        keys,
        user.id,
        user.org_id,
        role,
        user.email,
        refresh_token,
    )?))
}

async fn refresh_tokens(
    State(state): State<AppState>,
    meta: RequestMeta,
    ApiJson(req): ApiJson<RefreshRequest>,
) -> AppResult<Json<TokenPair>> {
    let keys = keys(&state)?;
    let mut tx = state.db.begin().await?;
    match refresh::rotate(&mut tx, &req.refresh_token).await? {
        Rotation::Invalid => Err(AppError::Unauthorized),
        Rotation::Reused { user_id, family_id } => {
            let org = sqlx::query_scalar!("SELECT org_id FROM users WHERE id = $1", user_id)
                .fetch_optional(&mut *tx)
                .await?
                .flatten();
            tracing::warn!(%family_id, "refresh token reuse detected; family revoked");
            let mut event =
                AuditEvent::system("auth.refresh_reuse_detected", "refresh_family", family_id)
                    .after(json!({"user_id": user_id}))
                    .meta(&meta);
            event.org_id = org;
            event.record(&mut tx).await?;
            // Commit the revocation, then refuse.
            tx.commit().await?;
            Err(AppError::Unauthorized)
        }
        Rotation::Rotated {
            user_id,
            family_id,
            token,
        } => {
            let user = sqlx::query!(
                "SELECT org_id, email, role, status FROM users WHERE id = $1",
                user_id
            )
            .fetch_one(&mut *tx)
            .await?;
            let role = rbac::role_from_str(&user.role);
            let Some(role) = role.filter(|_| user.status == "active") else {
                refresh::revoke_family(&mut tx, family_id).await?;
                tx.commit().await?;
                return Err(AppError::Unauthorized);
            };
            AuditEvent::user_id(
                user_id,
                user.org_id,
                "auth.refresh",
                "refresh_family",
                family_id,
            )
            .meta(&meta)
            .record(&mut tx)
            .await?;
            tx.commit().await?;
            Ok(Json(token_pair(
                keys,
                user_id,
                user.org_id,
                role,
                user.email,
                token,
            )?))
        }
    }
}

async fn logout(
    State(state): State<AppState>,
    meta: RequestMeta,
    ApiJson(req): ApiJson<RefreshRequest>,
) -> AppResult<StatusCode> {
    let mut tx = state.db.begin().await?;
    if let Some((user_id, family_id)) =
        refresh::revoke_family_of(&mut tx, &req.refresh_token).await?
    {
        let org = sqlx::query_scalar!("SELECT org_id FROM users WHERE id = $1", user_id)
            .fetch_optional(&mut *tx)
            .await?
            .flatten();
        AuditEvent::user_id(user_id, org, "auth.logout", "refresh_family", family_id)
            .meta(&meta)
            .record(&mut tx)
            .await?;
    }
    tx.commit().await?;
    // Unknown or already revoked tokens get the same answer (no token oracle).
    Ok(StatusCode::NO_CONTENT)
}
