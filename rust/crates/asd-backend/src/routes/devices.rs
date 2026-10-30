//! owner: WP-B.2
//!
//! Device enrolment and management (CON-10, CON-11, AUTH-3):
//!
//! - `POST /v1/devices/register` (device plane, no bearer, 10/min/IP)
//!   `RegisterDeviceRequest {enrol_code, platform, app_version, device_id?}`:
//!   - `device_id` of a live device enrolled by this (unrevoked) code: the device's token
//!     is rotated, no use is consumed -> `200 {status: "rotated", ...}`. Expiry does not
//!     block rotation (iOS re-registers with its stored code for the life of the install);
//!   - otherwise a new device -> `201 {status: "registered", ...}`, consuming one use;
//!   - unknown code `401`; revoked `410 enrol-code-revoked`; expired
//!     `410 enrol-code-expired`; all uses consumed `410 enrol-code-exhausted`.
//! - `GET /v1/orgs/{id}/devices` -> `DevicePage` (newest first).
//! - `POST /v1/orgs/{id}/devices/enrol-code` `EnrolCodeRequest` (body optional) ->
//!   `201 EnrolCodeResponse`; the plain code is returned only here.
//! - `POST /v1/devices/{id}/revoke` -> `200 Device`; idempotent.

use std::net::IpAddr;
use std::sync::Arc;

use asd_contract::api_v1::{
    Device, DevicePage, EnrolCodeRequest, EnrolCodeResponse, RegisterDeviceRequest,
    RegisterDeviceResponse, RegistrationStatus,
};
use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use chrono::{DateTime, Utc};
use http::StatusCode;
use serde_json::json;
use sqlx::PgConnection;
use uuid::Uuid;

use super::orgs::listing::{self, rfc3339, ListParams};
use crate::app::AppState;
use crate::audit::AuditEvent;
use crate::auth::extractors::{json_or_default, ApiJson, DevicePrincipal, RequestMeta, Require};
use crate::auth::rbac::perm;
use crate::auth::{self, device_token, RateLimiter};
use crate::error::{AppError, AppResult};
use crate::problem::InvalidParam;

pub const REGISTER_ATTEMPTS_PER_MINUTE: u32 = 10;
pub const DEFAULT_MAX_USES: u32 = 1;
pub const MAX_MAX_USES: u32 = 1_000;
pub const DEFAULT_EXPIRES_IN_DAYS: u32 = 7;
pub const MAX_EXPIRES_IN_DAYS: u32 = 90;
pub const MAX_LABEL_LEN: usize = 200;

/// Per-IP limiter for `POST /devices/register` (enrol codes are short secrets).
#[derive(Debug)]
pub struct RegisterLimiter(pub RateLimiter<Option<IpAddr>>);

pub fn router() -> Router<AppState> {
    let limiter = Arc::new(RegisterLimiter(RateLimiter::per_minute(
        REGISTER_ATTEMPTS_PER_MINUTE,
    )));
    Router::new()
        .route("/devices/register", post(register))
        .route("/devices/{id}/revoke", post(revoke))
        .route("/orgs/{id}/devices", get(list))
        .route("/orgs/{id}/devices/enrol-code", post(create_enrol_code))
        .layer(Extension(limiter))
}

fn invalid(name: &str, reason: &str) -> InvalidParam {
    InvalidParam {
        name: name.into(),
        reason: reason.into(),
    }
}

/// The latest bundle for the org, else the newest global default (`org_id IS NULL`).
async fn bundle_version(conn: &mut PgConnection, org: Uuid) -> AppResult<Option<String>> {
    Ok(sqlx::query_scalar!(
        "SELECT bundle_version FROM config_bundles
         WHERE org_id = $1 OR org_id IS NULL
         ORDER BY (org_id IS NULL), published_at DESC
         LIMIT 1",
        org
    )
    .fetch_optional(&mut *conn)
    .await?)
}

struct ValidRegistration {
    code_hash: [u8; 32],
    platform: String,
    app_version: String,
    device_id: Option<Uuid>,
}

fn validate_registration(req: &RegisterDeviceRequest) -> AppResult<ValidRegistration> {
    let mut errors = Vec::new();
    let code = auth::normalize_enrol_code(&req.enrol_code);
    if code.is_empty() || code.len() > 64 {
        errors.push(invalid("enrol_code", "must be 1 to 64 characters"));
    }
    let platform = req.platform.trim().to_owned();
    if platform.is_empty() || platform.chars().count() > 32 {
        errors.push(invalid("platform", "must be 1 to 32 characters"));
    }
    let app_version = req.app_version.trim().to_owned();
    if app_version.is_empty() || app_version.chars().count() > 64 {
        errors.push(invalid("app_version", "must be 1 to 64 characters"));
    }
    let device_id = match req.device_id.as_deref() {
        None => None,
        Some(raw) => match Uuid::parse_str(raw.trim()) {
            Ok(id) => Some(id),
            Err(_) => {
                errors.push(invalid("device_id", "must be a UUID"));
                None
            }
        },
    };
    if !errors.is_empty() {
        return Err(AppError::Validation(errors));
    }
    Ok(ValidRegistration {
        code_hash: auth::sha256(code.as_bytes()),
        platform,
        app_version,
        device_id,
    })
}

