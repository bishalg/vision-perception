//! owner: WP-C.11
//!
//! Hospital MRN mapping (MRN-1..6, ADR-0008 D4):
//!
//! - `PUT/GET/DELETE /v1/participants/{id}/patient-ref`
//! - `POST /v1/orgs/{id}/patient-refs/search`: the MRN is in the body, never in a URL (MRN-4)
//!
//! All four answer `409 patient-ref-disabled` unless the org opted in with
//! `settings.identity.patient_ref_mode = "mrn"` (MRN-2). The MRN is encrypted in
//! `patient_refs.mrn_ct` (AAD = `patient_refs:mrn_ct:{org}:{row id}`) and found through the
//! blind index `mrn_bidx` over `(authority, mrn)` after normalisation, so `tu-10042` and
//! `TU 10042` are one MRN. Every read, write, delete and search appends one `audit_log` row
//! in the handler's transaction with `target_type = "patient_ref"` and no MRN in it
//! (MRN-5). The authority code is stored trimmed and upper-cased, so the unique index on
//! `(org_id, assigning_authority, mrn_bidx)` also refuses case variants.

use asd_contract::clinical::{
    PatientRef, PatientRefMode, PatientRefSearchRequest, PatientRefSearchResponse,
    PutPatientRefRequest, MAX_ASSIGNING_AUTHORITY_CHARS, MAX_MRN_CHARS,
};
use asd_contract::problem::PATIENT_REF_DISABLED;
use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use http::StatusCode;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::app::AppState;
use crate::audit::AuditEvent;
use crate::auth::extractors::{ApiJson, RequestMeta, Require};
use crate::auth::rbac::perm;
use crate::crypto::blind_index::normalize;
use crate::crypto::{Aad, ClinicalCrypto};
use crate::error::{AppError, AppResult};
use crate::problem::InvalidParam;
use crate::read::cursor;
use crate::routes::participant_detail::participant_org;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/participants/{id}/patient-ref",
            get(read).put(put).delete(remove),
        )
        .route("/orgs/{id}/patient-refs/search", post(search))
}

/// MRN-2: 409 unless the org's `identity.patient_ref_mode` is `mrn`.
async fn ensure_enabled(db: &PgPool, org: Uuid) -> AppResult<()> {
    let mode: Option<String> = sqlx::query_scalar!(
        "SELECT settings #>> '{identity,patient_ref_mode}' FROM organizations WHERE id = $1",
        org
    )
    .fetch_one(db)
    .await?;
    if mode.as_deref().and_then(PatientRefMode::parse) == Some(PatientRefMode::Mrn) {
        Ok(())
    } else {
        Err(AppError::Conflict {
            slug: PATIENT_REF_DISABLED,
            detail: "this organization does not store hospital patient references".into(),
        })
    }
}

fn invalid(name: &str, reason: &str) -> AppError {
    AppError::Validation(vec![InvalidParam {
        name: name.into(),
        reason: reason.into(),
    }])
}

/// A trimmed value of 1..=`max` characters without control characters that still has
/// content after normalisation. Returns the trimmed text.
fn clean(name: &str, raw: &str, max: usize) -> AppResult<String> {
    let t = raw.trim();
    if t.is_empty() || normalize(t).is_empty() {
        return Err(invalid(name, "must not be empty"));
    }
    if t.chars().count() > max {
        return Err(invalid(name, &format!("must be at most {max} characters")));
    }
    if t.chars().any(char::is_control) {
        return Err(invalid(name, "must not contain control characters"));
    }
    Ok(t.to_owned())
}

fn authority(raw: &str) -> AppResult<String> {
    clean("assigning_authority", raw, MAX_ASSIGNING_AUTHORITY_CHARS).map(|a| a.to_uppercase())
}

fn mrn(raw: &str) -> AppResult<String> {
    clean("mrn", raw, MAX_MRN_CHARS)
}

fn conflict() -> AppError {
    AppError::Conflict {
        slug: asd_contract::problem::CONFLICT,
        detail: "this patient reference is already linked to a participant".into(),
    }
}

