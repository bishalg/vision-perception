//! owner: WP-I.7
//!
//! Integration settings and the delivery log (INTG-1..5, DLV-5):
//!
//! - `GET|POST /v1/orgs/{id}/integrations` (`IntegrationRead` / `IntegrationManage`);
//! - `GET /v1/orgs/{id}/export-destinations` (`ExportRead`): id, name, kind, environment and
//!   status of the active destinations, no config or secret;
//! - `GET|PATCH|DELETE /v1/integrations/{id}` (delete archives; `409` while deliveries are
//!   pending or sending);
//! - `PUT /v1/integrations/{id}/secret` (webhook): write-only, encrypted (INTG-2);
//! - `POST /v1/integrations/{id}/ssh-key` (sftp): generate or rotate the Ed25519 key and
//!   return its public half (INTG-5);
//! - `POST /v1/integrations/{id}/test`: a synthetic delivery (`TEST-0000`) whose outcome is
//!   stored as `last_test_*` with an error class only (INTG-4);
//! - `GET /v1/integrations/{id}/deliveries` and `GET /v1/exports/{id}/deliveries` (DLV-5).
//!
//! Audit rows carry the config with the webhook URL's query string removed, and for
//! secrets only fingerprints (INTG-2, trap 2, trap 20). Settings never touch
//! `organizations.settings` (G3).

use asd_contract::export::{
    CreateIntegrationRequest, DeliverableDestination, DeliverableDestinationList, Delivery,
    DeliveryEvent, DeliveryPage, DeliveryStatus, Integration, IntegrationEnvironment,
    IntegrationKind, IntegrationPage, IntegrationStatus, IntegrationTestResult,
    PutIntegrationSecretRequest, SshPublicKey, UpdateIntegrationRequest,
};
use asd_contract::problem::CONFLICT;
use axum::extract::{Path, Query, State};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use http::StatusCode;
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::app::AppState;
use crate::audit::AuditEvent;
use crate::auth::extractors::{ApiJson, RequestMeta, Require};
use crate::auth::rbac::perm;
use crate::crypto::ClinicalCrypto;
use crate::error::{AppError, AppResult};
use crate::export_build;
use crate::interop::destination::{self, DestinationRow};
use crate::interop::webhook::{self, TestRecord, WebhookEvent};
use crate::interop::{sftp, ssh_keys, DeliveryFailure, ErrorClass};
use crate::problem::InvalidParam;
use crate::read::cursor::{self, Cursor};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/orgs/{id}/integrations", get(list).post(create))
        .route("/orgs/{id}/export-destinations", get(deliverable))
        .route("/integrations/{id}", get(read).patch(update).delete(delete))
        .route("/integrations/{id}/secret", put(put_secret))
        .route("/integrations/{id}/ssh-key", post(ssh_key))
        .route("/integrations/{id}/test", post(test))
        .route("/integrations/{id}/deliveries", get(destination_deliveries))
        .route("/exports/{id}/deliveries", get(export_deliveries))
}

fn bad(name: &str, reason: impl Into<String>) -> InvalidParam {
    InvalidParam {
        name: name.into(),
        reason: reason.into(),
    }
}

fn id_of(raw: &str) -> AppResult<Uuid> {
    Uuid::parse_str(raw).map_err(|_| AppError::NotFound)
}

async fn load_in_org(
    state: &AppState,
    raw: &str,
    check: impl FnOnce(Uuid) -> AppResult<()>,
) -> AppResult<DestinationRow> {
    let row = destination::load(&state.db, id_of(raw)?)
        .await?
        .ok_or(AppError::NotFound)?;
    check(row.org_id)?;
    Ok(row)
}

async fn reload(state: &AppState, id: Uuid) -> AppResult<Integration> {
    Ok(destination::load(&state.db, id)
        .await?
        .ok_or_else(|| AppError::Internal("destination vanished".into()))?
        .to_dto())
}

#[derive(Debug, Default, Deserialize)]
struct PageQuery {
    cursor: Option<String>,
    limit: Option<String>,
}

