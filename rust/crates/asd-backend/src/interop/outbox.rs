//! owner: WP-I.7
//!
//! The delivery outbox (DLV-1, DLV-6).
//!
//! - A delivery row (`export_deliveries`, `pending`, no job) is written in the same
//!   transaction as its export: by `POST /v1/orgs/{id}/exports` for the named destinations
//!   ([`create_deliveries`]), or by the signing transaction for auto-deliveries
//!   ([`enqueue_auto_deliveries`]).
//! - When the export job records success it calls [`enqueue_ready`] in its record
//!   transaction: one `export_deliver` job per pending delivery. A delivery job therefore
//!   never waits on an unfinished export, and an export that fails takes its deliveries
//!   with it ([`fail_for_export`]).

use asd_contract::clinical::{settings as org_settings, PatientRefMode};
use asd_contract::export::{
    DeliveryEvent, DeliveryTrigger, ExportFilters, ExportFormat, ExportMode, ExportPurpose,
    IntegrationConfig,
};
use serde_json::{json, Value};
use sqlx::{Connection, PgConnection, PgPool};
use uuid::Uuid;

use super::ErrorClass;
use crate::audit::AuditEvent;
use crate::error::{AppError, AppResult};
use crate::export_build;
use crate::jobs::{self, export as export_job, KIND_DELIVER, KIND_EXPORT};

/// Job params of a delivery.
pub fn deliver_params(delivery_id: Uuid) -> Value {
    json!({ "delivery_id": delivery_id })
}

/// One `pending` delivery per destination of `export_id` (no job yet; [`enqueue_ready`]).
pub async fn create_deliveries(
    conn: &mut PgConnection,
    org: Uuid,
    export_id: Uuid,
    destinations: &[Uuid],
    event: DeliveryEvent,
) -> AppResult<()> {
    if destinations.is_empty() {
        return Ok(());
    }
    sqlx::query!(
        "INSERT INTO export_deliveries (org_id, export_id, destination_id, event)
         SELECT $1, $2, d, $4 FROM unnest($3::uuid[]) AS t (d)
         ON CONFLICT (export_id, destination_id) DO NOTHING",
        org,
        export_id,
        destinations,
        event.as_str(),
    )
    .execute(&mut *conn)
    .await?;
    Ok(())
}

/// Queues one `export_deliver` job per pending, job-less delivery of `export_id`. Called
/// in the export job's record transaction; re-running it queues nothing twice.
pub async fn enqueue_ready(conn: &mut PgConnection, export_id: Uuid) -> AppResult<u64> {
    let pending = sqlx::query!(
        "SELECT id, org_id FROM export_deliveries
         WHERE export_id = $1 AND status = 'pending' AND job_id IS NULL
         ORDER BY created_at, id
         FOR UPDATE",
        export_id
    )
    .fetch_all(&mut *conn)
    .await?;
    let mut queued = 0;
    for d in pending {
        let job = jobs::enqueue(
            conn,
            KIND_DELIVER,
            Some(d.org_id),
            &deliver_params(d.id),
            None,
        )
        .await?;
        sqlx::query!(
            "UPDATE export_deliveries SET job_id = $2, updated_at = now() WHERE id = $1",
            d.id,
            job.id
        )
        .execute(&mut *conn)
        .await?;
        queued += 1;
    }
    Ok(queued)
}