fn is_unique_violation(e: &sqlx::Error) -> bool {
    matches!(e, sqlx::Error::Database(d) if d.code().as_deref() == Some("23505"))
}

async fn put(
    auth: Require<perm::PatientRefWrite>,
    crypto: ClinicalCrypto,
    State(state): State<AppState>,
    meta: RequestMeta,
    Path(raw_id): Path<String>,
    ApiJson(req): ApiJson<PutPatientRefRequest>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    let (participant, org) = participant_org(&state.db, &raw_id).await?;
    auth.ensure_org(org)?;
    ensure_enabled(&state.db, org).await?;
    let authority = authority(&req.assigning_authority)?;
    let mrn = mrn(&req.mrn)?;
    let bidx = crypto.mrn_blind_index(org, &authority, &mrn).await?;

    // Encrypt before the transaction (the keyring may create the org's key on first use).
    let known: Option<Uuid> = sqlx::query_scalar!(
        "SELECT id FROM patient_refs WHERE participant_id = $1",
        participant
    )
    .fetch_optional(&state.db)
    .await?;
    let row_id = known.unwrap_or_else(Uuid::new_v4);
    let aad = Aad::new("patient_refs", "mrn_ct", org, row_id);
    let sealed = crypto.encrypt_str(org, &aad, &mrn).await?;

    let mut tx = state.db.begin().await?;
    // Serialise writers of one identity so the duplicate check below cannot race.
    sqlx::query!(
        "SELECT pg_advisory_xact_lock(hashtextextended($1, 0))",
        format!("patient_ref:{org}:{}", hex(&bidx))
    )
    .fetch_one(&mut *tx)
    .await?;
    let owner: Option<Uuid> = sqlx::query_scalar!(
        "SELECT participant_id FROM patient_refs WHERE org_id = $1 AND mrn_bidx = $2",
        org,
        &bidx[..]
    )
    .fetch_optional(&mut *tx)
    .await?;
    if owner.is_some_and(|o| o != participant) {
        return Err(conflict());
    }
    let current: Option<Uuid> = sqlx::query_scalar!(
        "SELECT id FROM patient_refs WHERE participant_id = $1 FOR UPDATE",
        participant
    )
    .fetch_optional(&mut *tx)
    .await?;
    if current != known {
        return Err(conflict()); // changed by a concurrent writer: retry
    }
    let updated_at: DateTime<Utc> = if known.is_some() {
        sqlx::query_scalar!(
            r#"UPDATE patient_refs
               SET assigning_authority = $2, mrn_ct = $3, mrn_key_id = $4, mrn_bidx = $5,
                   updated_by = $6, updated_at = now()
               WHERE id = $1 RETURNING updated_at"#,
            row_id,
            authority,
            sealed.ciphertext,
            sealed.key_id,
            &bidx[..],
            auth.principal.user_id,
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| {
            if is_unique_violation(&e) {
                conflict()
            } else {
                e.into()
            }
        })?
    } else {
        sqlx::query_scalar!(
            r#"INSERT INTO patient_refs
                 (id, org_id, participant_id, assigning_authority, mrn_ct, mrn_key_id, mrn_bidx,
                  created_by, updated_by)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $8) RETURNING updated_at"#,
            row_id,
            org,
            participant,
            authority,
            sealed.ciphertext,
            sealed.key_id,
            &bidx[..],
            auth.principal.user_id,
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| {
            if is_unique_violation(&e) {
                conflict()
            } else {
                e.into()
            }
        })?
    };
    AuditEvent::user(
        &auth.principal,
        "patient_ref.put",
        "patient_ref",
        participant,
    )
    .org(org)
    .before(json!({"set": known.is_some()}))
    .after(json!({"set": true}))
    .meta(&meta)
    .record(&mut tx)
    .await?;
    tx.commit().await?;

    let status = if known.is_some() {
        StatusCode::OK
    } else {
        StatusCode::CREATED
    };
    // The MRN is not echoed: the caller just sent it.
    Ok((
        status,
        Json(json!({
            "participant_id": participant.to_string(),
            "assigning_authority": authority,
            "updated_at": cursor::ts(updated_at),
        })),
    ))
}