fn page_args(q: &PageQuery) -> AppResult<(i64, Option<DateTime<Utc>>, Option<Uuid>)> {
    let limit = match q.limit.as_deref().filter(|s| !s.is_empty()) {
        None => cursor::limit(None)?,
        Some(raw) => {
            cursor::limit(Some(raw.parse::<i64>().map_err(|_| {
                AppError::Validation(vec![bad("limit", "must be an integer")])
            })?))?
        }
    };
    let after = q
        .cursor
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(Cursor::decode)
        .transpose()?;
    Ok(match after {
        Some(c) => (
            limit,
            Some(c.at),
            Some(Uuid::parse_str(&c.id).map_err(|_| {
                AppError::Validation(vec![bad("cursor", "is not a cursor of this list")])
            })?),
        ),
        None => (limit, None, None),
    })
}

// ---- destinations ------------------------------------------------------------------------

async fn list(
    auth: Require<perm::IntegrationRead>,
    State(state): State<AppState>,
    Query(q): Query<PageQuery>,
) -> AppResult<Json<IntegrationPage>> {
    let org = auth.org();
    let (limit, after_at, after_id) = page_args(&q)?;
    let rows = sqlx::query!(
        r#"SELECT id, created_at FROM integration_destinations
           WHERE org_id = $1 AND archived_at IS NULL
             AND ($2::timestamptz IS NULL OR (created_at, id) < ($2, $3::uuid))
           ORDER BY created_at DESC, id DESC LIMIT $4"#,
        org,
        after_at,
        after_id,
        limit + 1,
    )
    .fetch_all(&state.db)
    .await?;
    let total: i64 = sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!" FROM integration_destinations
           WHERE org_id = $1 AND archived_at IS NULL"#,
        org
    )
    .fetch_one(&state.db)
    .await?;
    let take = usize::try_from(limit).unwrap_or(usize::MAX);
    let has_more = rows.len() > take;
    let mut items = Vec::with_capacity(take.min(rows.len()));
    let mut last = None;
    for r in rows.into_iter().take(take) {
        last = Some(Cursor::new(r.created_at, r.id.to_string()));
        if let Some(row) = destination::load(&state.db, r.id).await? {
            items.push(row.to_dto());
        }
    }
    Ok(Json(IntegrationPage {
        items,
        next_cursor: if has_more {
            last.map(|c| c.encode())
        } else {
            None
        },
        total: Some(u64::try_from(total).unwrap_or(0)),
    }))
}

/// The destinations an exporter may pick (`ExportRead`): active, not archived, and no
/// config or secret (an exporter that cannot read integrations must not learn hosts or paths).
async fn deliverable(
    auth: Require<perm::ExportRead>,
    State(state): State<AppState>,
) -> AppResult<Json<DeliverableDestinationList>> {
    let rows = sqlx::query!(
        r#"SELECT id, name, kind, environment, status FROM integration_destinations
           WHERE org_id = $1 AND status = 'active' AND archived_at IS NULL
             AND kind IN ('webhook', 'sftp')
           ORDER BY created_at, id"#,
        auth.org()
    )
    .fetch_all(&state.db)
    .await?;
    let stored = |what: &str| AppError::Internal(format!("unknown {what} in database"));
    let items = rows
        .into_iter()
        .map(|r| {
            Ok(DeliverableDestination {
                id: r.id.to_string(),
                name: r.name,
                kind: IntegrationKind::parse(&r.kind).ok_or_else(|| stored("kind"))?,
                environment: IntegrationEnvironment::parse(&r.environment)
                    .ok_or_else(|| stored("environment"))?,
                status: IntegrationStatus::parse(&r.status).ok_or_else(|| stored("status"))?,
            })
        })
        .collect::<AppResult<Vec<_>>>()?;
    Ok(Json(DeliverableDestinationList { items }))
}

