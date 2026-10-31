//! owner: WP-I.6; WP-I.7 (queues the export's deliveries on success, fails them on failure)
//!
//! The `export` job (EXJ-3), a phased handler (G5, trap 3):
//!
//! 1. **Snapshot**: one `REPEATABLE READ READ ONLY` transaction reads every row the export
//!    needs ([`record::load`]); nothing is decrypted inside it.
//! 2. **Render and upload**, no transaction open: the consent policy
//!    (`asd_export::policy::select`, XC-5) picks the sessions, only theirs are decrypted,
//!    the `asd-export` encoder renders (de-identifying when the mode says so, XC-4), and
//!    the artifact goes to `exports/{org}/{export}/{file}` with `put_object_large`.
//! 3. **Record**: one transaction replaces the export's `export_items`, stores hash, size,
//!    counts and expiry, and appends the audit rows.
//!
//! Every phase is idempotent: the blob key is deterministic (a re-run overwrites), items
//! are replaced, and a job whose export already succeeded returns the stored result
//! without rendering again (trap 4: a reclaimed job may run twice). The runner heartbeats
//! the job while this runs.
//!
//! Failures that will not be retried mark the export `failed` with a short error class
//! (`export-too-large: ...`, `unsupported-format: ...`); messages never carry patient data.

use std::collections::BTreeMap;
use std::time::Duration;

use asd_contract::export::{ExportStatus, MAX_ARTIFACT_BYTES, MAX_EXPORT_ROWS};
use asd_export::export_refusal;
use asd_export::model::{ExportRecord, Mrn};
use asd_export::policy::{self, ExclusionReason};
use asd_export::watermark::Watermark;
use bytes::Bytes;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::runner::JobError;
use super::{Job, JobContext};
use crate::audit::AuditEvent;
use crate::blob::BlobError;
use crate::crypto::Aad;
use crate::export_build::record::{self, Sealed};
use crate::export_build::{self, filters, ExportRow, RenderError, RenderInput};
use crate::read::cursor;

/// `exports.excluded` key for sessions the format refuses (simulated or synthetic capture,
/// SIM-5), next to the consent reasons of `asd_export::policy::ExclusionReason`.
pub const EXCLUDED_CAPTURE_SOURCE: &str = "capture_source_refused";

/// Job params of an export.
pub fn params(export_id: Uuid) -> Value {
    json!({ "export_id": export_id })
}

fn permanent(m: impl Into<String>) -> JobError {
    JobError::Permanent(m.into())
}

/// Runs the export named in `job.params` (see the module docs).
pub async fn execute(ctx: &JobContext, job: &Job) -> Result<Value, JobError> {
    let export_id = job
        .params
        .get("export_id")
        .and_then(Value::as_str)
        .and_then(|s| Uuid::parse_str(s).ok())
        .ok_or_else(|| permanent("invalid export params: export_id"))?;
    let result = run(ctx, job, export_id).await;
    if let Err(e) = &result {
        if e.is_final(job.attempts) {
            let message: String = e.message().chars().take(500).collect();
            let marked = sqlx::query!(
                "UPDATE exports SET status = 'failed', error = $2, completed_at = now()
                 WHERE id = $1 AND status IN ('queued', 'running')",
                export_id,
                message,
            )
            .execute(&ctx.pool)
            .await;
            if let Err(db) = marked {
                tracing::warn!(%export_id, error = %db, "could not mark the export failed");
            }
            // DLV-1: deliveries of an export that will never exist fail with it.
            if let Err(db) = crate::interop::outbox::fail_for_export(
                &ctx.pool,
                export_id,
                crate::interop::ErrorClass::ExportUnavailable,
            )
            .await
            {
                tracing::warn!(%export_id, error = %db, "could not fail the export's deliveries");
            }
        }
    }
    result
}

fn result_of(row: &ExportRow) -> Value {
    json!({
        "export_id": row.id,
        "row_count": row.row_count,
        "byte_size": row.byte_size,
        "sha256": row.sha256.as_deref().map(export_build::hex),
    })
}

