//! owner: WP-B.5
//!
//! The job runner: claim, execute, record. `asd-backend worker` runs [`worker_loop`]; tests
//! call [`run_pending`] to drain the queue deterministically.
//!
//! ## Claiming
//!
//! [`claim_next`] is one statement: a CTE selects the oldest `queued` job
//! `FOR UPDATE SKIP LOCKED` and the outer `UPDATE` flips it to `running`. Two workers (or
//! 50 tasks) polling at once never get the same job, and never wait for each other.
//!
//! ## Execution and failure
//!
//! A job runs in one `REPEATABLE READ` transaction together with its success bookkeeping,
//! so a handler's writes (the `norm_runs` row, its audit row) and `status = 'succeeded'`
//! commit atomically. A handler error rolls everything back, then:
//!
//! - [`JobError::Permanent`] (bad params, unknown kind): `failed`, no retry;
//! - [`JobError::Transient`] (database trouble): back to `queued` until [`MAX_ATTEMPTS`].
//!
//! A worker that dies mid-job leaves a `running` row; [`reclaim_stale`] returns it to the
//! queue (or fails it at the attempt limit) once it has been running longer than the
//! visibility timeout. Handlers are therefore re-runnable.

use std::future::Future;
use std::time::Duration;

use sqlx::PgPool;
use uuid::Uuid;

use super::{norm_aggregate, Job, KIND_NORM_AGGREGATE};
use crate::error::AppError;

/// A job is attempted at most this many times.
pub const MAX_ATTEMPTS: i32 = 3;
/// A `running` job older than this is presumed orphaned by a dead worker.
pub const DEFAULT_VISIBILITY_TIMEOUT: Duration = Duration::from_secs(15 * 60);
/// Idle sleep between polls of an empty queue.
pub const DEFAULT_POLL_INTERVAL: Duration = Duration::from_secs(2);
const RECLAIM_EVERY: Duration = Duration::from_secs(60);
const MAX_ERROR_LEN: usize = 500;

/// Why a handler failed.
#[derive(Debug)]
pub enum JobError {
    /// Retrying cannot help.
    Permanent(String),
    /// Worth retrying (database or infrastructure trouble).
    Transient(String),
}

impl From<sqlx::Error> for JobError {
    fn from(e: sqlx::Error) -> Self {
        Self::Transient(e.to_string())
    }
}

impl From<AppError> for JobError {
    fn from(e: AppError) -> Self {
        match e {
            AppError::Database(_) | AppError::Unavailable => Self::Transient(e.to_string()),
            other => Self::Permanent(other.to_string()),
        }
    }
}

/// What happened to one claimed job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Succeeded,
    Retrying,
    Failed,
}

/// Claims the oldest queued job, or `None` when the queue is empty.
pub async fn claim_next(pool: &PgPool, worker: &str) -> Result<Option<Job>, sqlx::Error> {
    let r = sqlx::query!(
        r#"WITH next AS (
               SELECT id FROM jobs
               WHERE status = 'queued'
               ORDER BY created_at, id
               FOR UPDATE SKIP LOCKED
               LIMIT 1)
           UPDATE jobs j
           SET status = 'running', started_at = now(), finished_at = NULL,
               attempts = j.attempts + 1, locked_by = $1, error = NULL
           FROM next
           WHERE j.id = next.id
           RETURNING j.id, j.kind, j.status, j.params, j.result, j.attempts, j.org_id,
                     j.created_by, j.created_at, j.started_at, j.finished_at, j.error"#,
        worker,
    )
    .fetch_optional(pool)
    .await?;
    Ok(r.map(|r| Job {
        id: r.id,
        kind: r.kind,
        status: r.status,
        params: r.params,
        result: r.result,
        attempts: r.attempts,
        org_id: r.org_id,
        created_by: r.created_by,
        created_at: r.created_at,
        started_at: r.started_at,
        finished_at: r.finished_at,
        error: r.error,
    }))
}

/// Runs the handler for `job.kind` and records the outcome.
pub async fn execute(pool: &PgPool, job: &Job) -> Result<Outcome, sqlx::Error> {
    match run_handler(pool, job).await {
        Ok(()) => Ok(Outcome::Succeeded),
        Err(error) => record_failure(pool, job, error).await,
    }
}