async fn create(
    auth: Require<perm::IntegrationManage>,
    State(state): State<AppState>,
    meta: RequestMeta,
    ApiJson(req): ApiJson<CreateIntegrationRequest>,
) -> AppResult<(StatusCode, Json<Integration>)> {
    let org = auth.org();
    let mut errors = Vec::new();
    let name = destination::validate_name(&req.name, &mut errors);
    if let Err(why) = destination::kind_available(req.kind) {
        errors.push(bad("kind", why));
    } else {
        errors.extend(destination::validate_config(req.kind, &req.config));
    }
    if !errors.is_empty() {
        return Err(AppError::Validation(errors));
    }
    let environment = req.environment.unwrap_or(IntegrationEnvironment::Test);
    let id = Uuid::new_v4();
    let mut tx = state.db.begin().await?;
    sqlx::query!(
        "INSERT INTO integration_destinations
           (id, org_id, kind, name, config, environment, created_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
        id,
        org,
        req.kind.as_str(),
        name,
        destination::config_json(&req.config),
        environment.as_str(),
        auth.principal.user_id,
    )
    .execute(&mut *tx)
    .await?;
    AuditEvent::user(&auth.principal, "integration.create", "integration", id)
        .org(org)
        .after(json!({
            "kind": req.kind.as_str(),
            "name": name,
            "environment": environment.as_str(),
            "status": IntegrationStatus::Active.as_str(),
            "config": destination::audit_config(&req.config),
        }))
        .meta(&meta)
        .record(&mut tx)
        .await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(reload(&state, id).await?)))
}

async fn read(
    auth: Require<perm::IntegrationRead>,
    State(state): State<AppState>,
    Path(raw): Path<String>,
) -> AppResult<Json<Integration>> {
    let row = load_in_org(&state, &raw, |org| auth.ensure_org(org)).await?;
    Ok(Json(row.to_dto()))
}

async fn update(
    auth: Require<perm::IntegrationManage>,
    State(state): State<AppState>,
    meta: RequestMeta,
    Path(raw): Path<String>,
    ApiJson(req): ApiJson<UpdateIntegrationRequest>,
) -> AppResult<Json<Integration>> {
    let row = load_in_org(&state, &raw, |org| auth.ensure_org(org)).await?;
    let mut errors = Vec::new();
    let name = match &req.name {
        Some(n) => destination::validate_name(n, &mut errors),
        None => row.name.clone(),
    };
    let before_config = row.config();
    let config = req.config.clone().unwrap_or_else(|| before_config.clone());
    if req.config.is_some() {
        errors.extend(destination::validate_config(row.kind, &config));
    }
    if !errors.is_empty() {
        return Err(AppError::Validation(errors));
    }
    let environment = req.environment.unwrap_or(row.environment);
    let status = req.status.unwrap_or(row.status);
    let mut tx = state.db.begin().await?;
    sqlx::query!(
        "UPDATE integration_destinations
         SET name = $2, config = $3, environment = $4, status = $5, updated_at = now()
         WHERE id = $1",
        row.id,
        name,
        destination::config_json(&config),
        environment.as_str(),
        status.as_str(),
    )
    .execute(&mut *tx)
    .await?;
    AuditEvent::user(&auth.principal, "integration.update", "integration", row.id)
        .org(row.org_id)
        .before(json!({
            "name": row.name,
            "environment": row.environment.as_str(),
            "status": row.status.as_str(),
            "config": destination::audit_config(&before_config),
        }))
        .after(json!({
            "name": name,
            "environment": environment.as_str(),
            "status": status.as_str(),
            "config": destination::audit_config(&config),
        }))
        .meta(&meta)
        .record(&mut tx)
        .await?;
    tx.commit().await?;
    Ok(Json(reload(&state, row.id).await?))
}

