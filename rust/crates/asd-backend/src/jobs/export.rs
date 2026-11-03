//! owner: WP-I.6; WP-I.7 (queues the export's deliveries on success, fails them on failure);
//! WP-I.13 (per-format build step, shared `finish`, `case_package`)
//!
//! The `export` job (EXJ-3), a phased handler (G5, trap 3):
//!
//! 1. **Snapshot**: one `REPEATABLE READ READ ONLY` transaction reads every row the export
//!    needs ([`record::load`]); nothing is decrypted inside it.
//! 2. **Render and upload**, no transaction open: the consent policy
//!    (`asd_export::policy::select`, XC-5) picks the sessions, only theirs are decrypted,
//!    the `asd-export` encoder renders (de-identifying when the mode says so, XC-4), and
//!    the artifact goes to `exports/{org}/{export}/{attempt}/{file}` with
//!    `put_object_large`.
//! 3. **Record**: one transaction replaces the export's `export_items`, stores hash, size,
//!    counts and expiry, and appends the audit rows.
//!
//! Phase 1 and phase 2 up to the rendered artifact are the format's **build step**
//! (`build_sessions` for the session formats, `build_case` for `case_package`; WP-B.23
//! adds `audit_csv` as one more arm of `run`); the upload and phase 3 are one shared,
//! fenced `finish`.
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

use asd_contract::export::{
    ExportFormat, ExportMode, ExportStatus, MAX_ARTIFACT_BYTES, MAX_EXPORT_ROWS,
};
use asd_export::export_refusal;
use asd_export::model::{ExportRecord, Mrn, NoteText};
use asd_export::policy::{self, ExclusionReason};
use asd_export::watermark::Watermark;
use bytes::Bytes;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::runner::JobError;
use super::{Job, JobContext};
use crate::audit::AuditEvent;
use crate::blob::{BlobError, BlobStore};
use crate::crypto::Aad;
use crate::export_build::record::{self, Sealed, Snapshot};
use crate::export_build::{self, case, filters, Artifact, ExportRow, RenderError, RenderInput};
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
            // Fenced like every write of a run: a reclaimed run never fails the export
            // over the attempt that now owns the job.
            let marked = sqlx::query!(
                "UPDATE exports SET status = 'failed', error = $2, completed_at = now()
                 WHERE id = $1 AND status IN ('queued', 'running')
                   AND EXISTS (SELECT 1 FROM jobs
                               WHERE id = $3 AND attempts = $4 AND status = 'running')",
                export_id,
                message,
                job.id,
                job.attempts,
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
    let started = sqlx::query!(
        "UPDATE exports SET status = 'running', started_at = coalesce(started_at, now())
         WHERE id = $1 AND status IN ('queued', 'running')
           AND EXISTS (SELECT 1 FROM jobs
                       WHERE id = $2 AND attempts = $3 AND status = 'running')",
        export_id,
        job.id,
        job.attempts,
    )
    .execute(&ctx.pool)
    .await?;
    if started.rows_affected() == 0 {
        // Reclaimed before it started (or the export changed state): render nothing.
        return Err(permanent("job was reclaimed while running"));
    }

    // ---- the format's build step: phase 1 and phase 2 up to the rendered artifact
    let built = match row.format {
        ExportFormat::CasePackage => build_case(ctx, &row, &parsed).await?,
        ExportFormat::AuditCsv => build_audit(ctx, &row, &stored_filters, &parsed).await?,
        ExportFormat::Csv
        | ExportFormat::CsvExcel
        | ExportFormat::CsvSheets
        | ExportFormat::Xlsx
        | ExportFormat::PdfSummary
        | ExportFormat::FhirBundle
        | ExportFormat::FhirNdjson
        | ExportFormat::Hl7v2
        | ExportFormat::DatasetJsonl
        | ExportFormat::DatasetParquet => build_sessions(ctx, &row, &parsed).await?,
        // Refused at request time until WP-X.8 adds its build arm (`format_available`).
        ExportFormat::AnalysisResults => {
            return Err(permanent("analysis_results is not available yet (WP-X.8)"))
        }
    };

    // ---- upload and phase 3, shared by every format
    finish(ctx, job, &row, blobs.as_ref(), built).await
}

