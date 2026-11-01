//! owner: WP-B.12
//!
//! The `housekeeping_sweep` job (HK-1..5, ADR-0009 decision 6): one daily phased job that
//! deletes rows nothing reads any more, in batches of [`BATCH`] so no statement holds a
//! long lock:
//!
//! - `idempotency_keys` older than 7 days (every scope). A post after the sweep is still
//!   answered from the `sessions` row (HK-4): a replay of the same body is `200`, a
//!   different body (`sessions.request_hash`, 0021) `409`;
//! - `refresh_tokens` expired more than a day ago;
//! - `enrol_codes` revoked or expired more than 90 days ago **and** referenced by no
//!   device: rotation (CON-11) accepts expired codes on purpose, so a code a device
//!   enrolled with is its recovery path and stays (HK-5);
//! - `jobs` finished (succeeded, failed, cancelled) more than 30 days ago that no
//!   `norm_runs`, `exports`, `export_deliveries` or `erasure_requests` row references
//!   (those four foreign keys have no `ON DELETE`);
//! - pending TOTP enrolments (AUTH-6, migration 0020) past `totp_pending_expires_at`: the
//!   three pending columns are set to NULL (the user row stays). Confirm refuses them
//!   anyway; clearing them keeps sealed seeds from lingering under an old KEK.
//!
//! `dry_run` counts what would go and deletes nothing. Every manual run, and every
//! scheduled run that deleted something, writes one `housekeeping.sweep` (or
//! `housekeeping.sweep_dry_run`) audit row with org NULL and the counts. The worker
//! queues one scheduled run per day ([`schedule_daily`]).

use asd_contract::api_v1::HousekeepingResult;
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

use super::runner::JobError;
use super::{Job, JobContext, KIND_HOUSEKEEPING};
use crate::audit::AuditEvent;
use crate::error::AppResult;

/// Rows deleted per statement.
const BATCH: i64 = 5000;
/// Advisory-lock key serialising the daily scheduling across workers. Distinct from the
/// retention sweep's `0x0B09_5EED`, or the two daily jobs would block each other.
const SCHEDULE_LOCK: i64 = 0x0B09_5EEE;

/// Job params of a sweep. `scheduled` marks the daily run.
pub fn params(dry_run: bool, scheduled: bool) -> Value {
    json!({ "dry_run": dry_run, "scheduled": scheduled })
}

/// Rows removed (or, in a dry run, removable) per rule.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Counts {
    pub idempotency_keys: u64,
    pub refresh_tokens: u64,
    pub enrol_codes: u64,
    pub jobs: u64,
    pub totp_pending: u64,
}

impl Counts {
    fn total(&self) -> u64 {
        self.idempotency_keys
            + self.refresh_tokens
            + self.enrol_codes
            + self.jobs
            + self.totp_pending
    }

    fn result(self, dry_run: bool) -> HousekeepingResult {
        HousekeepingResult {
            idempotency_keys: self.idempotency_keys,
            refresh_tokens: self.refresh_tokens,
            enrol_codes: self.enrol_codes,
            jobs: self.jobs,
            totp_pending: self.totp_pending,
            dry_run,
        }
    }
}

fn count(n: i64) -> u64 {
    u64::try_from(n).unwrap_or(0)
}

/// Runs the sweep described by `job` (see the module docs).
pub async fn execute(ctx: &JobContext, job: &Job) -> Result<Value, JobError> {
    let dry_run = job
        .params
        .get("dry_run")
        .and_then(Value::as_bool)
        .ok_or_else(|| JobError::Permanent("invalid housekeeping params: dry_run".into()))?;
    let scheduled = job
        .params
        .get("scheduled")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let pool = &ctx.pool;
    let counts = if dry_run {
        count_eligible(pool).await?
    } else {
        sweep(pool).await?
    };
    let result = serde_json::to_value(counts.result(dry_run))
        .map_err(|e| JobError::Permanent(format!("housekeeping result: {e}")))?;
    if !scheduled || counts.total() > 0 {
        let action = if dry_run {
            "housekeeping.sweep_dry_run"
        } else {
            "housekeeping.sweep"
        };
        let mut tx = pool.begin().await?;
        AuditEvent::system(action, "job", job.id)
            .after({
                let mut after = result.clone();
                after["job_id"] = json!(job.id);
                after["scheduled"] = json!(scheduled);
                after
            })
            .record(&mut tx)
            .await?;
        tx.commit().await?;
    }
    Ok(result)
}

