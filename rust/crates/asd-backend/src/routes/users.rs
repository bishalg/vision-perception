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
//!
//! Signer profile (WP-C.9, NOTE-6): `display_name` (<= 120 characters) and
//! `professional_registration` (<= 60, e.g. "NMC 12345") on create and update (by
//! `UserManage`), and by the user through `GET/PATCH /v1/users/me` (any signed-in user,
//! profile fields only). Staff data, stored in clear; `""` clears a field.

use asd_contract::api_v1::{
    CreateUserRequest, OrgUser, OrgUserPage, Role, UpdateUserRequest, UserStatus,
};
use asd_contract::clinical::{
    UpdateProfileRequest, UserProfile, MAX_DISPLAY_NAME_CHARS, MAX_REGISTRATION_CHARS,
};
use axum::extract::{Path, State};
use axum::routing::{get, patch};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use http::StatusCode;
use serde_json::{json, Value};
use uuid::Uuid;

use super::auth::normalize_email;
use super::orgs::listing::{self, rfc3339, ListParams};
use crate::app::AppState;
use crate::audit::AuditEvent;
use crate::auth::extractors::{ApiJson, RequestMeta, Require, UserPrincipal};
use crate::auth::rbac::{self, perm};
use crate::auth::{password, refresh};
use crate::error::{AppError, AppResult};
use crate::problem::InvalidParam;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/orgs/{id}/users", get(list).post(create))
        .route("/orgs/{id}/users/{user_id}", patch(update))
        .route("/users/me", get(get_me).patch(patch_me))
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

/// The NOTE-6 profile columns of one user.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Profile {
    pub display_name: Option<String>,
    pub professional_registration: Option<String>,
}

#[allow(clippy::too_many_arguments)]
fn org_user(
    id: Uuid,
    org: Uuid,
    email: String,
    role: &str,
    status: &str,
    last_login_at: Option<DateTime<Utc>>,
    profile: Profile,
) -> AppResult<OrgUser> {
    Ok(OrgUser {
        id: id.to_string(),
        email,
        role: rbac::role_from_str(role)
            .ok_or_else(|| AppError::Internal(format!("unknown role in database: {role}")))?,
        org_id: org.to_string(),
        status: status_from_db(status),
        last_login_at: last_login_at.map(rfc3339),
        display_name: profile.display_name,
        professional_registration: profile.professional_registration,
    })
}

/// One profile field from a request: `None` = unchanged, `Some(None)` = clear (`""` after
/// trimming), `Some(Some(v))` = set to the trimmed value. At most `max` characters, no
/// control characters (the value is printed on signature blocks).
fn profile_field(
    name: &str,
    value: Option<&str>,
    max: usize,
    errors: &mut Vec<InvalidParam>,
) -> Option<Option<String>> {
    let v = value?.trim();
    if v.is_empty() {
        return Some(None);
    }
    if v.chars().count() > max || v.chars().any(char::is_control) {
        errors.push(InvalidParam {
            name: name.into(),
            reason: format!("must be at most {max} characters without line breaks"),
        });
        return None;
    }
    Some(Some(v.to_owned()))
}

/// Validated profile changes of a request.
#[derive(Debug, Default)]
struct ProfilePatch {
    display_name: Option<Option<String>>,
    professional_registration: Option<Option<String>>,
}

impl ProfilePatch {
    fn parse(
        display_name: Option<&str>,
        registration: Option<&str>,
        errors: &mut Vec<InvalidParam>,
    ) -> Self {
        Self {
            display_name: profile_field(
                "display_name",
                display_name,
                MAX_DISPLAY_NAME_CHARS,
                errors,
            ),
            professional_registration: profile_field(
                "professional_registration",
                registration,
                MAX_REGISTRATION_CHARS,
                errors,
            ),
        }
    }

