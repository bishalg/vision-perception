//! owner: WP-I.7
//!
//! The `export_deliver` job (DLV-1..5): one delivery of one export to one destination, a
//! phased handler (G5, trap 3):
//!
//! 1. **Load** (autocommit reads): the delivery, its destination and export. A delivery
//!    already `delivered` returns at once (trap 4: a reclaimed job may run twice); one
//!    `failed` stays failed. The row goes to `sending` with `attempts + 1`.
//! 2. **Send**, no transaction open: open the destination secret, then a signed webhook
//!    with a fresh 15-minute download link ([`interop::webhook`]) or an SFTP drop of the
//!    artifact ([`interop::sftp`]).
//! 3. **Record**: `delivered` with the remote reference and one `export.deliver` audit row
//!    in one transaction; or back to `pending` with the error class and a
//!    [`JobError::RetryAfter`] of the DLV-1 backoff (1 min, 5 min, 30 min, 2 h, 12 h), or
//!    `failed` when the class cannot heal by itself or the attempts are used up
//!    ([`runner::DELIVERY_MAX_ATTEMPTS`]).
//!
//! Only [`ErrorClass`] spellings and `http NNN` references are stored in
//! `export_deliveries`, `jobs.error` and the audit log (trap 20).

use std::time::Duration;

use asd_contract::export::{
    DeliveryEvent, DeliveryStatus, ExportStatus, IntegrationKind, IntegrationStatus,
    DELIVERY_BACKOFF_SECS, WEBHOOK_DOWNLOAD_TTL_SECS,
};
use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use uuid::Uuid;

use super::runner::{JobError, DELIVERY_MAX_ATTEMPTS};
use super::{Job, JobContext, KIND_DELIVER};
use crate::audit::AuditEvent;
use crate::blob::BlobError;
use crate::config::BlobMode;
use crate::export_build::{self, ExportRow};
use crate::interop::destination::{self, DestinationRow};
use crate::interop::webhook::{self, DownloadLink, WebhookEvent};
use crate::interop::{sftp, ssh_keys, DeliveryFailure, ErrorClass};
use crate::read::cursor;

/// `export_deliveries.remote_ref` is at most this long.
const MAX_REMOTE_REF: usize = 200;

/// The delay before the next attempt after attempt `attempts` failed (DLV-1).
pub fn backoff(attempts: i32) -> Duration {
    let i = usize::try_from(attempts.max(1) - 1).unwrap_or(0);
    Duration::from_secs(DELIVERY_BACKOFF_SECS[i.min(DELIVERY_BACKOFF_SECS.len() - 1)])
}

fn permanent(m: impl Into<String>) -> JobError {
    JobError::Permanent(m.into())
}

struct DeliveryRow {
    id: Uuid,
    org_id: Uuid,
    export_id: Uuid,
    destination_id: Uuid,
    event: DeliveryEvent,
    status: DeliveryStatus,
    created_at: DateTime<Utc>,
    remote_ref: Option<String>,
}

async fn load_delivery(ctx: &JobContext, id: Uuid) -> Result<Option<DeliveryRow>, JobError> {
    let Some(r) = sqlx::query!(
        "SELECT id, org_id, export_id, destination_id, event, status, created_at, remote_ref
         FROM export_deliveries WHERE id = $1",
        id
    )
    .fetch_optional(&ctx.pool)
    .await?
    else {
        return Ok(None);
    };
    Ok(Some(DeliveryRow {
        id: r.id,
        org_id: r.org_id,
        export_id: r.export_id,
        destination_id: r.destination_id,
        event: DeliveryEvent::parse(&r.event).ok_or_else(|| permanent("stored delivery event"))?,
        status: DeliveryStatus::parse(&r.status)
            .ok_or_else(|| permanent("stored delivery status"))?,
        created_at: r.created_at,
        remote_ref: r.remote_ref,
    }))
}

fn result_of(d: &DeliveryRow, remote_ref: Option<&str>) -> Value {
    json!({
        "delivery_id": d.id,
        "export_id": d.export_id,
        "destination_id": d.destination_id,
        "remote_ref": remote_ref,
    })
}