/// `DELETE /v1/integrations/{id}` (`IntegrationManage`): archives the destination (hidden,
/// disabled; the delivery log keeps its row). `409` while a delivery to it is pending or
/// sending.
async fn delete(
    auth: Require<perm::IntegrationManage>,
    State(state): State<AppState>,
    meta: RequestMeta,
    Path(raw): Path<String>,
) -> AppResult<StatusCode> {
    let row = load_in_org(&state, &raw, |org| auth.ensure_org(org)).await?;
    let mut tx = state.db.begin().await?;
    // Serialises with a concurrent delete of the same destination.
    let locked = sqlx::query_scalar!(
        "SELECT id FROM integration_destinations
         WHERE id = $1 AND archived_at IS NULL FOR UPDATE",
        row.id
    )
    .fetch_optional(&mut *tx)
    .await?;
    if locked.is_none() {
        return Err(AppError::NotFound);
    }
    let in_flight: i64 = sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!" FROM export_deliveries
           WHERE destination_id = $1 AND status IN ('pending', 'sending')"#,
        row.id
    )
    .fetch_one(&mut *tx)
    .await?;
    if in_flight > 0 {
        return Err(AppError::Conflict {
            slug: CONFLICT,
            detail: format!(
                "{in_flight} deliveries to this destination are still pending; disable it, or \
                 delete it once they are delivered or failed"
            ),
        });
    }
    sqlx::query!(
        "UPDATE integration_destinations
         SET archived_at = now(), status = 'disabled', updated_at = now()
         WHERE id = $1",
        row.id
    )
    .execute(&mut *tx)
    .await?;
    AuditEvent::user(&auth.principal, "integration.delete", "integration", row.id)
        .org(row.org_id)
        .before(json!({
            "kind": row.kind.as_str(),
            "name": row.name,
            "environment": row.environment.as_str(),
            "status": row.status.as_str(),
            "config": destination::audit_config(&row.config()),
        }))
        .meta(&meta)
        .record(&mut tx)
        .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

// ---- secrets -----------------------------------------------------------------------------

async fn put_secret(
    auth: Require<perm::IntegrationManage>,
    crypto: ClinicalCrypto,
    State(state): State<AppState>,
    meta: RequestMeta,
    Path(raw): Path<String>,
    ApiJson(req): ApiJson<PutIntegrationSecretRequest>,
) -> AppResult<Json<Integration>> {
    let row = load_in_org(&state, &raw, |org| auth.ensure_org(org)).await?;
    if row.kind != IntegrationKind::Webhook {
        return Err(AppError::Conflict {
            slug: CONFLICT,
            detail: "only webhook destinations take a secret; sftp destinations use the key from POST /v1/integrations/{id}/ssh-key".into(),
        });
    }
    destination::webhook_key(&req.secret)
        .map_err(|why| AppError::Validation(vec![bad("secret", why)]))?;
    let sealed = crypto
        .encrypt(
            row.org_id,
            &destination::secret_aad(row.org_id, row.id),
            req.secret.as_bytes(),
        )
        .await?;
    let fp = destination::fingerprint(req.secret.as_bytes());
    let mut tx = state.db.begin().await?;
    sqlx::query!(
        "UPDATE integration_destinations
         SET secret_ct = $2, secret_key_id = $3, secret_fingerprint = $4, updated_at = now()
         WHERE id = $1",
        row.id,
        sealed.ciphertext,
        sealed.key_id,
        fp,
    )
    .execute(&mut *tx)
    .await?;
    AuditEvent::user(
        &auth.principal,
        "integration.secret_set",
        "integration",
        row.id,
    )
    .org(row.org_id)
    .before(json!({"secret_fingerprint": row.secret_fingerprint}))
    .after(json!({"secret_fingerprint": fp}))
    .meta(&meta)
    .record(&mut tx)
    .await?;
    tx.commit().await?;
    Ok(Json(reload(&state, row.id).await?))
}

async fn ssh_key(
    auth: Require<perm::IntegrationManage>,
    crypto: ClinicalCrypto,
    State(state): State<AppState>,
    meta: RequestMeta,
    Path(raw): Path<String>,
) -> AppResult<Json<SshPublicKey>> {
    let row = load_in_org(&state, &raw, |org| auth.ensure_org(org)).await?;
    if row.kind != IntegrationKind::Sftp {
        return Err(AppError::Conflict {
            slug: CONFLICT,
            detail: "only sftp destinations authenticate with an SSH key".into(),
        });
    }
    let key = ssh_keys::generate().map_err(AppError::Internal)?;
    let sealed = crypto
        .encrypt(
            row.org_id,
            &destination::secret_aad(row.org_id, row.id),
            key.seed.as_ref(),
        )
        .await?;
    let fp = destination::fingerprint(key.public_openssh.as_bytes());
    let mut tx = state.db.begin().await?;
    sqlx::query!(
        "UPDATE integration_destinations
         SET secret_ct = $2, secret_key_id = $3, secret_fingerprint = $4, ssh_public_key = $5,
             updated_at = now()
         WHERE id = $1",
        row.id,
        sealed.ciphertext,
        sealed.key_id,
        fp,
        key.public_openssh,
    )
    .execute(&mut *tx)
    .await?;
    AuditEvent::user(
        &auth.principal,
        "integration.ssh_key_generate",
        "integration",
        row.id,
    )
    .org(row.org_id)
    .before(json!({"secret_fingerprint": row.secret_fingerprint}))
    .after(json!({"secret_fingerprint": fp, "ssh_fingerprint": key.fingerprint}))
    .meta(&meta)
    .record(&mut tx)
    .await?;
    tx.commit().await?;
    Ok(Json(SshPublicKey {
        public_key: key.public_openssh,
        fingerprint: key.fingerprint,
    }))
}