fn blob_error(e: BlobError, attempts: i32) -> JobError {
    match e {
        BlobError::Unavailable(m) => JobError::RetryAfter(
            format!("blob-store-unavailable: {m}"),
            Duration::from_secs(60 * u64::from(attempts.unsigned_abs().max(1))),
        ),
        BlobError::TooLarge { size, limit } => {
            permanent(format!("export-too-large: {size} bytes exceed {limit}"))
        }
        BlobError::InvalidKey => permanent("blob-store: invalid artifact key"),
        BlobError::Other(m) => JobError::Transient(format!("blob-store: {m}")),
    }
}

async fn open(ctx: &JobContext, org: Uuid, sealed: &Sealed) -> Result<String, JobError> {
    let keyring = ctx
        .keyring
        .as_ref()
        .ok_or_else(|| permanent("clinical-crypto-unavailable: this worker has no ASD_DATA_KEK"))?;
    let aad = Aad::new(sealed.table, sealed.column, org, sealed.row_id);
    keyring
        .decrypt_string(org, &aad, sealed.key_id, &sealed.ciphertext)
        .await
        .map_err(|e| match e {
            crate::crypto::CryptoError::Database(db) => JobError::Transient(db.to_string()),
            other => permanent(format!(
                "clinical-crypto: cannot open {}.{}: {other}",
                sealed.table, sealed.column
            )),
        })
}