/// Runs the delivery named in `job.params` (see the module docs).
pub async fn execute(ctx: &JobContext, job: &Job) -> Result<Value, JobError> {
    let delivery_id = job
        .params
        .get("delivery_id")
        .and_then(Value::as_str)
        .and_then(|s| Uuid::parse_str(s).ok())
        .ok_or_else(|| permanent("invalid delivery params: delivery_id"))?;
    let result = run(ctx, job, delivery_id).await;
    if let Err(e) = &result {
        if e.is_final_for(KIND_DELIVER, job.attempts) {
            // Whatever ended the job (also database trouble), the row must not stay
            // `pending` or `sending` forever.
            let marked = sqlx::query!(
                "UPDATE export_deliveries
                 SET status = 'failed', updated_at = now(),
                     last_error_class = coalesce(last_error_class, 'database-unavailable')
                 WHERE id = $1 AND status IN ('pending', 'sending')",
                delivery_id
            )
            .execute(&ctx.pool)
            .await;
            if let Err(db) = marked {
                tracing::warn!(%delivery_id, error = %db, "could not mark the delivery failed");
            }
        }
    }
    result
}

async fn run(ctx: &JobContext, job: &Job, delivery_id: Uuid) -> Result<Value, JobError> {
    let d = load_delivery(ctx, delivery_id)
        .await?
        .ok_or_else(|| permanent("delivery not found"))?;
    match d.status {
        DeliveryStatus::Delivered => return Ok(result_of(&d, d.remote_ref.as_deref())),
        DeliveryStatus::Failed => return Err(permanent("delivery is failed")),
        DeliveryStatus::Pending | DeliveryStatus::Sending => {}
    }
    let dest = destination::load(&ctx.pool, d.destination_id)
        .await?
        .ok_or_else(|| permanent("destination not found"))?;
    let export = export_build::load_export(&ctx.pool, d.export_id)
        .await?
        .ok_or_else(|| permanent("export not found"))?;

    sqlx::query!(
        "UPDATE export_deliveries
         SET status = 'sending', attempts = attempts + 1, updated_at = now()
         WHERE id = $1 AND status IN ('pending', 'sending')",
        d.id
    )
    .execute(&ctx.pool)
    .await?;

    let outcome = attempt(ctx, &d, &dest, &export).await;
    match outcome {
        Ok(remote_ref) => {
            let remote_ref: String = remote_ref.chars().take(MAX_REMOTE_REF).collect();
            let mut tx = ctx.pool.begin().await?;
            let done = sqlx::query!(
                "UPDATE export_deliveries
                 SET status = 'delivered', remote_ref = $2, last_error_class = NULL,
                     delivered_at = now(), updated_at = now()
                 WHERE id = $1 AND status IN ('pending', 'sending')",
                d.id,
                remote_ref,
            )
            .execute(&mut *tx)
            .await?;
            if done.rows_affected() == 1 {
                // EXJ-6/7: each delivery has its own audit row (no URL, no secret).
                AuditEvent::system("export.deliver", "export", d.export_id)
                    .org(d.org_id)
                    .after(json!({
                        "delivery_id": d.id,
                        "destination_id": d.destination_id,
                        "kind": dest.kind.as_str(),
                        "event": d.event.as_str(),
                        "remote_ref": remote_ref,
                        "attempts": job.attempts,
                    }))
                    .record(&mut tx)
                    .await?;
            }
            tx.commit().await?;
            Ok(result_of(&d, Some(&remote_ref)))
        }
        Err(failure) => {
            let class = failure.class;
            let final_attempt = !class.is_retryable() || job.attempts >= DELIVERY_MAX_ATTEMPTS;
            let status = if final_attempt { "failed" } else { "pending" };
            let remote_ref = failure
                .remote_ref
                .as_deref()
                .map(|r| r.chars().take(MAX_REMOTE_REF).collect::<String>());
            let mut tx = ctx.pool.begin().await?;
            sqlx::query!(
                "UPDATE export_deliveries
                 SET status = $2, last_error_class = $3, remote_ref = $4, updated_at = now()
                 WHERE id = $1 AND status IN ('pending', 'sending')",
                d.id,
                status,
                class.as_str(),
                remote_ref,
            )
            .execute(&mut *tx)
            .await?;
            if final_attempt {
                AuditEvent::system("export.deliver_failed", "export", d.export_id)
                    .org(d.org_id)
                    .after(json!({
                        "delivery_id": d.id,
                        "destination_id": d.destination_id,
                        "kind": dest.kind.as_str(),
                        "error_class": class.as_str(),
                        "remote_ref": remote_ref,
                        "attempts": job.attempts,
                    }))
                    .record(&mut tx)
                    .await?;
            }
            tx.commit().await?;
            let message = match &remote_ref {
                Some(r) => format!("{class} ({r})"),
                None => class.to_string(),
            };
            Err(if class.is_retryable() {
                JobError::RetryAfter(message, backoff(job.attempts))
            } else {
                JobError::Permanent(message)
            })
        }
    }
}