async fn run_handler(pool: &PgPool, job: &Job) -> Result<(), JobError> {
    let mut tx = pool.begin().await?;
    // Must be the first statement: every page of the aggregation reads one snapshot.
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ")
        .execute(&mut *tx)
        .await?;
    let result = match job.kind.as_str() {
        KIND_NORM_AGGREGATE => norm_aggregate::execute(&mut tx, job).await?,
        other => return Err(JobError::Permanent(format!("unknown job kind {other:?}"))),
    };
    let done = sqlx::query!(
        "UPDATE jobs SET status = 'succeeded', result = $2, finished_at = now(),
                         locked_by = NULL, error = NULL
         WHERE id = $1 AND status = 'running'",
        job.id,
        result,
    )
    .execute(&mut *tx)
    .await?;
    if done.rows_affected() != 1 {
        // Reclaimed by another worker while this one ran: discard our work.
        return Err(JobError::Permanent(
            "job was reclaimed while running".into(),
        ));
    }
    tx.commit().await?;
    Ok(())
}

async fn record_failure(pool: &PgPool, job: &Job, error: JobError) -> Result<Outcome, sqlx::Error> {
    let (retry, message) = match error {
        JobError::Permanent(m) => (false, m),
        JobError::Transient(m) => (job.attempts < MAX_ATTEMPTS, m),
    };
    let message: String = message.chars().take(MAX_ERROR_LEN).collect();
    tracing::warn!(job_id = %job.id, kind = %job.kind, attempts = job.attempts, retry, error = %message, "job failed");
    let status = if retry { "queued" } else { "failed" };
    sqlx::query!(
        "UPDATE jobs
         SET status = $2, error = $3, locked_by = NULL,
             finished_at = CASE WHEN $2 = 'failed' THEN now() ELSE NULL END
         WHERE id = $1 AND status = 'running'",
        job.id,
        status,
        message,
    )
    .execute(pool)
    .await?;
    Ok(if retry {
        Outcome::Retrying
    } else {
        Outcome::Failed
    })
}

/// Claims and runs one job; `None` when the queue was empty.
pub async fn run_one(pool: &PgPool, worker: &str) -> Result<Option<(Uuid, Outcome)>, sqlx::Error> {
    let Some(job) = claim_next(pool, worker).await? else {
        return Ok(None);
    };
    let outcome = execute(pool, &job).await?;
    Ok(Some((job.id, outcome)))
}

/// Runs jobs until the queue is empty; returns how many ran (retries count again).
pub async fn run_pending(pool: &PgPool, worker: &str) -> Result<usize, sqlx::Error> {
    let mut ran = 0;
    while run_one(pool, worker).await?.is_some() {
        ran += 1;
    }
    Ok(ran)
}

/// Returns jobs stuck `running` longer than `timeout` to the queue, or fails them at the
/// attempt limit. Returns how many rows changed.
pub async fn reclaim_stale(pool: &PgPool, timeout: Duration) -> Result<u64, sqlx::Error> {
    let seconds = timeout.as_secs_f64();
    let r = sqlx::query!(
        "UPDATE jobs
         SET status = CASE WHEN attempts >= $2 THEN 'failed' ELSE 'queued' END,
             finished_at = CASE WHEN attempts >= $2 THEN now() ELSE NULL END,
             error = 'worker stopped before the job finished', locked_by = NULL
         WHERE status = 'running'
           AND started_at < now() - make_interval(secs => $1)",
        seconds,
        MAX_ATTEMPTS,
    )
    .execute(pool)
    .await?;
    Ok(r.rows_affected())
}

/// The `asd-backend worker` loop: reclaim orphans every minute, drain the queue, sleep
/// when idle, stop when `shutdown` resolves (between jobs; a running job finishes).
pub async fn worker_loop(
    pool: PgPool,
    worker: String,
    poll: Duration,
    visibility_timeout: Duration,
    shutdown: impl Future<Output = ()>,
) {
    tokio::pin!(shutdown);
    let mut last_reclaim: Option<tokio::time::Instant> = None;
    loop {
        if last_reclaim.is_none_or(|t| t.elapsed() >= RECLAIM_EVERY) {
            match reclaim_stale(&pool, visibility_timeout).await {
                Ok(0) => {}
                Ok(n) => tracing::warn!(reclaimed = n, "returned orphaned jobs to the queue"),
                Err(e) => tracing::warn!(error = %e, "reclaiming stale jobs failed"),
            }
            last_reclaim = Some(tokio::time::Instant::now());
        }
        match run_one(&pool, &worker).await {
            Ok(Some((id, outcome))) => {
                tracing::info!(job_id = %id, ?outcome, "job finished");
                // More may be queued: poll again at once, unless asked to stop.
                if tokio::time::timeout(Duration::ZERO, &mut shutdown)
                    .await
                    .is_ok()
                {
                    break;
                }
                continue;
            }
            Ok(None) => {}
            Err(e) => tracing::warn!(error = %e, "job poll failed"),
        }
        tokio::select! {
            () = &mut shutdown => break,
            () = tokio::time::sleep(poll) => {}
        }
    }
    tracing::info!("job worker stopped");
}
