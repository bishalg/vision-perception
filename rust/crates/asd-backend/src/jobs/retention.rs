//! owner: WP-B.9
//!
//! The `retention_sweep` job (02 §5: "retention sweep (nightly: delete frames blobs beyond
//! retention, keep aggregate counts)"; ADR-B8: retention per org, default 24 months).
//!
//! For each org (or the one in `jobs.org_id`), sessions **received** before
//! `now() - organizations.retention_months` lose their frame-level blob
//! (`frames.jsonl.enc`, under the recorded key and the ADR-B3 key): the blob is deleted,
//! `frames_blob_key` cleared and `frames_purged_at` set, so the next sweep skips them. The
//! session row, its summary, clips, reviews and clinical records stay: they are the
//! aggregate counts and the clinical record. `received_at` is the server's clock, so a
//! device clock cannot postpone the sweep.
//!
//! `dry_run` counts what would be purged and changes nothing. Every manual run, and every
//! scheduled run that purged something, writes one `retention.sweep` (or
//! `retention.sweep_dry_run`) audit row per org with the counts. The worker queues one
//! scheduled run per day ([`schedule_nightly`]).
//!
//! Re-runnable: blob deletes of absent keys succeed, and a session is marked only after its
//! blob delete succeeded.

use asd_contract::privacy::{RetentionOrgResult, RetentionSweepResult};
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

use super::erasure::{delete_blobs, session_blob_keys};
use super::runner::JobError;
use super::{Job, JobContext, KIND_RETENTION_SWEEP};
use crate::audit::AuditEvent;
use crate::error::AppResult;
use crate::read::cursor;

/// Sessions purged per batch (one blob pass and one UPDATE).
const BATCH: i64 = 500;
/// Advisory-lock key that serialises the nightly scheduling across workers.
const SCHEDULE_LOCK: i64 = 0x0B09_5EED;

/// Job params of a sweep. `scheduled` marks the nightly run.
pub fn params(dry_run: bool, scheduled: bool) -> Value {
    json!({ "dry_run": dry_run, "scheduled": scheduled })
}

fn permanent(m: impl Into<String>) -> JobError {
    JobError::Permanent(m.into())
}

/// Runs the sweep described by `job` (see the module docs).
pub async fn execute(ctx: &JobContext, job: &Job) -> Result<Value, JobError> {
    let dry_run = job
        .params
        .get("dry_run")
        .and_then(Value::as_bool)
        .ok_or_else(|| permanent("invalid retention params: dry_run"))?;
    let scheduled = job
        .params
        .get("scheduled")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let blobs = match (&ctx.blobs, dry_run) {
        (Some(b), _) => Some(b.clone()),
        (None, true) => None,
        (None, false) => return Err(permanent("this worker has no blob store")),
    };
    let pool = &ctx.pool;
    let orgs = sqlx::query!(
        r#"SELECT id, retention_months,
                  now() - make_interval(months => retention_months) AS "cutoff!"
           FROM organizations WHERE ($1::uuid IS NULL OR id = $1) ORDER BY id"#,
        job.org_id,
    )
    .fetch_all(pool)
    .await?;

    let mut results = Vec::with_capacity(orgs.len());
    for org in orgs {
        let purged = if dry_run {
            let n = sqlx::query_scalar!(
                r#"SELECT count(*) AS "n!" FROM sessions
                   WHERE org_id = $1 AND received_at < $2 AND frames_purged_at IS NULL"#,
                org.id,
                org.cutoff,
            )
            .fetch_one(pool)
            .await?;
            u64::try_from(n).unwrap_or(0)
        } else {
            let blobs = blobs.as_ref().expect("checked above");
            let mut purged = 0u64;
            loop {
                let batch = sqlx::query!(
                    r#"SELECT id, participant_token, frames_blob_key FROM sessions
                       WHERE org_id = $1 AND received_at < $2 AND frames_purged_at IS NULL
                       ORDER BY received_at, id LIMIT $3"#,
                    org.id,
                    org.cutoff,
                    BATCH,
                )
                .fetch_all(pool)
                .await?;
                if batch.is_empty() {
                    break;
                }
                for s in &batch {
                    let keys = session_blob_keys(
                        org.id,
                        &s.participant_token,
                        &s.id,
                        s.frames_blob_key.as_deref(),
                        None,
                        true,
                    );
                    delete_blobs(&**blobs, &keys, job.attempts).await?;
                }
                let ids: Vec<String> = batch.into_iter().map(|s| s.id).collect();
                purged += sqlx::query!(
                    "UPDATE sessions SET frames_blob_key = NULL, frames_purged_at = now()
                     WHERE id = ANY($1) AND frames_purged_at IS NULL",
                    &ids,
                )
                .execute(pool)
                .await?
                .rows_affected();
            }
            purged
        };
        let result = RetentionOrgResult {
            org_id: org.id.to_string(),
            retention_months: org.retention_months,
            cutoff: cursor::ts(org.cutoff),
            frames_purged: purged,
        };
        if !scheduled || purged > 0 {
            let action = if dry_run {
                "retention.sweep_dry_run"
            } else {
                "retention.sweep"
            };
            let mut tx = pool.begin().await?;
            AuditEvent::system(action, "organization", org.id)
                .org(org.id)
                .after(json!({
                    "job_id": job.id,
                    "dry_run": dry_run,
                    "scheduled": scheduled,
                    "retention_months": org.retention_months,
                    "cutoff": result.cutoff,
                    "frames_purged": purged,
                }))
                .record(&mut tx)
                .await?;
            tx.commit().await?;
        }
        results.push(result);
    }
    Ok(serde_json::to_value(RetentionSweepResult {
        dry_run,
        orgs: results,
    })
    .expect("plain data"))
}

/// Queues the nightly sweep (every org, not a dry run) unless one was queued in the last
/// day. Serialised across workers with an advisory lock. Returns the job id when queued.
pub async fn schedule_nightly(pool: &PgPool) -> AppResult<Option<Uuid>> {
    let mut tx = pool.begin().await?;
    sqlx::query!("SELECT pg_advisory_xact_lock($1)", SCHEDULE_LOCK)
        .execute(&mut *tx)
        .await?;
    let recent = sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM jobs
                          WHERE kind = $1 AND org_id IS NULL
                            AND params ->> 'scheduled' = 'true'
                            AND created_at > now() - interval '1 day') AS "x!""#,
        KIND_RETENTION_SWEEP,
    )
    .fetch_one(&mut *tx)
    .await?;
    if recent {
        tx.rollback().await?;
        return Ok(None);
    }
    let job = super::enqueue(
        &mut tx,
        KIND_RETENTION_SWEEP,
        None,
        &params(false, true),
        None,
    )
    .await?;
    tx.commit().await?;
    Ok(Some(job.id))
}
