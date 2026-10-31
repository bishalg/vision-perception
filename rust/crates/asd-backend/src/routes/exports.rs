//! owner: WP-I.6
//!
//! Export routes (EXJ-1..6):
//!
//! - `POST /v1/orgs/{id}/exports` (`ExportDeidentified`; identified mode also needs
//!   `ExportIdentified`): validates, queues one `export` job, `202` with the [`Export`];
//! - `GET /v1/orgs/{id}/exports` and `GET /v1/exports/{id}` (`ExportRead`);
//! - `POST /v1/exports/{id}/download-url` (`ExportRead`, same org, never superadmin):
//!   audited, valid five minutes, `410 export-expired` after expiry;
//! - `GET /v1/exports/{id}/file?t=` (proxy blob mode only): the token is the credential;
//! - `GET /v1/participants/{id}/disclosures` (`ClinicalContentRead`).
//!
//! Request-time order: path org (404) and base role (403), identified role (403), body
//! validation (400 `validation-failed`), consent purpose (400 `consent-purpose-mismatch`),
//! MRN mode for HL7 (409 `patient-ref-disabled`).

use std::collections::BTreeMap;
use std::time::Duration;

use asd_contract::clinical::{settings as org_settings, PatientRefMode};
use asd_contract::export::{
    Disclosure, DisclosureDelivery, DisclosurePage, DownloadUrl, Export, ExportFormat, ExportMode,
    ExportPage, ExportRequest, ExportStatus, DOWNLOAD_URL_TTL_SECS, MAX_REASON_CHARS,
    MIN_REASON_CHARS,
};
use asd_contract::problem::{
    CONFLICT, CONSENT_PURPOSE_MISMATCH, EXPORT_EXPIRED, PATIENT_REF_DISABLED,
};
use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::Utc;
use http::{header, HeaderValue, StatusCode};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::app::AppState;
use crate::audit::AuditEvent;
use crate::auth::extractors::{ApiJson, RequestMeta, Require};
use crate::auth::rbac::{self, perm, Action};
use crate::config::BlobMode;
use crate::error::{AppError, AppResult};
use crate::export_build::{self, filters, ExportRow};
use crate::jobs::{self, export as export_job, KIND_EXPORT};
use crate::problem::InvalidParam;
use crate::read::cursor::{self, Cursor};
use crate::routes::participant_detail::participant_org;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/orgs/{id}/exports", post(create).get(list))
        .route("/exports/{id}", get(read))
        .route("/exports/{id}/download-url", post(download_url))
        .route("/exports/{id}/file", get(file))
        .route("/participants/{id}/disclosures", get(disclosures))
}

fn bad(name: &str, reason: impl Into<String>) -> InvalidParam {
    InvalidParam {
        name: name.into(),
        reason: reason.into(),
    }
}

fn export_id(raw: &str) -> AppResult<Uuid> {
    Uuid::parse_str(raw).map_err(|_| AppError::NotFound)
}