async fn count_eligible(pool: &PgPool) -> Result<Counts, sqlx::Error> {
    Ok(Counts {
        idempotency_keys: count(
            sqlx::query_scalar!(
                r#"SELECT count(*) AS "n!" FROM idempotency_keys
                   WHERE created_at < now() - interval '7 days'"#
            )
            .fetch_one(pool)
            .await?,
        ),
        refresh_tokens: count(
            sqlx::query_scalar!(
                r#"SELECT count(*) AS "n!" FROM refresh_tokens
                   WHERE expires_at < now() - interval '1 day'"#
            )
            .fetch_one(pool)
            .await?,
        ),
        enrol_codes: count(
            sqlx::query_scalar!(
                r#"SELECT count(*) AS "n!" FROM enrol_codes c
                   WHERE coalesce(c.revoked_at, c.expires_at) < now() - interval '90 days'
                     AND NOT EXISTS (SELECT 1 FROM devices d WHERE d.enrol_code_id = c.id)"#
            )
            .fetch_one(pool)
            .await?,
        ),
        jobs: count(
            sqlx::query_scalar!(
                r#"SELECT count(*) AS "n!" FROM jobs j
                   WHERE j.status IN ('succeeded', 'failed', 'cancelled')
                     AND j.finished_at < now() - interval '30 days'
                     AND NOT EXISTS (SELECT 1 FROM norm_runs r WHERE r.job_id = j.id)
                     AND NOT EXISTS (SELECT 1 FROM exports e WHERE e.job_id = j.id)
                     AND NOT EXISTS (SELECT 1 FROM export_deliveries x WHERE x.job_id = j.id)
                     AND NOT EXISTS (SELECT 1 FROM erasure_requests q WHERE q.job_id = j.id)"#
            )
            .fetch_one(pool)
            .await?,
        ),
        totp_pending: count(
            sqlx::query_scalar!(
                r#"SELECT count(*) AS "n!" FROM users WHERE totp_pending_expires_at < now()"#
            )
            .fetch_one(pool)
            .await?,
        ),
    })
}

/// Deletes each rule's rows in batches until a batch comes back short. The order matters
/// only in that every rule is independent: none deletes a row another rule's guard reads.
async fn sweep(pool: &PgPool) -> Result<Counts, sqlx::Error> {
    let mut counts = Counts::default();
    loop {
        let n = sqlx::query!(
            "DELETE FROM idempotency_keys WHERE ctid IN (
                 SELECT ctid FROM idempotency_keys
                 WHERE created_at < now() - interval '7 days' LIMIT $1)",
            BATCH,
        )
        .execute(pool)
        .await?
        .rows_affected();
        counts.idempotency_keys += n;
        if n < BATCH as u64 {
            break;
        }
    }
    loop {
        let n = sqlx::query!(
            "DELETE FROM refresh_tokens WHERE ctid IN (
                 SELECT ctid FROM refresh_tokens
                 WHERE expires_at < now() - interval '1 day' LIMIT $1)",
            BATCH,
        )
        .execute(pool)
        .await?
        .rows_affected();
        counts.refresh_tokens += n;
        if n < BATCH as u64 {
            break;
        }
    }
    loop {
        let n = sqlx::query!(
            "DELETE FROM enrol_codes WHERE ctid IN (
                 SELECT c.ctid FROM enrol_codes c
                 WHERE coalesce(c.revoked_at, c.expires_at) < now() - interval '90 days'
                   AND NOT EXISTS (SELECT 1 FROM devices d WHERE d.enrol_code_id = c.id)
                 LIMIT $1)",
            BATCH,
        )
        .execute(pool)
        .await?
        .rows_affected();
        counts.enrol_codes += n;
        if n < BATCH as u64 {
            break;
        }
    }
    loop {
        let n = sqlx::query!(
            "DELETE FROM jobs WHERE ctid IN (
                 SELECT j.ctid FROM jobs j
                 WHERE j.status IN ('succeeded', 'failed', 'cancelled')
                   AND j.finished_at < now() - interval '30 days'
                   AND NOT EXISTS (SELECT 1 FROM norm_runs r WHERE r.job_id = j.id)
                   AND NOT EXISTS (SELECT 1 FROM exports e WHERE e.job_id = j.id)
                   AND NOT EXISTS (SELECT 1 FROM export_deliveries x WHERE x.job_id = j.id)
                   AND NOT EXISTS (SELECT 1 FROM erasure_requests q WHERE q.job_id = j.id)
                 LIMIT $1)",
            BATCH,
        )
        .execute(pool)
        .await?
        .rows_affected();
        counts.jobs += n;
        if n < BATCH as u64 {
            break;
        }
    }
    loop {
        let n = sqlx::query!(
            "UPDATE users
             SET totp_pending_ct = NULL, totp_pending_kek_id = NULL, totp_pending_expires_at = NULL
             WHERE id IN (
                 SELECT id FROM users WHERE totp_pending_expires_at < now() LIMIT $1)
               AND totp_pending_expires_at < now()",
            BATCH,
        )
        .execute(pool)
        .await?
        .rows_affected();
        counts.totp_pending += n;
        if n < BATCH as u64 {
            break;
        }
    }
    Ok(counts)
}

/// Queues the daily sweep (not a dry run) unless one was queued in the last day.
/// Serialised across workers with an advisory lock. Returns the job id when queued.
pub async fn schedule_daily(pool: &PgPool) -> AppResult<Option<Uuid>> {
    let mut tx = pool.begin().await?;
    sqlx::query!("SELECT pg_advisory_xact_lock($1)", SCHEDULE_LOCK)
        .execute(&mut *tx)
        .await?;
    let recent = sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM jobs
                          WHERE kind = $1 AND org_id IS NULL
                            AND params ->> 'scheduled' = 'true'
                            AND created_at > now() - interval '1 day') AS "x!""#,
        KIND_HOUSEKEEPING,
    )
    .fetch_one(&mut *tx)
    .await?;
    if recent {
        tx.rollback().await?;
        return Ok(None);
    }
    let job = super::enqueue(&mut tx, KIND_HOUSEKEEPING, None, &params(false, true), None).await?;
    tx.commit().await?;
    Ok(Some(job.id))
}