/// One send. Never returns remote text.
async fn attempt(
    ctx: &JobContext,
    d: &DeliveryRow,
    dest: &DestinationRow,
    export: &ExportRow,
) -> Result<String, DeliveryFailure> {
    if dest.status != IntegrationStatus::Active {
        return Err(ErrorClass::DestinationDisabled.into());
    }
    let past_expiry = export.expires_at.is_some_and(|t| t <= Utc::now());
    if export.effective_status() != ExportStatus::Succeeded || past_expiry {
        return Err(ErrorClass::ExportUnavailable.into());
    }
    let secret = dest.open_secret(ctx.keyring.as_deref()).await?;
    let config = dest.config();
    match dest.kind {
        IntegrationKind::Webhook => {
            let url = config.url.as_deref().ok_or(ErrorClass::InvalidConfig)?;
            let secret = std::str::from_utf8(&secret).map_err(|_| ErrorClass::MissingSecret)?;
            let key = destination::webhook_key(secret).map_err(|_| ErrorClass::MissingSecret)?;
            let download = download_link(ctx, export).await?;
            let event = WebhookEvent {
                event_type: d.event.as_str(),
                id: d.id.to_string(),
                org_id: d.org_id.to_string(),
                export_id: d.export_id.to_string(),
                format: export.format.as_str().to_owned(),
                mode: export.mode.as_str().to_owned(),
                created_at: cursor::ts(d.created_at),
                download: Some(download),
                test: None,
                test_record: None,
            };
            webhook::post(url, &key, &d.id.to_string(), event.to_body()).await
        }
        IntegrationKind::Sftp => {
            let target = sftp::SftpTarget::from_config(&config)?;
            let key = ssh_keys::private_key(&secret).ok_or(ErrorClass::MissingSecret)?;
            let (Some(blob_key), Some(file_name), Some(sha)) =
                (&export.blob_key, &export.file_name, &export.sha256)
            else {
                return Err(ErrorClass::ExportUnavailable.into());
            };
            let blobs = ctx.blobs.as_ref().ok_or(ErrorClass::BlobUnavailable)?;
            let blob = blobs
                .get(blob_key)
                .await
                .map_err(|_| ErrorClass::BlobUnavailable)?
                .ok_or(ErrorClass::ExportUnavailable)?;
            sftp::drop_file(
                &target,
                key,
                file_name,
                &blob.bytes,
                &export_build::hex(sha),
            )
            .await
        }
        IntegrationKind::FhirServer | IntegrationKind::Hl7Mllp => {
            Err(ErrorClass::UnsupportedKind.into())
        }
    }
}

/// DLV-2: an absolute download link valid 15 minutes: pre-signed (S3) or a proxy-mode
/// token under `ASD_PUBLIC_BASE_URL`. Proxy tokens of deliveries carry the nil user (no
/// console user asked; the delivery audit row names the destination).
async fn download_link(ctx: &JobContext, export: &ExportRow) -> Result<DownloadLink, ErrorClass> {
    let ttl = Duration::from_secs(WEBHOOK_DOWNLOAD_TTL_SECS);
    let config = ctx
        .config
        .as_ref()
        .ok_or(ErrorClass::DownloadUrlUnavailable)?;
    let (Some(key), Some(file_name)) = (&export.blob_key, &export.file_name) else {
        return Err(ErrorClass::ExportUnavailable);
    };
    match config.blob_mode {
        BlobMode::Presigned => {
            let blobs = ctx.blobs.as_ref().ok_or(ErrorClass::BlobUnavailable)?;
            let get = blobs
                .presign_get(key, ttl, file_name)
                .await
                .map_err(|e| match e {
                    BlobError::Unavailable(_) => ErrorClass::BlobUnavailable,
                    _ => ErrorClass::DownloadUrlUnavailable,
                })?;
            Ok(DownloadLink {
                url: get.url,
                expires_at: cursor::ts(get.expires_at),
            })
        }
        BlobMode::Proxy => {
            let base = config
                .public_base_url
                .as_deref()
                .ok_or(ErrorClass::DownloadUrlUnavailable)?;
            let expires_at =
                Utc::now() + chrono::Duration::from_std(ttl).expect("15 minutes fits in chrono");
            let token = export_build::download_token(
                &export.download_secret,
                export.id,
                Uuid::nil(),
                expires_at.timestamp(),
            );
            Ok(DownloadLink {
                url: format!("{base}/v1/exports/{}/file?t={token}", export.id),
                expires_at: cursor::ts(expires_at),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_follows_dlv_1() {
        let got: Vec<u64> = (1..=6).map(|a| backoff(a).as_secs()).collect();
        assert_eq!(got, [60, 300, 1_800, 7_200, 43_200, 43_200]);
        assert_eq!(DELIVERY_MAX_ATTEMPTS, 6);
    }
}