async fn create(
    auth: Require<perm::ExportDeidentified>,
    State(state): State<AppState>,
    meta: RequestMeta,
    ApiJson(req): ApiJson<ExportRequest>,
) -> AppResult<(StatusCode, Json<Export>)> {
    let org = auth.org();
    let role = auth.principal.role;
    if req.mode == ExportMode::Identified {
        rbac::authorize(&auth.principal, Action::ExportIdentified, Some(org))?;
    }

    // ---- body validation (every problem at once)
    let mut errors = Vec::new();
    let reason = req.reason.trim().to_owned();
    let chars = reason.chars().count();
    if !(MIN_REASON_CHARS..=MAX_REASON_CHARS).contains(&chars) {
        errors.push(bad(
            "reason",
            format!("must be {MIN_REASON_CHARS}..{MAX_REASON_CHARS} characters"),
        ));
    } else if reason.chars().any(|c| c.is_control() && c != '\n') {
        errors.push(bad("reason", "must not contain control characters"));
    }
    if let Err(why) = export_build::format_available(req.format) {
        errors.push(bad("format", why));
    }
    if req.format == ExportFormat::Hl7v2 && req.mode == ExportMode::Deidentified {
        errors.push(bad("mode", "hl7v2 exports are identified only (HL7-3)"));
    }
    if export_build::single_session(req.format) && req.filters.session_ids.len() != 1 {
        errors.push(bad(
            "filters.session_ids",
            format!(
                "{} holds exactly one session; name it here",
                req.format.as_str()
            ),
        ));
    }
    if let Err(AppError::Validation(p)) = filters::parse(&req.filters) {
        errors.extend(p);
    }
    let mut destinations = Vec::with_capacity(req.destination_ids.len());
    for raw in &req.destination_ids {
        match Uuid::parse_str(raw) {
            Ok(id) => destinations.push(id),
            Err(_) => {
                errors.push(bad("destination_ids", "must be UUIDs"));
                break;
            }
        }
    }
    destinations.sort();
    destinations.dedup();
    if !destinations.is_empty() {
        let known: i64 = sqlx::query_scalar!(
            r#"SELECT count(*) AS "n!" FROM integration_destinations
               WHERE org_id = $1 AND id = ANY($2) AND status = 'active'"#,
            org,
            &destinations,
        )
        .fetch_one(&state.db)
        .await?;
        if usize::try_from(known).unwrap_or(0) != destinations.len() {
            errors.push(bad("destination_ids", "unknown or disabled destination"));
        }
    }
    if !errors.is_empty() {
        return Err(AppError::Validation(errors));
    }

    // ---- consent purpose (XC-5)
    asd_export::policy::validate_request(
        export_build::policy_purpose(req.purpose),
        export_build::policy_mode(req.mode),
    )
    .map_err(|e| AppError::BadRequest {
        slug: CONSENT_PURPOSE_MISMATCH,
        detail: e.to_string(),
    })?;

    // ---- org settings: MRN mode (MRN-2, EXJ-2) and artifact lifetime (EXJ-4)
    let settings: serde_json::Value =
        sqlx::query_scalar!("SELECT settings FROM organizations WHERE id = $1", org)
            .fetch_one(&state.db)
            .await?;
    let mrn_mode = settings
        .get(org_settings::IDENTITY)
        .and_then(|i| i.get(org_settings::PATIENT_REF_MODE))
        .and_then(serde_json::Value::as_str)
        .and_then(PatientRefMode::parse)
        == Some(PatientRefMode::Mrn);
    if export_build::needs_mrn(req.format) && !mrn_mode {
        return Err(AppError::Conflict {
            slug: PATIENT_REF_DISABLED,
            detail: "hl7v2 exports need the organization's MRN mapping (identity.patient_ref_mode = mrn)"
                .into(),
        });
    }
    let ttl = export_build::ttl_hours(
        settings
            .get(asd_contract::export::settings::EXPORTS)
            .and_then(|e| e.get(asd_contract::export::settings::TTL_HOURS))
            .and_then(serde_json::Value::as_i64),
    );
    let identified = req.mode == ExportMode::Identified;
    let include_clinical_content = identified && rbac::allows(role, Action::ClinicalContentRead);
    let include_mrn = identified && mrn_mode && rbac::allows(role, Action::PatientRefRead);

    let id = Uuid::new_v4();
    let mut secret = [0u8; 32];
    getrandom::fill(&mut secret).map_err(|e| AppError::Internal(format!("randomness: {e}")))?;
    let filters_json = serde_json::to_value(&req.filters).expect("plain data");
    let mut tx = state.db.begin().await?;
    let job = jobs::enqueue(
        &mut tx,
        KIND_EXPORT,
        Some(org),
        &export_job::params(id),
        Some(auth.principal.user_id),
    )
    .await?;
    sqlx::query!(
        r#"INSERT INTO exports
             (id, org_id, format, mode, purpose, filters, reason, destination_ids,
              include_clinical_content, include_mrn, requested_by, job_id, ttl_hours,
              download_secret)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)"#,
        id,
        org,
        req.format.as_str(),
        req.mode.as_str(),
        req.purpose.as_str(),
        filters_json,
        reason,
        &destinations,
        include_clinical_content,
        include_mrn,
        auth.principal.user_id,
        job.id,
        ttl,
        &secret[..],
    )
    .execute(&mut *tx)
    .await?;
    // WP-I.7 (DLV-1): one pending delivery per destination, queued when the export succeeds.
    crate::interop::outbox::create_deliveries(
        &mut tx,
        org,
        id,
        &destinations,
        asd_contract::export::DeliveryEvent::ExportReady,
    )
    .await?;
    AuditEvent::user(&auth.principal, "export.create", "export", id)
        .org(org)
        .after(json!({
            "format": req.format.as_str(),
            "mode": req.mode.as_str(),
            "purpose": req.purpose.as_str(),
            "filters": filters_json,
            "destination_ids": destinations,
            "reason": reason,
            "include_clinical_content": include_clinical_content,
            "include_mrn": include_mrn,
            "job_id": job.id,
        }))
        .meta(&meta)
        .record(&mut tx)
        .await?;
    tx.commit().await?;
    let row = export_build::load_export(&state.db, id)
        .await?
        .ok_or_else(|| AppError::Internal("export vanished after insert".into()))?;
    Ok((StatusCode::ACCEPTED, Json(row.to_dto())))
}