/// What a format's build step hands to [`finish`].
struct Built {
    artifact: Artifact,
    /// `(participant, session id)` of every included session (`export_items`, EXJ-6).
    items: Vec<(Uuid, String)>,
    /// `exports.excluded`: sessions left out, by reason.
    excluded: BTreeMap<String, u64>,
    /// Decrypted MRNs (MRN-5 audit).
    mrn_reads: u32,
    /// The artifact lifetime (EXJ-4).
    ttl: i32,
    /// `exports.row_count` when it is not the number of `items` (`audit_csv`: audit rows).
    row_count: Option<usize>,
}

/// The snapshot's sessions after the consent policy and the capture-source rule, with
/// what the requester could read decrypted.
struct Selected {
    included: Vec<(Uuid, ExportRecord)>,
    gazes: Vec<Option<Value>>,
    app_versions: Vec<Option<String>>,
    labels: Vec<Vec<asd_export::dataset::DatasetLabel>>,
    excluded: BTreeMap<String, u64>,
    mrn_reads: u32,
}

/// Formats that refuse simulated and synthetic captures (SIM-5): health-record formats,
/// case packages, and ML datasets (WP-B.9), so generated data never reaches a training set.
fn refuses_generated_captures(format: ExportFormat) -> bool {
    export_build::needs_gaze(format)
        || export_build::needs_mrn(format)
        || export_build::is_dataset(format)
        || format == ExportFormat::CasePackage
}

/// Phase 1 for every session-based format: one snapshot of the sessions `filters` select.
async fn snapshot(
    ctx: &JobContext,
    row: &ExportRow,
    filters: &filters::ParsedFilters,
    want_gaze: bool,
) -> Result<Snapshot, JobError> {
    let snap = record::load(&ctx.pool, row, filters, want_gaze).await?;
    if snap.sessions.len() > MAX_EXPORT_ROWS {
        return Err(permanent(format!(
            "export-too-large: more than {MAX_EXPORT_ROWS} sessions match; narrow the filters"
        )));
    }
    Ok(snap)
}