fn gone(slug: &'static str, detail: &str) -> AppError {
    AppError::Gone {
        slug,
        detail: detail.into(),
    }
}

async fn register(
    State(state): State<AppState>,
    Extension(limiter): Extension<Arc<RegisterLimiter>>,
    meta: RequestMeta,
    ApiJson(req): ApiJson<RegisterDeviceRequest>,
) -> AppResult<(StatusCode, Json<RegisterDeviceResponse>)> {
    limiter
        .0
        .check(&meta.ip)
        .map_err(|retry_after_secs| AppError::RateLimited { retry_after_secs })?;
    let reg = validate_registration(&req)?;

    let mut tx = state.db.begin().await?;
    // The row lock serialises registrations against one code (uses never exceed max_uses).
    let code = sqlx::query!(
        "SELECT id, org_id, max_uses, uses, expires_at, revoked_at, created_by, label
         FROM enrol_codes WHERE code_hash = $1 FOR UPDATE",
        reg.code_hash.as_slice()
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::Unauthorized)?;
    if code.revoked_at.is_some() {
        return Err(gone("enrol-code-revoked", "this enrol code was revoked"));
    }

    // CON-11 re-registration: rotate the token of a live device this code enrolled.
    if let Some(device_id) = reg.device_id {
        let existing = sqlx::query_scalar!(
            "SELECT id FROM devices
             WHERE id = $1 AND enrol_code_id = $2 AND org_id = $3 AND revoked_at IS NULL
             FOR UPDATE",
            device_id,
            code.id,
            code.org_id
        )
        .fetch_optional(&mut *tx)
        .await?;
        if existing.is_some() {
            let (token, hash) = device_token::new_token();
            sqlx::query!(
                "UPDATE devices SET token_hash = $2, platform = $3, app_version = $4 WHERE id = $1",
                device_id,
                hash.as_slice(),
                reg.platform,
                reg.app_version
            )
            .execute(&mut *tx)
            .await?;
            let device = DevicePrincipal {
                device_id,
                org_id: code.org_id,
            };
            AuditEvent::device(&device, "device.token_rotate", "device", device_id)
                .after(json!({"platform": reg.platform, "app_version": reg.app_version}))
                .meta(&meta)
                .record(&mut tx)
                .await?;
            let config_bundle_version = bundle_version(&mut tx, code.org_id).await?;
            tx.commit().await?;
            return Ok((
                StatusCode::OK,
                Json(RegisterDeviceResponse {
                    status: RegistrationStatus::Rotated,
                    device_id: device_id.to_string(),
                    device_token: token,
                    org_id: code.org_id.to_string(),
                    config_bundle_version,
                }),
            ));
        }
    }

    if code.expires_at <= Utc::now() {
        return Err(gone("enrol-code-expired", "this enrol code has expired"));
    }
    if code.uses >= code.max_uses {
        return Err(gone(
            "enrol-code-exhausted",
            "this enrol code has been used; re-register with device_id or ask for a new code",
        ));
    }

    let (token, hash) = device_token::new_token();
    let device_id = sqlx::query_scalar!(
        "INSERT INTO devices (org_id, platform, app_version, token_hash, label, enrolled_by, enrol_code_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         RETURNING id",
        code.org_id,
        reg.platform,
        reg.app_version,
        hash.as_slice(),
        code.label,
        code.created_by,
        code.id
    )
    .fetch_one(&mut *tx)
    .await?;
    sqlx::query!(
        "UPDATE enrol_codes SET uses = uses + 1 WHERE id = $1",
        code.id
    )
    .execute(&mut *tx)
    .await?;
    let device = DevicePrincipal {
        device_id,
        org_id: code.org_id,
    };
    AuditEvent::device(&device, "device.register", "device", device_id)
        .after(json!({
            "platform": reg.platform,
            "app_version": reg.app_version,
            "enrol_code_id": code.id,
        }))
        .meta(&meta)
        .record(&mut tx)
        .await?;
    let config_bundle_version = bundle_version(&mut tx, code.org_id).await?;
    tx.commit().await?;
    Ok((
        StatusCode::CREATED,
        Json(RegisterDeviceResponse {
            status: RegistrationStatus::Registered,
            device_id: device_id.to_string(),
            device_token: token,
            org_id: code.org_id.to_string(),
            config_bundle_version,
        }),
    ))
}