#[derive(Debug, Default, Deserialize)]
struct ListQuery {
    cursor: Option<String>,
    limit: Option<String>,
}

async fn list(
    auth: Require<perm::ExportRead>,
    State(state): State<AppState>,
    Query(q): Query<ListQuery>,
) -> AppResult<Json<ExportPage>> {
    let org = auth.org();
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
    let (after_at, after_id) = match &after {
        Some(c) => (
            Some(c.at),
            Some(Uuid::parse_str(&c.id).map_err(|_| {
                AppError::Validation(vec![bad("cursor", "is not a cursor of this list")])
            })?),
        ),
        None => (None, None),
    };
    let ids: Vec<(Uuid, chrono::DateTime<Utc>)> = sqlx::query!(
        r#"SELECT id, created_at FROM exports
           WHERE org_id = $1
             AND ($2::timestamptz IS NULL OR (created_at, id) < ($2, $3::uuid))
           ORDER BY created_at DESC, id DESC LIMIT $4"#,
        org,
        after_at,
        after_id,
        limit + 1,
    )
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .map(|r| (r.id, r.created_at))
    .collect();
    let total: i64 = sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!" FROM exports WHERE org_id = $1"#,
        org
    )
    .fetch_one(&state.db)
    .await?;
    let take = usize::try_from(limit).unwrap_or(usize::MAX);
    let has_more = ids.len() > take;
    let mut items = Vec::new();
    let mut last = None;
    for (id, at) in ids.into_iter().take(take) {
        if let Some(row) = export_build::load_export(&state.db, id).await? {
            items.push(row.to_dto());
        }
        last = Some(Cursor::new(at, id.to_string()));
    }
    Ok(Json(ExportPage {
        items,
        next_cursor: if has_more {
            last.map(|c| c.encode())
        } else {
            None
        },
        total: Some(u64::try_from(total).unwrap_or(0)),
    }))
}

async fn load_in_org(
    state: &AppState,
    raw: &str,
    check: impl FnOnce(Uuid) -> AppResult<()>,
) -> AppResult<ExportRow> {
    let row = export_build::load_export(&state.db, export_id(raw)?)
        .await?
        .ok_or(AppError::NotFound)?;
    check(row.org_id)?;
    Ok(row)
}

async fn read(
    auth: Require<perm::ExportRead>,
    State(state): State<AppState>,
    Path(raw): Path<String>,
) -> AppResult<Json<Export>> {
    let row = load_in_org(&state, &raw, |org| auth.ensure_org(org)).await?;
    Ok(Json(row.to_dto()))
}

fn expired() -> AppError {
    AppError::Gone {
        slug: EXPORT_EXPIRED,
        detail: "the export artifact has expired; request a new export".into(),
    }
}

/// `410` for an expired artifact, `409` for one not ready (or failed).
fn ensure_downloadable(row: &ExportRow) -> AppResult<String> {
    let past_expiry = row.expires_at.is_some_and(|t| t <= Utc::now());
    match row.effective_status() {
        ExportStatus::Expired => Err(expired()),
        ExportStatus::Succeeded if past_expiry => Err(expired()),
        ExportStatus::Succeeded => row
            .blob_key
            .clone()
            .ok_or_else(|| AppError::Internal("succeeded export without a blob".into())),
        s => Err(AppError::Conflict {
            slug: CONFLICT,
            detail: format!("the export is {}; nothing to download", s.as_str()),
        }),
    }
}

async fn download_url(
    auth: Require<perm::ExportRead>,
    State(state): State<AppState>,
    meta: RequestMeta,
    Path(raw): Path<String>,
) -> AppResult<Json<DownloadUrl>> {
    let p = auth.principal;
    // RBAC-3: the vendor operates the system but never takes copies of clinic data.
    if p.is_superadmin() {
        return Err(AppError::Forbidden);
    }
    let row = load_in_org(&state, &raw, |org| auth.ensure_org(org)).await?;
    if row.mode == ExportMode::Identified
        && row.requested_by != Some(p.user_id)
        && !rbac::allows(p.role, Action::ExportIdentified)
    {
        return Err(AppError::Forbidden);
    }
    let key = ensure_downloadable(&row)?;
    let file_name = row
        .file_name
        .clone()
        .ok_or_else(|| AppError::Internal("succeeded export without a file name".into()))?;
    let ttl = Duration::from_secs(DOWNLOAD_URL_TTL_SECS);
    let (url, expires_at, mode) = match state.config.blob_mode {
        BlobMode::Presigned => {
            let get = state.blobs.presign_get(&key, ttl, &file_name).await?;
            (get.url, get.expires_at, "presigned")
        }
        BlobMode::Proxy => {
            let expires_at =
                Utc::now() + chrono::Duration::from_std(ttl).expect("five minutes fits in chrono");
            let token = export_build::download_token(
                &row.download_secret,
                row.id,
                p.user_id,
                expires_at.timestamp(),
            );
            (
                format!("/v1/exports/{}/file?t={token}", row.id),
                expires_at,
                "proxy",
            )
        }
    };
    let mut tx = state.db.begin().await?;
    AuditEvent::user(&p, "export.download", "export", row.id)
        .org(row.org_id)
        .after(json!({"url_mode": mode, "expires_at": cursor::ts(expires_at)}))
        .meta(&meta)
        .record(&mut tx)
        .await?;
    tx.commit().await?;
    Ok(Json(DownloadUrl {
        url,
        expires_at: cursor::ts(expires_at),
    }))
}