/// Phase 2 up to the render: consent policy (XC-5), capture source (SIM-5), decryption of
/// the kept sessions only.
async fn select(ctx: &JobContext, row: &ExportRow, snap: &Snapshot) -> Result<Selected, JobError> {
    let mode = export_build::policy_mode(row.mode);
    let purpose = export_build::policy_purpose(row.purpose);
    let records: Vec<ExportRecord> = snap.sessions.iter().map(|s| s.record.clone()).collect();
    let selection = policy::select(&records, purpose, mode)
        .map_err(|e| permanent(format!("consent-purpose-mismatch: {e}")))?;
    drop(records);
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
    let mut labels = Vec::new();
    let mut mrn_reads = 0u32;
    for &i in &selection.included {
        let s = &snap.sessions[i];
        if refuses_generated_captures(row.format) {
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
        labels.push(s.labels.clone());
        included.push((s.participant_id, rec));
    }
    Ok(Selected {
        included,
        gazes,
        app_versions,
        labels,
        excluded,
        mrn_reads,
    })
}

fn watermark(row: &ExportRow, snap: &Snapshot) -> Watermark {
    Watermark {
        export_id: row.id.to_string(),
        requested_by_display: snap.requested_by_display.clone(),
        requested_at_utc: cursor::ts(row.created_at),
        mode: export_build::policy_mode(row.mode),
        purpose: export_build::policy_purpose(row.purpose),
        org_name: snap.org.name.clone(),
    }
}

/// XC-4: a fresh salt per run, dropped when the build returns (never stored).
fn fresh_salt() -> Result<[u8; asd_export::deid::SALT_LEN], JobError> {
    let mut salt = [0u8; asd_export::deid::SALT_LEN];
    getrandom::fill(&mut salt).map_err(|e| JobError::Transient(format!("randomness: {e}")))?;
    Ok(salt)
}

fn ttl_of(row: &ExportRow, snap: &Snapshot) -> i32 {
    snap.ttl_hours.map_or(row.ttl_hours, |h| {
        export_build::ttl_hours(Some(i64::from(h)))
    })
}

fn items_of(included: &[(Uuid, ExportRecord)]) -> Vec<(Uuid, String)> {
    included
        .iter()
        .map(|(p, r)| (*p, r.session.summary.id.clone()))
        .collect()
}

/// The session formats (CSV, XLSX, FHIR, HL7, datasets): one artifact over the sessions the
/// filters select, through [`export_build::render`].
async fn build_sessions(
    ctx: &JobContext,
    row: &ExportRow,
    filters: &filters::ParsedFilters,
) -> Result<Built, JobError> {
    let snap = snapshot(ctx, row, filters, export_build::needs_gaze(row.format)).await?;
    let selected = select(ctx, row, &snap).await?;
    let requested_at = cursor::ts(row.created_at);
    let watermark = watermark(row, &snap);
    let hl7 = export_build::hl7_profile(
        snap.hl7.as_ref().map(|(c, _)| c),
        snap.hl7.as_ref().is_some_and(|(_, test)| *test),
        &snap.org.name,
    );
    let records: Vec<ExportRecord> = selected.included.iter().map(|(_, r)| r.clone()).collect();
    let artifact = export_build::render(&RenderInput {
        format: row.format,
        records: &records,
        gazes: &selected.gazes,
        app_versions: &selected.app_versions,
        watermark: &watermark,
        deid_salt: fresh_salt()?,
        generated_at: &requested_at,
        hl7: &hl7,
        labels: &selected.labels,
        include_labels: row.include_labels,
    })
    .map_err(|e: RenderError| permanent(e.to_string()))?;
    Ok(Built {
        artifact,
        items: items_of(&selected.included),
        excluded: selected.excluded,
        mrn_reads: selected.mrn_reads,
        ttl: ttl_of(row, &snap),
        row_count: None,
    })
}

/// `case_package` (SHR-1..7): one participant's sessions (the stored filters name the
/// participant and, optionally, the sessions), their notes when the requester could read
/// them, the recipient and the consent, through [`export_build::case`].
async fn build_case(
    ctx: &JobContext,
    row: &ExportRow,
    filters: &filters::ParsedFilters,
) -> Result<Built, JobError> {
    let participant = row
        .case_participant_id
        .ok_or_else(|| permanent("invalid stored case: no participant"))?;
    if filters.participant_ids != [participant] {
        return Err(permanent(
            "invalid stored case: the filters name another participant",
        ));
    }
    let sealed_recipient = row
        .case_recipient
        .as_ref()
        .ok_or_else(|| permanent("invalid stored case: no recipient"))?;
    let snap = snapshot(ctx, row, filters, true).await?;
    let selected = select(ctx, row, &snap).await?;

    let mut records: Vec<ExportRecord> = selected.included.iter().map(|(_, r)| r.clone()).collect();
    if row.mode == ExportMode::Identified && row.include_clinical_content {
        let session_ids: Vec<String> = records
            .iter()
            .map(|r| r.session.summary.id.clone())
            .collect();
        let sealed = case::load_notes(&ctx.pool, row.org_id, participant, &session_ids).await?;
        let mut notes = Vec::with_capacity(sealed.len());
        for n in sealed {
            notes.push((
                n.session_id,
                NoteText {
                    created_at: n.created_at,
                    author_display_name: n.author_display_name,
                    body: open(ctx, row.org_id, &n.body).await?,
                },
            ));
        }
        case::attach_notes(&mut records, notes);
    }
    let recipient = open(
        ctx,
        row.org_id,
        &Sealed {
            table: "exports",
            column: "case_recipient_ct",
            row_id: row.id,
            key_id: sealed_recipient.key_id,
            ciphertext: sealed_recipient.ciphertext.clone(),
        },
    )
    .await?;
    let sessions = records
        .into_iter()
        .zip(selected.gazes.iter().cloned())
        .zip(selected.app_versions.iter().cloned())
        .map(|((record, gaze), app_version)| case::IncludedSession {
            record,
            gaze,
            app_version,
        })
        .collect();
    let mut input = case::package_input(
        case::CaseFacts {
            watermark: watermark(row, &snap),
            created_at: cursor::ts(row.created_at),
            deid_salt: fresh_salt()?,
            recipient,
            consent: case::consent(row, &snap.requested_by_display),
        },
        sessions,
    )
    .map_err(|e| permanent(e.to_string()))?;
    case::complete(ctx, row, participant, &mut input).await?;
    let artifact = case::render(&input).map_err(|e| permanent(e.to_string()))?;
    Ok(Built {
        artifact,
        items: items_of(&selected.included),
        excluded: selected.excluded,
        mrn_reads: selected.mrn_reads,
        ttl: ttl_of(row, &snap),
        row_count: None,
    })
}

/// `audit_csv` (HIP-3): the org's audit rows through `asd_export::tabular::audit`. No
/// session is read, so no `export_items` rows are written; `row_count` is the audit rows.
/// The org's artifact lifetime applies like for every export.
async fn build_audit(
    ctx: &JobContext,
    row: &ExportRow,
    stored: &asd_contract::export::ExportFilters,
    parsed: &filters::ParsedFilters,
) -> Result<Built, JobError> {
    let rows = export_build::audit::load(&ctx.pool, row.org_id, stored, parsed)
        .await
        .map_err(|e| match e {
            crate::error::AppError::Internal(m) if m.starts_with("export-too-large") => {
                permanent(m)
            }
            other => JobError::from(other),
        })?;
    let bytes = asd_export::tabular::audit::encode(&rows).map_err(|e| permanent(e.to_string()))?;
    let settings: Value = sqlx::query_scalar!(
        "SELECT settings FROM organizations WHERE id = $1",
        row.org_id
    )
    .fetch_one(&ctx.pool)
    .await?;
    let ttl = export_build::ttl_hours(
        settings
            .get(asd_contract::export::settings::EXPORTS)
            .and_then(|e| e.get(asd_contract::export::settings::TTL_HOURS))
            .and_then(Value::as_i64),
    );
    Ok(Built {
        artifact: Artifact {
            bytes,
            file_name: asd_export::tabular::audit::file_name(&row.id.to_string()),
            content_type: "text/csv; charset=utf-8",
        },
        items: Vec::new(),
        excluded: BTreeMap::new(),
        mrn_reads: 0,
        ttl,
        row_count: Some(rows.len()),
    })
}

/// The shared end of every export: uploads the artifact under this attempt's key, then,
/// in one transaction fenced on the job attempt, replaces `export_items`, marks the export
/// `succeeded`, audits `export.complete` (and the MRN read, MRN-5) and queues the export's
/// deliveries (DLV-1).
async fn finish(
    ctx: &JobContext,
    job: &Job,
    row: &ExportRow,
    blobs: &dyn BlobStore,
    built: Built,
) -> Result<Value, JobError> {
    let export_id = row.id;
    let Built {
        artifact,
        items,
        excluded,
        mrn_reads,
        ttl,
        row_count,
    } = built;
    if artifact.bytes.len() > MAX_ARTIFACT_BYTES {
        return Err(permanent(format!(
            "export-too-large: {} bytes exceed {MAX_ARTIFACT_BYTES}",
            artifact.bytes.len()
        )));
    }
    let sha256 = Sha256::digest(&artifact.bytes).to_vec();
    let byte_size = i64::try_from(artifact.bytes.len()).expect("capped at 500 MB");
    let key = export_build::blob_key(row.org_id, export_id, job.attempts, &artifact.file_name);
    blobs
        .put_object_large(&key, artifact.content_type, Bytes::from(artifact.bytes))
        .await
        .map_err(|e| blob_error(e, job.attempts))?;

    let participants: Vec<Uuid> = items.iter().map(|(p, _)| *p).collect();
    let sessions: Vec<String> = items.into_iter().map(|(_, s)| s).collect();
    let row_count = i32::try_from(row_count.unwrap_or(sessions.len())).expect("capped at 200 000");
    let excluded_json = serde_json::to_value(&excluded).expect("string keys");
    let mut tx = ctx.pool.begin().await?;
    // Fence: this attempt must still own the job. The row lock holds off `reclaim_stale`
    // and a newer claim until this transaction ends.
    let owns_job = sqlx::query_scalar!(
        r#"SELECT 1 AS "one!" FROM jobs
           WHERE id = $1 AND attempts = $2 AND status = 'running'
           FOR UPDATE"#,
        job.id,
        job.attempts,
    )
    .fetch_optional(&mut *tx)
    .await?
    .is_some();
    if !owns_job {
        tx.rollback().await?;
        // Our artifact is nobody's: remove it (best effort; the key is ours alone).
        if let Err(e) = blobs.delete(&key).await {
            tracing::warn!(%export_id, error = %e, "could not delete a stale run's artifact");
        }
        return Err(permanent("job was reclaimed while running"));
    }
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