#[allow(clippy::too_many_arguments)]
fn device_view(
    id: Uuid,
    org: Uuid,
    platform: String,
    app_version: String,
    label: Option<String>,
    enrolled_at: DateTime<Utc>,
    revoked_at: Option<DateTime<Utc>>,
    last_seen_at: Option<DateTime<Utc>>,
    config_bundle_version: Option<String>,
) -> Device {
    Device {
        id: id.to_string(),
        org_id: org.to_string(),
        platform,
        app_version,
        label,
        enrolled_at: rfc3339(enrolled_at),
        revoked_at: revoked_at.map(rfc3339),
        last_seen_at: last_seen_at.map(rfc3339),
        config_bundle_version,
    }
}

async fn load_device(conn: &mut PgConnection, id: Uuid) -> AppResult<Device> {
    let r = sqlx::query!(
        r#"SELECT d.id, d.org_id, d.platform, d.app_version, d.label, d.enrolled_at, d.revoked_at,
                  GREATEST(
                    (SELECT max(s.received_at) FROM sessions s WHERE s.device_id = d.id),
                    (SELECT max(a.at) FROM bundle_adoption a WHERE a.device_id = d.id)
                  ) AS "last_seen_at?",
                  (SELECT a.bundle_version FROM bundle_adoption a WHERE a.device_id = d.id
                   ORDER BY a.at DESC LIMIT 1) AS "config_bundle_version?"
           FROM devices d WHERE d.id = $1"#,
        id
    )
    .fetch_one(&mut *conn)
    .await?;
    Ok(device_view(
        r.id,
        r.org_id,
        r.platform,
        r.app_version,
        r.label,
        r.enrolled_at,
        r.revoked_at,
        r.last_seen_at,
        r.config_bundle_version,
    ))
}

async fn list(
    State(state): State<AppState>,
    auth: Require<perm::DeviceRead>,
    page: ListParams,
) -> AppResult<Json<DevicePage>> {
    let org = auth.org();
    let after = page.cursor_ts()?;
    let rows = sqlx::query!(
        r#"SELECT d.id, d.platform, d.app_version, d.label, d.enrolled_at, d.revoked_at,
                  GREATEST(
                    (SELECT max(s.received_at) FROM sessions s WHERE s.device_id = d.id),
                    (SELECT max(a.at) FROM bundle_adoption a WHERE a.device_id = d.id)
                  ) AS "last_seen_at?",
                  (SELECT a.bundle_version FROM bundle_adoption a WHERE a.device_id = d.id
                   ORDER BY a.at DESC LIMIT 1) AS "config_bundle_version?"
           FROM devices d
           WHERE d.org_id = $1
             AND ($2::timestamptz IS NULL OR (d.enrolled_at, d.id) < ($2, $3))
           ORDER BY d.enrolled_at DESC, d.id DESC
           LIMIT $4"#,
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
                .map(|r| listing::encode_ts_cursor(r.enrolled_at, r.id))
        })
        .flatten();
    let items = rows
        .into_iter()
        .map(|r| {
            device_view(
                r.id,
                org,
                r.platform,
                r.app_version,
                r.label,
                r.enrolled_at,
                r.revoked_at,
                r.last_seen_at,
                r.config_bundle_version,
            )
        })
        .collect();
    Ok(Json(DevicePage {
        items,
        next_cursor,
        total: None,
    }))
}

fn validate_enrol_request(req: &EnrolCodeRequest) -> AppResult<(u32, u32, Option<String>)> {
    let mut errors = Vec::new();
    let max_uses = req.max_uses.unwrap_or(DEFAULT_MAX_USES);
    if !(1..=MAX_MAX_USES).contains(&max_uses) {
        errors.push(invalid("max_uses", "must be 1 to 1000"));
    }
    let days = req.expires_in_days.unwrap_or(DEFAULT_EXPIRES_IN_DAYS);
    if !(1..=MAX_EXPIRES_IN_DAYS).contains(&days) {
        errors.push(invalid("expires_in_days", "must be 1 to 90"));
    }
    let label = req
        .label
        .as_deref()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_owned);
    if label
        .as_ref()
        .is_some_and(|l| l.chars().count() > MAX_LABEL_LEN)
    {
        errors.push(invalid("label", "must be at most 200 characters"));
    }
    if errors.is_empty() {
        Ok((max_uses, days, label))
    } else {
        Err(AppError::Validation(errors))
    }
}