#[derive(Debug, Deserialize)]
struct FileQuery {
    t: Option<String>,
}

/// Proxy-mode download. Bad, tampered or expired tokens are `403`; an unknown export or
/// another blob mode is `404`. The query string is never logged (OBS-1).
async fn file(
    State(state): State<AppState>,
    Path(raw): Path<String>,
    Query(q): Query<FileQuery>,
) -> AppResult<Response> {
    if state.config.blob_mode != BlobMode::Proxy {
        return Err(AppError::NotFound);
    }
    let row = export_build::load_export(&state.db, export_id(&raw)?)
        .await?
        .ok_or(AppError::NotFound)?;
    let token = q.t.as_deref().ok_or(AppError::Forbidden)?;
    export_build::verify_download_token(
        &row.download_secret,
        row.id,
        token,
        Utc::now().timestamp(),
    )
    .ok_or(AppError::Forbidden)?;
    let key = ensure_downloadable(&row)?;
    let blob = state.blobs.get(&key).await?.ok_or(expired())?;
    let file_name = row.file_name.unwrap_or_else(|| "export.bin".into());
    let mut response = Response::new(Body::from(blob.bytes));
    let h = response.headers_mut();
    h.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&blob.content_type)
            .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream")),
    );
    if let Ok(v) = HeaderValue::from_str(&format!("attachment; filename=\"{file_name}\"")) {
        h.insert(header::CONTENT_DISPOSITION, v);
    }
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    h.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    Ok(response)
}

async fn disclosures(
    auth: Require<perm::ClinicalContentRead>,
    State(state): State<AppState>,
    Path(raw): Path<String>,
) -> AppResult<Json<DisclosurePage>> {
    let (participant, org) = participant_org(&state.db, &raw).await?;
    auth.ensure_org(org)?;
    let rows = sqlx::query!(
        r#"SELECT e.id, array_agg(i.session_id ORDER BY i.session_id) AS "session_ids!"
           FROM exports e JOIN export_items i ON i.export_id = e.id
           WHERE e.org_id = $1 AND i.participant_id = $2
           GROUP BY e.id, e.created_at
           ORDER BY e.created_at DESC, e.id DESC"#,
        org,
        participant,
    )
    .fetch_all(&state.db)
    .await?;
    let export_ids: Vec<Uuid> = rows.iter().map(|r| r.id).collect();
    let mut deliveries: BTreeMap<Uuid, Vec<DisclosureDelivery>> = BTreeMap::new();
    for d in sqlx::query!(
        r#"SELECT d.export_id, d.destination_id, n.name, d.status, d.delivered_at
           FROM export_deliveries d JOIN integration_destinations n ON n.id = d.destination_id
           WHERE d.export_id = ANY($1) ORDER BY d.created_at, d.id"#,
        &export_ids,
    )
    .fetch_all(&state.db)
    .await?
    {
        deliveries
            .entry(d.export_id)
            .or_default()
            .push(DisclosureDelivery {
                destination_id: d.destination_id.to_string(),
                destination_name: d.name,
                status: d.status,
                delivered_at: d.delivered_at.map(cursor::ts),
            });
    }
    let mut items = Vec::with_capacity(rows.len());
    for r in rows {
        let Some(e) = export_build::load_export(&state.db, r.id).await? else {
            continue;
        };
        items.push(Disclosure {
            export_id: e.id.to_string(),
            kind: e.kind,
            format: e.format,
            mode: e.mode,
            purpose: e.purpose,
            status: e.effective_status(),
            requested_by: e.requested_by.map(|u| u.to_string()),
            created_at: cursor::ts(e.created_at),
            session_ids: r.session_ids,
            deliveries: deliveries.remove(&e.id).unwrap_or_default(),
        });
    }
    let total = u64::try_from(items.len()).unwrap_or(u64::MAX);
    Ok(Json(DisclosurePage {
        items,
        next_cursor: None,
        total: Some(total),
    }))
}