// ---- connection test (INTG-4) ------------------------------------------------------------

/// The synthetic file an SFTP test drops: no participant data, the fixed test token.
fn test_file(destination: Uuid, at: &str) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "test": true,
        "participant_token": webhook::TEST_TOKEN,
        "destination_id": destination,
        "sent_at": at,
    }))
    .expect("plain data")
}

async fn run_test(
    crypto: &ClinicalCrypto,
    row: &DestinationRow,
    at: &str,
) -> Result<String, DeliveryFailure> {
    if let Err(_why) = destination::kind_available(row.kind) {
        return Err(ErrorClass::UnsupportedKind.into());
    }
    let config = row.config();
    let secret = row.open_secret(Some(&crypto.0)).await?;
    match row.kind {
        IntegrationKind::Webhook => {
            let url = config.url.as_deref().ok_or(ErrorClass::InvalidConfig)?;
            let secret = std::str::from_utf8(&secret).map_err(|_| ErrorClass::MissingSecret)?;
            let key = destination::webhook_key(secret).map_err(|_| ErrorClass::MissingSecret)?;
            let id = format!("test-{}", Uuid::new_v4());
            let event = WebhookEvent {
                event_type: DeliveryEvent::ExportReady.as_str(),
                id: id.clone(),
                org_id: row.org_id.to_string(),
                export_id: Uuid::nil().to_string(),
                format: config.format.map_or("csv", |f| f.as_str()).to_owned(),
                mode: "identified".into(),
                created_at: at.to_owned(),
                download: None,
                test: Some(true),
                test_record: Some(TestRecord {
                    participant_token: webhook::TEST_TOKEN,
                }),
            };
            webhook::post(url, &key, &id, event.to_body()).await
        }
        IntegrationKind::Sftp => {
            let target = sftp::SftpTarget::from_config(&config)?;
            let key = ssh_keys::private_key(&secret).ok_or(ErrorClass::MissingSecret)?;
            let body = test_file(row.id, at);
            let sha = export_build::hex(&<sha2::Sha256 as sha2::Digest>::digest(&body));
            let name = format!("curv-test-{}.json", webhook::TEST_TOKEN);
            sftp::drop_file(&target, key, &name, &body, &sha).await
        }
        IntegrationKind::FhirServer | IntegrationKind::Hl7Mllp => {
            Err(ErrorClass::UnsupportedKind.into())
        }
    }
}

async fn test(
    auth: Require<perm::IntegrationManage>,
    crypto: ClinicalCrypto,
    State(state): State<AppState>,
    meta: RequestMeta,
    Path(raw): Path<String>,
) -> AppResult<Json<IntegrationTestResult>> {
    let row = load_in_org(&state, &raw, |org| auth.ensure_org(org)).await?;
    let now = Utc::now();
    let at = cursor::ts(now);
    let outcome = run_test(&crypto, &row, &at).await;
    let (ok, error_class, remote_ref) = match &outcome {
        Ok(r) => (true, None, Some(r.clone())),
        Err(f) => (false, Some(f.class.as_str()), f.remote_ref.clone()),
    };
    let remote_ref = remote_ref.map(|r| r.chars().take(200).collect::<String>());
    let mut tx = state.db.begin().await?;
    sqlx::query!(
        "UPDATE integration_destinations
         SET last_test_at = $2, last_test_ok = $3, last_test_error = $4
         WHERE id = $1",
        row.id,
        now,
        ok,
        error_class,
    )
    .execute(&mut *tx)
    .await?;
    AuditEvent::user(&auth.principal, "integration.test", "integration", row.id)
        .org(row.org_id)
        .after(json!({"ok": ok, "error_class": error_class, "remote_ref": remote_ref}))
        .meta(&meta)
        .record(&mut tx)
        .await?;
    tx.commit().await?;
    Ok(Json(IntegrationTestResult {
        ok,
        error_class: error_class.map(str::to_owned),
        remote_ref,
        tested_at: at,
    }))
}