async fn create_enrol_code(
    State(state): State<AppState>,
    auth: Require<perm::EnrolCodeCreate>,
    meta: RequestMeta,
    body: Bytes,
) -> AppResult<(StatusCode, Json<EnrolCodeResponse>)> {
    let org = auth.org();
    let req: EnrolCodeRequest = json_or_default(&body)?;
    let (max_uses, days, label) = validate_enrol_request(&req)?;
    let expires_at = Utc::now() + chrono::Duration::days(i64::from(days));
    let max_uses_db = i32::try_from(max_uses).map_err(|_| AppError::Internal("max_uses".into()))?;

    let mut tx = state.db.begin().await?;
    // 60-bit codes: a collision is astronomically unlikely, but never reuse a hash.
    let mut created = None;
    for _ in 0..3 {
        let code = auth::new_enrol_code();
        let hash = auth::enrol_code_hash(&code);
        let id = sqlx::query_scalar!(
            "INSERT INTO enrol_codes (org_id, code_hash, max_uses, expires_at, created_by, label)
             VALUES ($1, $2, $3, $4, $5, $6)
             ON CONFLICT (code_hash) DO NOTHING
             RETURNING id",
            org,
            hash.as_slice(),
            max_uses_db,
            expires_at,
            auth.principal.user_id,
            label
        )
        .fetch_optional(&mut *tx)
        .await?;
        if let Some(id) = id {
            created = Some((id, code));
            break;
        }
    }
    let (id, code) =
        created.ok_or_else(|| AppError::Internal("could not allocate an enrol code".into()))?;
    AuditEvent::user(&auth.principal, "enrol_code.create", "enrol_code", id)
        .org(org)
        .after(json!({"max_uses": max_uses, "expires_at": rfc3339(expires_at), "label": label}))
        .meta(&meta)
        .record(&mut tx)
        .await?;
    tx.commit().await?;
    Ok((
        StatusCode::CREATED,
        Json(EnrolCodeResponse {
            id: id.to_string(),
            code,
            max_uses,
            expires_at: rfc3339(expires_at),
        }),
    ))
}

async fn revoke(
    State(state): State<AppState>,
    auth: Require<perm::DeviceRevoke>,
    Path(id): Path<String>,
    meta: RequestMeta,
) -> AppResult<Json<Device>> {
    let id = Uuid::parse_str(&id).map_err(|_| AppError::NotFound)?;
    let mut tx = state.db.begin().await?;
    let row = sqlx::query!(
        "SELECT org_id, revoked_at FROM devices WHERE id = $1 FOR UPDATE",
        id
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::NotFound)?;
    auth.ensure_org(row.org_id)?;
    if row.revoked_at.is_none() {
        let revoked_at = sqlx::query_scalar!(
            "UPDATE devices SET revoked_at = now() WHERE id = $1 RETURNING revoked_at AS \"revoked_at!\"",
            id
        )
        .fetch_one(&mut *tx)
        .await?;
        AuditEvent::user(&auth.principal, "device.revoke", "device", id)
            .org(row.org_id)
            .before(json!({"revoked_at": null}))
            .after(json!({"revoked_at": rfc3339(revoked_at)}))
            .meta(&meta)
            .record(&mut tx)
            .await?;
    }
    let device = load_device(&mut tx, id).await?;
    tx.commit().await?;
    Ok(Json(device))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(code: &str, device_id: Option<&str>) -> RegisterDeviceRequest {
        RegisterDeviceRequest {
            enrol_code: code.into(),
            platform: " iOS ".into(),
            app_version: "1.0".into(),
            device_id: device_id.map(str::to_owned),
        }
    }

    #[test]
    fn registration_validation() {
        let ok = validate_registration(&req("abcd-efgh", None)).unwrap();
        assert_eq!(ok.platform, "iOS");
        assert_eq!(ok.code_hash, auth::enrol_code_hash("ABCDEFGH"));
        assert!(validate_registration(&req(" - ", None)).is_err());
        assert!(validate_registration(&req("A", Some("nope"))).is_err());
        let id = Uuid::new_v4();
        assert_eq!(
            validate_registration(&req("A", Some(&id.to_string())))
                .unwrap()
                .device_id,
            Some(id)
        );
    }

    #[test]
    fn enrol_request_defaults_and_bounds() {
        assert_eq!(
            validate_enrol_request(&EnrolCodeRequest::default()).unwrap(),
            (1, 7, None)
        );
        let r = EnrolCodeRequest {
            max_uses: Some(1000),
            expires_in_days: Some(90),
            label: Some("  ".into()),
        };
        assert_eq!(validate_enrol_request(&r).unwrap(), (1000, 90, None));
        for bad in [
            EnrolCodeRequest {
                max_uses: Some(0),
                ..Default::default()
            },
            EnrolCodeRequest {
                max_uses: Some(1001),
                ..Default::default()
            },
            EnrolCodeRequest {
                expires_in_days: Some(91),
                ..Default::default()
            },
        ] {
            assert!(validate_enrol_request(&bad).is_err());
        }
    }
}