    fn apply(&self, current: &Profile) -> Profile {
        Profile {
            display_name: self
                .display_name
                .clone()
                .unwrap_or_else(|| current.display_name.clone()),
            professional_registration: self
                .professional_registration
                .clone()
                .unwrap_or_else(|| current.professional_registration.clone()),
        }
    }
}

/// Audit snapshots of the profile fields that differ (`{}` when none do).
fn profile_diff(
    before: &Profile,
    after: &Profile,
) -> (
    serde_json::Map<String, Value>,
    serde_json::Map<String, Value>,
) {
    let mut b = serde_json::Map::new();
    let mut a = serde_json::Map::new();
    if before.display_name != after.display_name {
        b.insert("display_name".into(), json!(before.display_name));
        a.insert("display_name".into(), json!(after.display_name));
    }
    if before.professional_registration != after.professional_registration {
        b.insert(
            "professional_registration".into(),
            json!(before.professional_registration),
        );
        a.insert(
            "professional_registration".into(),
            json!(after.professional_registration),
        );
    }
    (b, a)
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
        "SELECT id, email, role, status, last_login_at, created_at, display_name,
                professional_registration
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
        .map(|r| {
            org_user(
                r.id,
                org,
                r.email,
                &r.role,
                &r.status,
                r.last_login_at,
                Profile {
                    display_name: r.display_name,
                    professional_registration: r.professional_registration,
                },
            )
        })
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
    let profile = ProfilePatch::parse(
        req.display_name.as_deref(),
        req.professional_registration.as_deref(),
        &mut errors,
    )
    .apply(&Profile::default());
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
        "INSERT INTO users (org_id, email, pw_hash, role, status, display_name,
                            professional_registration)
         VALUES ($1, $2, $3, $4, 'active', $5, $6)
         ON CONFLICT ((lower(email))) DO NOTHING
         RETURNING id, email, role, status, last_login_at, display_name,
                   professional_registration",
        org,
        email,
        pw_hash,
        role,
        profile.display_name,
        profile.professional_registration
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::Conflict {
        slug: "email-taken",
        detail: "a user with this e-mail address already exists".into(),
    })?;
    AuditEvent::user(&auth.principal, "user.create", "user", row.id)
        .org(org)
        .after(json!({
            "email": row.email,
            "role": row.role,
            "status": row.status,
            "display_name": row.display_name,
            "professional_registration": row.professional_registration,
        }))
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
            Profile {
                display_name: row.display_name,
                professional_registration: row.professional_registration,
            },
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
    let mut errors = Vec::new();
    // `invited` is set by an invitation flow, not by an edit.
    if req.status == Some(UserStatus::Invited) {
        errors.push(InvalidParam {
            name: "status".into(),
            reason: "must be active or disabled".into(),
        });
    }
    let profile_patch = ProfilePatch::parse(
        req.display_name.as_deref(),
        req.professional_registration.as_deref(),
        &mut errors,
    );
    if !errors.is_empty() {
        return Err(AppError::Validation(errors));
    }

    let mut tx = state.db.begin().await?;
    // Serialises owner changes in this org (last-owner check below).
    sqlx::query!("SELECT id FROM organizations WHERE id = $1 FOR UPDATE", org)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(AppError::NotFound)?;
    let target = sqlx::query!(
        "SELECT email, role, status, last_login_at, display_name, professional_registration
         FROM users WHERE id = $1 AND org_id = $2 FOR UPDATE",
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
    let current_profile = Profile {
        display_name: target.display_name.clone(),
        professional_registration: target.professional_registration.clone(),
    };
    let new_profile = profile_patch.apply(&current_profile);
    let (profile_before, profile_after) = profile_diff(&current_profile, &new_profile);
    let access_changed = role_str != target.role || new_status != target.status;
    if access_changed || !profile_after.is_empty() {
        sqlx::query!(
            "UPDATE users SET role = $2, status = $3, display_name = $4,
                              professional_registration = $5
             WHERE id = $1",
            user_id,
            role_str,
            new_status,
            new_profile.display_name,
            new_profile.professional_registration
        )
        .execute(&mut *tx)
        .await?;
        if new_status != "active" {
            refresh::revoke_user(&mut tx, user_id).await?;
        }
        let mut before = profile_before;
        let mut after = profile_after;
        before.insert("role".into(), json!(target.role));
        before.insert("status".into(), json!(target.status));
        after.insert("role".into(), json!(role_str));
        after.insert("status".into(), json!(new_status));
        AuditEvent::user(&auth.principal, "user.update", "user", user_id)
            .org(org)
            .before(Value::Object(before))
            .after(Value::Object(after))
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
        new_profile,
    )?))
}