// ---- delivery log (DLV-5) ----------------------------------------------------------------

async fn deliveries_page(
    state: &AppState,
    destination: Option<Uuid>,
    export: Option<Uuid>,
    q: &PageQuery,
) -> AppResult<DeliveryPage> {
    let (limit, after_at, after_id) = page_args(q)?;
    let rows = sqlx::query!(
        r#"SELECT d.id, d.export_id, d.destination_id, n.name, d.event, d.status, d.attempts,
                  d.last_error_class, d.remote_ref, d.created_at, d.updated_at, d.delivered_at
           FROM export_deliveries d JOIN integration_destinations n ON n.id = d.destination_id
           WHERE ($1::uuid IS NULL OR d.destination_id = $1)
             AND ($2::uuid IS NULL OR d.export_id = $2)
             AND ($3::timestamptz IS NULL OR (d.created_at, d.id) < ($3, $4::uuid))
           ORDER BY d.created_at DESC, d.id DESC LIMIT $5"#,
        destination,
        export,
        after_at,
        after_id,
        limit + 1,
    )
    .fetch_all(&state.db)
    .await?;
    let total: i64 = sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!" FROM export_deliveries
           WHERE ($1::uuid IS NULL OR destination_id = $1)
             AND ($2::uuid IS NULL OR export_id = $2)"#,
        destination,
        export,
    )
    .fetch_one(&state.db)
    .await?;
    let take = usize::try_from(limit).unwrap_or(usize::MAX);
    let has_more = rows.len() > take;
    let mut items = Vec::new();
    let mut last = None;
    for r in rows.into_iter().take(take) {
        last = Some(Cursor::new(r.created_at, r.id.to_string()));
        items.push(Delivery {
            id: r.id.to_string(),
            export_id: r.export_id.to_string(),
            destination_id: r.destination_id.to_string(),
            destination_name: r.name,
            event: DeliveryEvent::parse(&r.event).unwrap_or(DeliveryEvent::ExportReady),
            status: DeliveryStatus::parse(&r.status).unwrap_or(DeliveryStatus::Pending),
            attempts: u32::try_from(r.attempts).unwrap_or(0),
            last_error_class: r.last_error_class,
            remote_ref: r.remote_ref,
            created_at: cursor::ts(r.created_at),
            updated_at: cursor::ts(r.updated_at),
            delivered_at: r.delivered_at.map(cursor::ts),
        });
    }
    Ok(DeliveryPage {
        items,
        next_cursor: if has_more {
            last.map(|c| c.encode())
        } else {
            None
        },
        total: Some(u64::try_from(total).unwrap_or(0)),
    })
}

async fn destination_deliveries(
    auth: Require<perm::IntegrationRead>,
    State(state): State<AppState>,
    Path(raw): Path<String>,
    Query(q): Query<PageQuery>,
) -> AppResult<Json<DeliveryPage>> {
    let row = load_in_org(&state, &raw, |org| auth.ensure_org(org)).await?;
    Ok(Json(deliveries_page(&state, Some(row.id), None, &q).await?))
}

async fn export_deliveries(
    auth: Require<perm::ExportRead>,
    State(state): State<AppState>,
    Path(raw): Path<String>,
    Query(q): Query<PageQuery>,
) -> AppResult<Json<DeliveryPage>> {
    let export = export_build::load_export(&state.db, id_of(&raw)?)
        .await?
        .ok_or(AppError::NotFound)?;
    auth.ensure_org(export.org_id)?;
    Ok(Json(
        deliveries_page(&state, None, Some(export.id), &q).await?,
    ))
}
