//! owner: WP-B.2
//!
//! Org user management (API-2):
//!
//! - `GET /v1/orgs/{id}/users` -> `OrgUserPage` (newest first, `cursor`/`limit`);
//! - `POST /v1/orgs/{id}/users` `CreateUserRequest` -> `201 OrgUser`;
//! - `PATCH /v1/orgs/{id}/users/{user_id}` `UpdateUserRequest` -> `200 OrgUser`.
//!
//! Rules on top of the matrix (`UserRead`/`UserManage`): an actor grants and edits roles
//! only up to its own rank (admins never create or touch owners); `superadmin` is not an
//! org role; nobody changes their own role or status; the last active owner of an org
//! cannot be demoted or disabled (`409 last-owner`); disabling a user revokes their
//! refresh tokens (and the access-token extractor refuses them at once). `invited` users
//! are listed as such and cannot sign in; `PATCH` sets only `active` or `disabled`.

use asd_contract::api_v1::{
    CreateUserRequest, OrgUser, OrgUserPage, Role, UpdateUserRequest, UserStatus,
};
use axum::extract::{Path, State};
use axum::routing::{get, patch};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use http::StatusCode;
use serde_json::json;
use uuid::Uuid;

use super::auth::normalize_email;
use super::orgs::listing::{self, rfc3339, ListParams};
use crate::app::AppState;
use crate::audit::AuditEvent;
use crate::auth::extractors::{ApiJson, RequestMeta, Require};
use crate::auth::rbac::{self, perm};
use crate::auth::{password, refresh};
use crate::error::{AppError, AppResult};
use crate::problem::InvalidParam;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/orgs/{id}/users", get(list).post(create))
        .route("/orgs/{id}/users/{user_id}", patch(update))
}

pub const MAX_EMAIL_LEN: usize = 254;

fn status_str(s: UserStatus) -> &'static str {
    match s {
        UserStatus::Active => "active",
        UserStatus::Disabled => "disabled",
        UserStatus::Invited => "invited",
    }
}

/// `users.status` (CHECK: invited, active, disabled). An unknown value cannot sign in,
/// so it reads as `disabled`.
fn status_from_db(s: &str) -> UserStatus {
    match s {
        "active" => UserStatus::Active,
        "invited" => UserStatus::Invited,
        _ => UserStatus::Disabled,
    }
}

fn org_user(
    id: Uuid,
    org: Uuid,
    email: String,
    role: &str,
    status: &str,
    last_login_at: Option<DateTime<Utc>>,
) -> AppResult<OrgUser> {
    Ok(OrgUser {
        id: id.to_string(),
        email,
        role: rbac::role_from_str(role)
            .ok_or_else(|| AppError::Internal(format!("unknown role in database: {role}")))?,
        org_id: org.to_string(),
        status: status_from_db(status),
        last_login_at: last_login_at.map(rfc3339),
    })
}

/// A plausible address: one `@`, non-empty local part, a dotted domain, no whitespace.
pub fn valid_email(email: &str) -> bool {
    let Some((local, domain)) = email.split_once('@') else {
        return false;
    };
    email.len() <= MAX_EMAIL_LEN
        && !local.is_empty()
        && !domain.contains('@')
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && !email.chars().any(char::is_whitespace)
}

fn not_an_org_role() -> AppError {
    AppError::Validation(vec![InvalidParam {
        name: "role".into(),
        reason: "superadmin is a global role, not an org role".into(),
    }])
}

async fn list(
    State(state): State<AppState>,
    auth: Require<perm::UserRead>,
    page: ListParams,
) -> AppResult<Json<OrgUserPage>> {
    let org = auth.org();
    let after = page.cursor_ts()?;
    let rows = sqlx::query!(
        "SELECT id, email, role, status, last_login_at, created_at
         FROM users
         WHERE org_id = $1
           AND ($2::timestamptz IS NULL OR (created_at, id) < ($2, $3))
         ORDER BY created_at DESC, id DESC
         LIMIT $4",
        org,
        after.map(|a| a.0),
        after.map(|a| a.1),
        i64::from(page.limit) + 1
    )
    .fetch_all(&state.db)
    .await?;
    let (rows, more) = listing::split_page(rows, page.limit);
    let next_cursor = more
        .then(|| {
            rows.last()
                .map(|r| listing::encode_ts_cursor(r.created_at, r.id))
        })
        .flatten();
    let items = rows
        .into_iter()
        .map(|r| org_user(r.id, org, r.email, &r.role, &r.status, r.last_login_at))
        .collect::<AppResult<Vec<_>>>()?;
    Ok(Json(OrgUserPage {
        items,
        next_cursor,
        total: None,
    }))
}