fn user_profile(
    id: Uuid,
    totp_enrolled: bool,
    email: String,
    role: &str,
    org_id: Option<Uuid>,
    profile: Profile,
) -> AppResult<UserProfile> {
    Ok(UserProfile {
        id: id.to_string(),
        user_id: Some(id.to_string()),
        totp_enrolled: Some(totp_enrolled),
        email,
        role: rbac::role_from_str(role)
            .ok_or_else(|| AppError::Internal(format!("unknown role in database: {role}")))?,
        org_id: org_id.map(|o| o.to_string()),
        display_name: profile.display_name,
        professional_registration: profile.professional_registration,
    })
}

/// `GET /v1/users/me`: the caller's account and signer profile.
async fn get_me(
    State(state): State<AppState>,
    principal: UserPrincipal,
) -> AppResult<Json<UserProfile>> {
    let row = sqlx::query!(
        r#"SELECT email, role, org_id, display_name, professional_registration,
                (totp_secret IS NOT NULL AND btrim(totp_secret) <> '') AS "totp_enrolled!"
         FROM users WHERE id = $1"#,
        principal.user_id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::Unauthorized)?;
    Ok(Json(user_profile(
        principal.user_id,
        row.totp_enrolled,
        row.email,
        &row.role,
        row.org_id,
        Profile {
            display_name: row.display_name,
            professional_registration: row.professional_registration,
        },
    )?))
}

/// `PATCH /v1/users/me`: profile fields only (role and status are never self-service;
/// the body type rejects unknown fields).
async fn patch_me(
    State(state): State<AppState>,
    principal: UserPrincipal,
    meta: RequestMeta,
    ApiJson(req): ApiJson<UpdateProfileRequest>,
) -> AppResult<Json<UserProfile>> {
    let mut errors = Vec::new();
    let patch = ProfilePatch::parse(
        req.display_name.as_deref(),
        req.professional_registration.as_deref(),
        &mut errors,
    );
    if !errors.is_empty() {
        return Err(AppError::Validation(errors));
    }
    let mut tx = state.db.begin().await?;
    let row = sqlx::query!(
        r#"SELECT email, role, org_id, display_name, professional_registration,
                (totp_secret IS NOT NULL AND btrim(totp_secret) <> '') AS "totp_enrolled!"
         FROM users WHERE id = $1 FOR UPDATE"#,
        principal.user_id
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::Unauthorized)?;
    let current = Profile {
        display_name: row.display_name,
        professional_registration: row.professional_registration,
    };
    let new = patch.apply(&current);
    let (before, after) = profile_diff(&current, &new);
    if !after.is_empty() {
        sqlx::query!(
            "UPDATE users SET display_name = $2, professional_registration = $3 WHERE id = $1",
            principal.user_id,
            new.display_name,
            new.professional_registration
        )
        .execute(&mut *tx)
        .await?;
        AuditEvent::user(&principal, "user.profile_update", "user", principal.user_id)
            .before(Value::Object(before))
            .after(Value::Object(after))
            .meta(&meta)
            .record(&mut tx)
            .await?;
    }
    tx.commit().await?;
    Ok(Json(user_profile(
        principal.user_id,
        row.totp_enrolled,
        row.email,
        &row.role,
        row.org_id,
        new,
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