async fn read(
    auth: Require<perm::PatientRefRead>,
    crypto: ClinicalCrypto,
    State(state): State<AppState>,
    meta: RequestMeta,
    Path(raw_id): Path<String>,
) -> AppResult<Json<PatientRef>> {
    let (participant, org) = participant_org(&state.db, &raw_id).await?;
    auth.ensure_org(org)?;
    ensure_enabled(&state.db, org).await?;
    let row = sqlx::query!(
        r#"SELECT id, assigning_authority, mrn_ct, mrn_key_id, updated_at
           FROM patient_refs WHERE participant_id = $1"#,
        participant
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;
    let aad = Aad::new("patient_refs", "mrn_ct", org, row.id);
    let mrn = crypto
        .decrypt_string(org, &aad, row.mrn_key_id, &row.mrn_ct)
        .await?;
    // MRN-5: the decrypt is audited before the value leaves; no audit row, no response.
    let mut tx = state.db.begin().await?;
    AuditEvent::user(
        &auth.principal,
        "patient_ref.read",
        "patient_ref",
        participant,
    )
    .org(org)
    .after(json!({"set": true}))
    .meta(&meta)
    .record(&mut tx)
    .await?;
    tx.commit().await?;
    Ok(Json(PatientRef {
        participant_id: participant.to_string(),
        assigning_authority: row.assigning_authority,
        mrn,
        updated_at: cursor::ts(row.updated_at),
    }))
}

async fn remove(
    auth: Require<perm::PatientRefWrite>,
    _crypto: ClinicalCrypto,
    State(state): State<AppState>,
    meta: RequestMeta,
    Path(raw_id): Path<String>,
) -> AppResult<StatusCode> {
    let (participant, org) = participant_org(&state.db, &raw_id).await?;
    auth.ensure_org(org)?;
    ensure_enabled(&state.db, org).await?;
    let mut tx = state.db.begin().await?;
    let gone = sqlx::query_scalar!(
        "DELETE FROM patient_refs WHERE participant_id = $1 RETURNING id",
        participant
    )
    .fetch_optional(&mut *tx)
    .await?;
    if gone.is_none() {
        return Err(AppError::NotFound);
    }
    AuditEvent::user(
        &auth.principal,
        "patient_ref.delete",
        "patient_ref",
        participant,
    )
    .org(org)
    .before(json!({"set": true}))
    .after(json!({"set": false}))
    .meta(&meta)
    .record(&mut tx)
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn search(
    auth: Require<perm::PatientRefRead>,
    crypto: ClinicalCrypto,
    State(state): State<AppState>,
    meta: RequestMeta,
    ApiJson(req): ApiJson<PatientRefSearchRequest>,
) -> AppResult<Json<PatientRefSearchResponse>> {
    let org = auth.org();
    ensure_enabled(&state.db, org).await?;
    let authority = authority(&req.assigning_authority)?;
    let mrn = mrn(&req.mrn)?;
    let bidx = crypto.mrn_blind_index(org, &authority, &mrn).await?;
    let found: Option<Uuid> = sqlx::query_scalar!(
        "SELECT participant_id FROM patient_refs WHERE org_id = $1 AND mrn_bidx = $2",
        org,
        &bidx[..]
    )
    .fetch_optional(&state.db)
    .await?;
    let mut tx = state.db.begin().await?;
    AuditEvent::user(
        &auth.principal,
        "patient_ref.search",
        "patient_ref",
        found.unwrap_or(org),
    )
    .org(org)
    .after(json!({"found": found.is_some()}))
    .meta(&meta)
    .record(&mut tx)
    .await?;
    tx.commit().await?;
    Ok(Json(PatientRefSearchResponse {
        participant_id: found.map(|p| p.to_string()),
    }))
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}