async fn create(
    State(state): State<AppState>,
    auth: Require<perm::UserManage>,
    meta: RequestMeta,
    ApiJson(req): ApiJson<CreateUserRequest>,
) -> AppResult<(StatusCode, Json<OrgUser>)> {
    let org = auth.org();
    if req.role == Role::Superadmin {
        return Err(not_an_org_role());
    }
    let email = normalize_email(&req.email);
    let mut errors = Vec::new();
    if !valid_email(&email) {
        errors.push(InvalidParam {
            name: "email".into(),
            reason: "must be an e-mail address".into(),
        });
    }
    if let Err(e) = password::validate_new(&req.password) {
        errors.push(e);
    }
    if !errors.is_empty() {
        return Err(AppError::Validation(errors));
    }
    if !rbac::can_assign(auth.principal.role, req.role) {
        return Err(AppError::Forbidden);
    }

    let pw_hash = password::hash_blocking(req.password).await?;
    let role = rbac::role_as_str(req.role);
    let mut tx = state.db.begin().await?;
    let row = sqlx::query!(
        "INSERT INTO users (org_id, email, pw_hash, role, status)
         VALUES ($1, $2, $3, $4, 'active')
         ON CONFLICT ((lower(email))) DO NOTHING
         RETURNING id, email, role, status, last_login_at",
        org,
        email,
        pw_hash,
        role
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::Conflict {
        slug: "email-taken",
        detail: "a user with this e-mail address already exists".into(),
    })?;
    AuditEvent::user(&auth.principal, "user.create", "user", row.id)
        .org(org)
        .after(json!({"email": row.email, "role": row.role, "status": row.status}))
        .meta(&meta)
        .record(&mut tx)
        .await?;
    tx.commit().await?;
    Ok((
        StatusCode::CREATED,
        Json(org_user(
            row.id,
            org,
            row.email,
            &row.role,
            &row.status,
            row.last_login_at,
        )?),
    ))
}

async fn update(
    State(state): State<AppState>,
    auth: Require<perm::UserManage>,
    Path((_, user_id)): Path<(String, String)>,
    meta: RequestMeta,
    ApiJson(req): ApiJson<UpdateUserRequest>,
) -> AppResult<Json<OrgUser>> {
    let org = auth.org();
    let user_id = Uuid::parse_str(&user_id).map_err(|_| AppError::NotFound)?;
    if req.role == Some(Role::Superadmin) {
        return Err(not_an_org_role());
    }
    // `invited` is set by an invitation flow, not by an edit.
    if req.status == Some(UserStatus::Invited) {
        return Err(AppError::Validation(vec![InvalidParam {
            name: "status".into(),
            reason: "must be active or disabled".into(),
        }]));
    }

    let mut tx = state.db.begin().await?;
    // Serialises owner changes in this org (last-owner check below).
    sqlx::query!("SELECT id FROM organizations WHERE id = $1 FOR UPDATE", org)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(AppError::NotFound)?;
    let target = sqlx::query!(
        "SELECT email, role, status, last_login_at FROM users
         WHERE id = $1 AND org_id = $2 FOR UPDATE",
        user_id,
        org
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::NotFound)?;
    let current_role = rbac::role_from_str(&target.role)
        .ok_or_else(|| AppError::Internal("unknown role in database".into()))?;

    if user_id == auth.principal.user_id {
        return Err(AppError::Forbidden);
    }
    let actor = auth.principal.role;
    if !rbac::can_assign(actor, current_role) {
        return Err(AppError::Forbidden);
    }
    let new_role = req.role.unwrap_or(current_role);
    if !rbac::can_assign(actor, new_role) {
        return Err(AppError::Forbidden);
    }
    let new_status: &str = match req.status {
        Some(s) => status_str(s),
        None => &target.status,
    };

    let was_active_owner = current_role == Role::Owner && target.status == "active";
    let stays_active_owner = new_role == Role::Owner && new_status == "active";
    if was_active_owner && !stays_active_owner {
        let others = sqlx::query_scalar!(
            r#"SELECT count(*) AS "n!" FROM users
               WHERE org_id = $1 AND role = 'owner' AND status = 'active' AND id <> $2"#,
            org,
            user_id
        )
        .fetch_one(&mut *tx)
        .await?;
        if others == 0 {
            return Err(AppError::Conflict {
                slug: "last-owner",
                detail: "an organisation keeps at least one active owner".into(),
            });
        }
    }

    let role_str = rbac::role_as_str(new_role);
    let changed = role_str != target.role || new_status != target.status;
    if changed {
        sqlx::query!(
            "UPDATE users SET role = $2, status = $3 WHERE id = $1",
            user_id,
            role_str,
            new_status
        )
        .execute(&mut *tx)
        .await?;
        if new_status != "active" {
            refresh::revoke_user(&mut tx, user_id).await?;
        }
        AuditEvent::user(&auth.principal, "user.update", "user", user_id)
            .org(org)
            .before(json!({"role": target.role, "status": target.status}))
            .after(json!({"role": role_str, "status": new_status}))
            .meta(&meta)
            .record(&mut tx)
            .await?;
    }
    tx.commit().await?;
    Ok(Json(org_user(
        user_id,
        org,
        target.email,
        role_str,
        new_status,
        target.last_login_at,
    )?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn email_shape() {
        for ok in ["a@b.org", "first.last+tag@sub.example.org"] {
            assert!(valid_email(ok), "{ok}");
        }
        for bad in [
            "",
            "a",
            "a@b",
            "@b.org",
            "a@@b.org",
            "a b@c.org",
            "a@.org",
            "a@b.org.",
        ] {
            assert!(!valid_email(bad), "{bad}");
        }
        assert!(!valid_email(&format!("{}@b.org", "a".repeat(250))));
    }
}