async fn run(ctx: &JobContext, job: &Job, export_id: Uuid) -> Result<Value, JobError> {
    let blobs = ctx
        .blobs
        .clone()
        .ok_or_else(|| permanent("this worker has no blob store"))?;
    let row = export_build::load_export(&ctx.pool, export_id)
        .await?
        .ok_or_else(|| permanent("export not found"))?;
    match row.status {
        ExportStatus::Succeeded => return Ok(result_of(&row)),
        ExportStatus::Failed | ExportStatus::Expired => {
            return Err(permanent(format!("export is {}", row.status.as_str())))
        }
        ExportStatus::Queued | ExportStatus::Running => {}
    }
    export_build::format_available(row.format)
        .map_err(|r| permanent(format!("unsupported-format: {r}")))?;
    let stored_filters = serde_json::from_value(row.filters.clone())
        .map_err(|_| permanent("invalid stored filters"))?;
    let parsed =
        filters::parse(&stored_filters).map_err(|_| permanent("invalid stored filters"))?;
    sqlx::query!(
        "UPDATE exports SET status = 'running', started_at = coalesce(started_at, now())
         WHERE id = $1 AND status IN ('queued', 'running')",
        export_id
    )
    .execute(&ctx.pool)
    .await?;

    // ---- phase 1: one snapshot
    let snap = record::load(
        &ctx.pool,
        &row,
        &parsed,
        export_build::needs_gaze(row.format),
    )
    .await?;
    if snap.sessions.len() > MAX_EXPORT_ROWS {
        return Err(permanent(format!(
            "export-too-large: more than {MAX_EXPORT_ROWS} sessions match; narrow the filters"
        )));
    }

    // ---- phase 2: consent policy, decrypt, render, upload (no transaction open)
    let mode = export_build::policy_mode(row.mode);
    let purpose = export_build::policy_purpose(row.purpose);
    let records: Vec<ExportRecord> = snap.sessions.iter().map(|s| s.record.clone()).collect();
    let selection = policy::select(&records, purpose, mode)
        .map_err(|e| permanent(format!("consent-purpose-mismatch: {e}")))?;
    let mut excluded: BTreeMap<String, u64> = selection
        .excluded
        .iter()
        .map(|(reason, n): (&ExclusionReason, &usize)| {
            (
                reason.as_str().to_owned(),
                u64::try_from(*n).unwrap_or(u64::MAX),
            )
        })
        .collect();

    let mut included = Vec::with_capacity(selection.included.len());
    let mut gazes = Vec::new();
    let mut app_versions = Vec::new();
    let mut mrn_reads = 0u32;
    for &i in &selection.included {
        let s = &snap.sessions[i];
        // SIM-5: health-record formats refuse simulated and synthetic captures.
        if export_build::needs_gaze(row.format) || export_build::needs_mrn(row.format) {
            let refused = s
                .gaze
                .as_ref()
                .and_then(|g| serde_json::from_value(g.clone()).ok())
                .and_then(|g| export_refusal(&g))
                .is_some_and(asd_export::Refusal::is_capture_source)
                || s.record.session.summary.capture_source
                    != Some(asd_core::types::CaptureSource::TrueDepth);
            if refused {
                *excluded
                    .entry(EXCLUDED_CAPTURE_SOURCE.to_owned())
                    .or_default() += 1;
                continue;
            }
        }
        let mut rec = s.record.clone();
        if let Some(review) = rec.review.as_mut() {
            if let Some(sealed) = &s.impression {
                review.impression = Some(open(ctx, row.org_id, sealed).await?);
            }
            if let Some(sealed) = &s.recommendations {
                review.recommendations = Some(open(ctx, row.org_id, sealed).await?);
            }
        }
        if let Some((sealed, authority)) = &s.mrn {
            rec.patient.mrn = Some(Mrn {
                value: open(ctx, row.org_id, sealed).await?,
                assigning_authority: authority.clone(),
            });
            mrn_reads += 1;
        }
        gazes.push(s.gaze.clone());
        app_versions.push(s.app_version.clone());
        included.push((s.participant_id, rec));
    }

    let requested_at = cursor::ts(row.created_at);
    let watermark = Watermark {
        export_id: export_id.to_string(),
        requested_by_display: snap.requested_by_display.clone(),
        requested_at_utc: requested_at.clone(),
        mode,
        purpose,
        org_name: snap.org.name.clone(),
    };
    let hl7 = export_build::hl7_profile(
        snap.hl7.as_ref().map(|(c, _)| c),
        snap.hl7.as_ref().is_some_and(|(_, test)| *test),
        &snap.org.name,
    );
    // XC-4: a fresh salt per run, dropped when this function returns (never stored).
    let mut salt = [0u8; asd_export::deid::SALT_LEN];
    getrandom::fill(&mut salt).map_err(|e| JobError::Transient(format!("randomness: {e}")))?;
    let records: Vec<ExportRecord> = included.iter().map(|(_, r)| r.clone()).collect();
    let artifact = export_build::render(&RenderInput {
        format: row.format,
        records: &records,
        gazes: &gazes,
        app_versions: &app_versions,
        watermark: &watermark,
        deid_salt: salt,
        generated_at: &requested_at,
        hl7: &hl7,
    })
    .map_err(|e: RenderError| permanent(e.to_string()))?;
    drop(records);
    if artifact.bytes.len() > MAX_ARTIFACT_BYTES {
        return Err(permanent(format!(
            "export-too-large: {} bytes exceed {MAX_ARTIFACT_BYTES}",
            artifact.bytes.len()
        )));
    }
    let sha256 = Sha256::digest(&artifact.bytes).to_vec();
    let byte_size = i64::try_from(artifact.bytes.len()).expect("capped at 500 MB");
    let key = export_build::blob_key(row.org_id, export_id, &artifact.file_name);
    blobs
        .put_object_large(&key, artifact.content_type, Bytes::from(artifact.bytes))
        .await
        .map_err(|e| blob_error(e, job.attempts))?;

    // ---- phase 3: record
    let participants: Vec<Uuid> = included.iter().map(|(p, _)| *p).collect();
    let sessions: Vec<String> = included
        .iter()
        .map(|(_, r)| r.session.summary.id.clone())
        .collect();
    let row_count = i32::try_from(sessions.len()).expect("capped at 200 000");
    let ttl = snap.ttl_hours.map_or(row.ttl_hours, |h| {
        export_build::ttl_hours(Some(i64::from(h)))
    });
    let excluded_json = serde_json::to_value(&excluded).expect("string keys");
    let mut tx = ctx.pool.begin().await?;
    sqlx::query!("DELETE FROM export_items WHERE export_id = $1", export_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query!(
        "INSERT INTO export_items (export_id, participant_id, session_id)
         SELECT $1, p, s FROM unnest($2::uuid[], $3::text[]) AS t (p, s)",
        export_id,
        &participants,
        &sessions,
    )
    .execute(&mut *tx)
    .await?;
    let done = sqlx::query!(
        "UPDATE exports
         SET status = 'succeeded', blob_key = $2, file_name = $3, content_type = $4,
             sha256 = $5, byte_size = $6, row_count = $7, excluded = $8, error = NULL,
             ttl_hours = $9, completed_at = now(),
             expires_at = now() + make_interval(hours => $9)
         WHERE id = $1 AND status IN ('queued', 'running')",
        export_id,
        key,
        artifact.file_name,
        artifact.content_type,
        sha256,
        byte_size,
        row_count,
        excluded_json,
        ttl,
    )
    .execute(&mut *tx)
    .await?;
    if done.rows_affected() != 1 {
        // Another run recorded it first (reclaim): ours is the same export; keep theirs.
        tx.rollback().await?;
        let row = export_build::load_export(&ctx.pool, export_id)
            .await?
            .ok_or_else(|| permanent("export not found"))?;
        return match row.status {
            ExportStatus::Succeeded => Ok(result_of(&row)),
            s => Err(permanent(format!("export is {}", s.as_str()))),
        };
    }
    AuditEvent::system("export.complete", "export", export_id)
        .org(row.org_id)
        .after(json!({
            "format": row.format.as_str(),
            "mode": row.mode.as_str(),
            "purpose": row.purpose.as_str(),
            "row_count": row_count,
            "excluded": excluded_json,
            "byte_size": byte_size,
            "sha256": export_build::hex(&sha256),
        }))
        .record(&mut tx)
        .await?;
    if mrn_reads > 0 {
        // MRN-5: decrypted MRNs are audited (one row per export, no MRN in it).
        AuditEvent::system("patient_ref.export_read", "patient_ref", export_id)
            .org(row.org_id)
            .after(json!({"set": true, "count": mrn_reads, "export_id": export_id}))
            .record(&mut tx)
            .await?;
    }
    // DLV-1: the artifact exists from this commit on; queue its deliveries with it.
    crate::interop::outbox::enqueue_ready(&mut tx, export_id).await?;
    tx.commit().await?;
    Ok(json!({
        "export_id": export_id,
        "row_count": row_count,
        "byte_size": byte_size,
        "sha256": export_build::hex(&sha256),
    }))
}

/// EXJ-4: deletes the artifacts of succeeded exports past `expires_at` and marks them
/// `expired`; `export_items` stay (accounting of disclosures). Re-runnable: the row is
/// updated only after the blob delete succeeded. Returns how many expired.
pub async fn sweep_expired(ctx: &JobContext) -> Result<u64, JobError> {
    let blobs = ctx
        .blobs
        .clone()
        .ok_or_else(|| permanent("this worker has no blob store"))?;
    let due = sqlx::query!(
        r#"SELECT id, org_id, blob_key FROM exports
           WHERE status = 'succeeded' AND expires_at <= now()
           ORDER BY expires_at LIMIT 500"#
    )
    .fetch_all(&ctx.pool)
    .await?;
    let mut expired = 0;
    for e in due {
        if let Some(key) = &e.blob_key {
            blobs.delete(key).await.map_err(|err| blob_error(err, 1))?;
        }
        let mut tx = ctx.pool.begin().await?;
        let changed = sqlx::query!(
            "UPDATE exports SET status = 'expired', expired_at = now()
             WHERE id = $1 AND status = 'succeeded'",
            e.id
        )
        .execute(&mut *tx)
        .await?;
        if changed.rows_affected() == 1 {
            AuditEvent::system("export.expire", "export", e.id)
                .org(e.org_id)
                .record(&mut tx)
                .await?;
            expired += 1;
        }
        tx.commit().await?;
    }
    Ok(expired)
}