/// Marks the not-yet-sent deliveries of an export that will never succeed as `failed`.
pub async fn fail_for_export(pool: &PgPool, export_id: Uuid, class: ErrorClass) -> AppResult<()> {
    sqlx::query!(
        "UPDATE export_deliveries
         SET status = 'failed', last_error_class = $2, updated_at = now()
         WHERE export_id = $1 AND status = 'pending'",
        export_id,
        class.as_str(),
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// The reason stored on an auto-export (EXJ-1: 10..500 characters, shown in the audit log).
fn auto_reason(review_id: Uuid) -> String {
    format!("Automatic delivery on final sign of clinical review {review_id}")
}

/// DLV-6, called by [`super::on_review_finalized`]: inside a savepoint of the signing
/// transaction, one export and one delivery per eligible destination. Never fails the
/// caller (see there).
pub async fn enqueue_auto_deliveries(tx: &mut PgConnection, review_id: Uuid) {
    let mut savepoint = match tx.begin().await {
        Ok(sp) => sp,
        Err(e) => {
            tracing::error!(%review_id, error = %e, "auto-delivery skipped: savepoint failed");
            return;
        }
    };
    match auto_deliveries(&mut savepoint, review_id).await {
        Ok(0) => {
            let _ = savepoint.commit().await;
        }
        Ok(n) => match savepoint.commit().await {
            Ok(()) => tracing::info!(%review_id, exports = n, "auto-deliveries enqueued"),
            Err(e) => {
                tracing::error!(%review_id, error = %e, "auto-delivery skipped: release failed")
            }
        },
        Err(e) => {
            tracing::error!(%review_id, error = %e, "auto-delivery skipped; the signature stands");
            if let Err(e) = savepoint.rollback().await {
                tracing::error!(%review_id, error = %e, "rollback to savepoint failed");
            }
        }
    }
}

async fn auto_deliveries(conn: &mut PgConnection, review_id: Uuid) -> AppResult<usize> {
    let review = sqlx::query!(
        "SELECT org_id, session_id FROM clinical_reviews WHERE id = $1",
        review_id
    )
    .fetch_one(&mut *conn)
    .await?;
    let org = review.org_id;
    let destinations = sqlx::query!(
        r#"SELECT id, kind, config FROM integration_destinations
           WHERE org_id = $1 AND status = 'active' AND environment = 'production'
             AND kind IN ('webhook', 'sftp')
             AND config ->> 'trigger' = 'on_final_sign'
           ORDER BY created_at, id"#,
        org
    )
    .fetch_all(&mut *conn)
    .await?;
    if destinations.is_empty() {
        return Ok(0);
    }

    // The finaliser (the last signature) requested the export.
    let signer: Option<Uuid> = sqlx::query_scalar!(
        "SELECT signer_id FROM clinical_signatures WHERE review_id = $1
         ORDER BY signed_at DESC, id DESC LIMIT 1",
        review_id
    )
    .fetch_optional(&mut *conn)
    .await?;
    let settings: Value =
        sqlx::query_scalar!("SELECT settings FROM organizations WHERE id = $1", org)
            .fetch_one(&mut *conn)
            .await?;
    let mrn_mode = settings
        .get(org_settings::IDENTITY)
        .and_then(|i| i.get(org_settings::PATIENT_REF_MODE))
        .and_then(Value::as_str)
        .and_then(PatientRefMode::parse)
        == Some(PatientRefMode::Mrn);
    let ttl = export_build::ttl_hours(
        settings
            .get(asd_contract::export::settings::EXPORTS)
            .and_then(|e| e.get(asd_contract::export::settings::TTL_HOURS))
            .and_then(Value::as_i64),
    );
    let filters = serde_json::to_value(ExportFilters {
        session_ids: vec![review.session_id.clone()],
        ..ExportFilters::default()
    })
    .expect("plain data");

    let mut made = 0;
    for d in destinations {
        let config: IntegrationConfig = match serde_json::from_value(d.config) {
            Ok(c) => c,
            Err(_) => {
                tracing::warn!(destination = %d.id, "auto-delivery skipped: config does not parse");
                continue;
            }
        };
        if config.trigger != Some(DeliveryTrigger::OnFinalSign) {
            continue;
        }
        if d.kind == "webhook"
            && !config.events.is_empty()
            && !config.events.contains(&DeliveryEvent::ReviewFinalized)
        {
            continue;
        }
        let Some(format) = config
            .format
            .filter(|f| export_build::format_available(*f).is_ok())
        else {
            tracing::warn!(destination = %d.id, "auto-delivery skipped: no available format");
            continue;
        };
        create_auto_export(
            conn,
            AutoExport {
                org,
                review_id,
                destination: d.id,
                format,
                filters: &filters,
                requested_by: signer,
                include_mrn: mrn_mode,
                ttl_hours: ttl,
            },
        )
        .await?;
        made += 1;
    }
    Ok(made)
}

struct AutoExport<'a> {
    org: Uuid,
    review_id: Uuid,
    destination: Uuid,
    format: ExportFormat,
    filters: &'a Value,
    requested_by: Option<Uuid>,
    include_mrn: bool,
    ttl_hours: i32,
}

async fn create_auto_export(conn: &mut PgConnection, a: AutoExport<'_>) -> AppResult<Uuid> {
    let id = Uuid::new_v4();
    let mut secret = [0u8; 32];
    getrandom::fill(&mut secret).map_err(|e| AppError::Internal(format!("randomness: {e}")))?;
    let job = jobs::enqueue(
        conn,
        KIND_EXPORT,
        Some(a.org),
        &export_job::params(id),
        a.requested_by,
    )
    .await?;
    let reason = auto_reason(a.review_id);
    sqlx::query!(
        r#"INSERT INTO exports
             (id, org_id, format, mode, purpose, filters, reason, destination_ids,
              include_clinical_content, include_mrn, requested_by, job_id, ttl_hours,
              download_secret, review_id)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, true, $9, $10, $11, $12, $13, $14)"#,
        id,
        a.org,
        a.format.as_str(),
        ExportMode::Identified.as_str(),
        ExportPurpose::ClinicalCare.as_str(),
        a.filters,
        reason,
        &[a.destination][..],
        a.include_mrn,
        a.requested_by,
        job.id,
        a.ttl_hours,
        &secret[..],
        a.review_id,
    )
    .execute(&mut *conn)
    .await?;
    create_deliveries(
        conn,
        a.org,
        id,
        &[a.destination],
        DeliveryEvent::ReviewFinalized,
    )
    .await?;
    AuditEvent::system("export.create", "export", id)
        .org(a.org)
        .after(json!({
            "format": a.format.as_str(),
            "mode": ExportMode::Identified.as_str(),
            "purpose": ExportPurpose::ClinicalCare.as_str(),
            "filters": a.filters,
            "destination_ids": [a.destination],
            "reason": reason,
            "trigger": DeliveryTrigger::OnFinalSign.as_str(),
            "review_id": a.review_id,
            "include_clinical_content": true,
            "include_mrn": a.include_mrn,
            "job_id": job.id,
        }))
        .record(conn)
        .await?;
    Ok(id)
}
