//! owner: WP-B.5 (claim, execute, record); WP-I.6 (`run_after`, heartbeat, phased handlers)
//!
//! The job runner: claim, execute, record. `asd-backend worker` runs [`worker_loop_in`];
//! tests call [`run_pending`] / [`run_pending_in`] to drain the queue deterministically.
//!
//! ## Claiming
//!
//! [`claim_next`] is one statement: a CTE selects the oldest `queued` job whose
//! `run_after` has passed, `FOR UPDATE SKIP LOCKED`, and the outer `UPDATE` flips it to
//! `running`. Two workers (or 50 tasks) polling at once never get the same job, and never
//! wait for each other. Each claim increments `attempts`, which then fences every later
//! write of that run: a worker whose job was reclaimed and claimed again cannot record
//! success or failure over the newer run.
//!
//! ## Execution and failure
//!
//! Two handler shapes (G5, trap 3):
//!
//! - **In-transaction** (`norm_aggregate`): the handler runs in one `REPEATABLE READ`
//!   transaction together with its success bookkeeping, so its writes and
//!   `status = 'succeeded'` commit atomically.
//! - **Phased** (`export`): the handler gets the [`JobContext`] and opens short
//!   transactions itself (snapshot, then render and upload with no transaction open, then
//!   record), each idempotent. While it runs, a [`Heartbeat`] refreshes
//!   `jobs.heartbeat_at`, and [`reclaim_stale`] measures staleness from the latest of
//!   `started_at` and `heartbeat_at`, so a long export is not handed to a second worker
//!   (EXJ-9, trap 4).
//!
//! A handler error rolls back what was open, then:
//!
//! - [`JobError::Permanent`] (bad params, unknown kind): `failed`, no retry;
//! - [`JobError::Transient`] (database trouble): back to `queued` until [`MAX_ATTEMPTS`];
//! - [`JobError::RetryAfter`]: as `Transient`, but not claimed again before the delay
//!   (`jobs.run_after`; blob store outages, DLV-1 backoff).
//!
//! A worker that dies mid-job leaves a `running` row; [`reclaim_stale`] returns it to the
//! queue (or fails it at the attempt limit) once it has been silent longer than the
//! visibility timeout. Handlers are therefore re-runnable.

use std::future::Future;
use std::time::Duration;

use serde_json::Value;
use sqlx::PgPool;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use uuid::Uuid;

use super::{export, norm_aggregate, Job, JobContext, KIND_EXPORT, KIND_NORM_AGGREGATE};
use crate::error::AppError;

/// A job is attempted at most this many times.
pub const MAX_ATTEMPTS: i32 = 3;
/// A `running` job silent for longer than this is presumed orphaned by a dead worker.
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
    /// Worth retrying at once (database or infrastructure trouble).
    Transient(String),
    /// Worth retrying, but not before the delay.
    RetryAfter(String, Duration),
}

impl JobError {
    pub fn message(&self) -> &str {
        match self {
            Self::Permanent(m) | Self::Transient(m) | Self::RetryAfter(m, _) => m,
        }
    }

    /// Whether the runner gives up after this error on attempt `attempts`.
    pub fn is_final(&self, attempts: i32) -> bool {
        match self {
            Self::Permanent(_) => true,
            Self::Transient(_) | Self::RetryAfter(..) => attempts >= MAX_ATTEMPTS,
        }
    }
}

impl std::fmt::Display for JobError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message())
    }
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

/// Claims the oldest queued job that is due, or `None` when none is.
pub async fn claim_next(pool: &PgPool, worker: &str) -> Result<Option<Job>, sqlx::Error> {
    let r = sqlx::query!(
        r#"WITH next AS (
               SELECT id FROM jobs
               WHERE status = 'queued' AND run_after <= now()
               ORDER BY created_at, id
               FOR UPDATE SKIP LOCKED
               LIMIT 1)
           UPDATE jobs j
           SET status = 'running', started_at = now(), finished_at = NULL,
               heartbeat_at = NULL, attempts = j.attempts + 1, locked_by = $1, error = NULL
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

/// Runs the handler for `job.kind` with a database-only context and records the outcome.
pub async fn execute(pool: &PgPool, job: &Job) -> Result<Outcome, sqlx::Error> {
    execute_in(&JobContext::db_only(pool.clone()), job).await
}

/// Runs the handler for `job.kind` and records the outcome.
pub async fn execute_in(ctx: &JobContext, job: &Job) -> Result<Outcome, sqlx::Error> {
    let result = match job.kind.as_str() {
        KIND_NORM_AGGREGATE => run_in_transaction(&ctx.pool, job).await,
        KIND_EXPORT => run_phased(ctx, job).await,
        other => Err(JobError::Permanent(format!("unknown job kind {other:?}"))),
    };
    match result {
        Ok(()) => Ok(Outcome::Succeeded),
        Err(error) => record_failure(&ctx.pool, job, error).await,
    }
}

async fn run_in_transaction(pool: &PgPool, job: &Job) -> Result<(), JobError> {
    let mut tx = pool.begin().await?;
    // Must be the first statement: every page of the aggregation reads one snapshot.
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ")
        .execute(&mut *tx)
        .await?;
    let result = norm_aggregate::execute(&mut tx, job).await?;
    let done = sqlx::query!(
        "UPDATE jobs SET status = 'succeeded', result = $2, finished_at = now(),
                         locked_by = NULL, error = NULL
         WHERE id = $1 AND status = 'running' AND attempts = $3",
        job.id,
        result,
        job.attempts,
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

async fn run_phased(ctx: &JobContext, job: &Job) -> Result<(), JobError> {
    let heartbeat = Heartbeat::start(ctx.pool.clone(), job.id, job.attempts, ctx.heartbeat_every);
    let result = export::execute(ctx, job).await;
    heartbeat.stop().await;
    mark_succeeded(&ctx.pool, job, &result?).await
}

/// `succeeded` for a phased job, fenced by `attempts`. The handler's own writes already
/// committed (idempotently), so losing the fence only means another run records it.
async fn mark_succeeded(pool: &PgPool, job: &Job, result: &Value) -> Result<(), JobError> {
    let done = sqlx::query!(
        "UPDATE jobs SET status = 'succeeded', result = $2, finished_at = now(),
                         locked_by = NULL, error = NULL, heartbeat_at = NULL
         WHERE id = $1 AND status = 'running' AND attempts = $3",
        job.id,
        result,
        job.attempts,
    )
    .execute(pool)
    .await?;
    if done.rows_affected() == 1 {
        Ok(())
    } else {
        Err(JobError::Permanent(
            "job was reclaimed while running".into(),
        ))
    }
}

async fn record_failure(pool: &PgPool, job: &Job, error: JobError) -> Result<Outcome, sqlx::Error> {
    let retry = !error.is_final(job.attempts);
    let delay = match &error {
        JobError::RetryAfter(_, d) => d.as_secs_f64(),
        _ => 0.0,
    };
    let message: String = error.message().chars().take(MAX_ERROR_LEN).collect();
    tracing::warn!(job_id = %job.id, kind = %job.kind, attempts = job.attempts, retry, error = %message, "job failed");
    let status = if retry { "queued" } else { "failed" };
    sqlx::query!(
        "UPDATE jobs
         SET status = $2, error = $3, locked_by = NULL, heartbeat_at = NULL,
             finished_at = CASE WHEN $2 = 'failed' THEN now() ELSE NULL END,
             run_after = CASE WHEN $2 = 'queued' THEN now() + make_interval(secs => $5)
                              ELSE run_after END
         WHERE id = $1 AND status = 'running' AND attempts = $4",
        job.id,
        status,
        message,
        job.attempts,
        delay,
    )
    .execute(pool)
    .await?;
    Ok(if retry {
        Outcome::Retrying
    } else {
        Outcome::Failed
    })
}

/// Refreshes `jobs.heartbeat_at` every `every` while a phased job runs (EXJ-9). Fenced by
/// `attempts`: a heartbeat never revives a job that was reclaimed and claimed again.
pub struct Heartbeat {
    stop: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<()>>,
}

impl Heartbeat {
    pub fn start(pool: PgPool, job_id: Uuid, attempts: i32, every: Duration) -> Self {
        let (stop, mut stopped) = oneshot::channel::<()>();
        let task = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = &mut stopped => break,
                    () = tokio::time::sleep(every) => {}
                }
                let beat = sqlx::query!(
                    "UPDATE jobs SET heartbeat_at = now()
                     WHERE id = $1 AND status = 'running' AND attempts = $2",
                    job_id,
                    attempts,
                )
                .execute(&pool)
                .await;
                match beat {
                    Ok(r) if r.rows_affected() == 0 => break, // reclaimed or finished
                    Ok(_) => {}
                    Err(e) => tracing::warn!(%job_id, error = %e, "job heartbeat failed"),
                }
            }
        });
        Self {
            stop: Some(stop),
            task: Some(task),
        }
    }

    /// Stops beating and waits for an in-flight beat to finish.
    pub async fn stop(mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(task) = self.task.take() {
            let _ = task.await;
        }
    }
}

impl Drop for Heartbeat {
    fn drop(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

/// Claims and runs one job with a database-only context; `None` when none was due.
pub async fn run_one(pool: &PgPool, worker: &str) -> Result<Option<(Uuid, Outcome)>, sqlx::Error> {
    run_one_in(&JobContext::db_only(pool.clone()), worker).await
}

/// Claims and runs one job; `None` when none was due.
pub async fn run_one_in(
    ctx: &JobContext,
    worker: &str,
) -> Result<Option<(Uuid, Outcome)>, sqlx::Error> {
    let Some(job) = claim_next(&ctx.pool, worker).await? else {
        return Ok(None);
    };
    let outcome = execute_in(ctx, &job).await?;
    Ok(Some((job.id, outcome)))
}

/// Runs due jobs with a database-only context until none is left; returns how many ran.
pub async fn run_pending(pool: &PgPool, worker: &str) -> Result<usize, sqlx::Error> {
    run_pending_in(&JobContext::db_only(pool.clone()), worker).await
}

/// Runs due jobs until none is left; returns how many ran (retries due now count again).
pub async fn run_pending_in(ctx: &JobContext, worker: &str) -> Result<usize, sqlx::Error> {
    let mut ran = 0;
    while run_one_in(ctx, worker).await?.is_some() {
        ran += 1;
    }
    Ok(ran)
}

/// Returns jobs silent (no start or heartbeat) longer than `timeout` to the queue, or
/// fails them at the attempt limit. Returns how many rows changed.
pub async fn reclaim_stale(pool: &PgPool, timeout: Duration) -> Result<u64, sqlx::Error> {
    let seconds = timeout.as_secs_f64();
    let r = sqlx::query!(
        "UPDATE jobs
         SET status = CASE WHEN attempts >= $2 THEN 'failed' ELSE 'queued' END,
             finished_at = CASE WHEN attempts >= $2 THEN now() ELSE NULL END,
             error = 'worker stopped before the job finished', locked_by = NULL,
             heartbeat_at = NULL, run_after = now()
         WHERE status = 'running'
           AND greatest(started_at, heartbeat_at) < now() - make_interval(secs => $1)",
        seconds,
        MAX_ATTEMPTS,
    )
    .execute(pool)
    .await?;
    Ok(r.rows_affected())
}

/// [`worker_loop_in`] with a database-only context (no blob store: export jobs fail).
pub async fn worker_loop(
    pool: PgPool,
    worker: String,
    poll: Duration,
    visibility_timeout: Duration,
    shutdown: impl Future<Output = ()>,
) {
    worker_loop_in(
        JobContext::db_only(pool),
        worker,
        poll,
        visibility_timeout,
        shutdown,
    )
    .await;
}

/// The `asd-backend worker` loop: every minute reclaim orphans and sweep expired export
/// artifacts (EXJ-4), drain the queue, sleep when idle, stop when `shutdown` resolves
/// (between jobs; a running job finishes).
pub async fn worker_loop_in(
    ctx: JobContext,
    worker: String,
    poll: Duration,
    visibility_timeout: Duration,
    shutdown: impl Future<Output = ()>,
) {
    tokio::pin!(shutdown);
    let mut last_reclaim: Option<tokio::time::Instant> = None;
    loop {
        if last_reclaim.is_none_or(|t| t.elapsed() >= RECLAIM_EVERY) {
            match reclaim_stale(&ctx.pool, visibility_timeout).await {
                Ok(0) => {}
                Ok(n) => tracing::warn!(reclaimed = n, "returned orphaned jobs to the queue"),
                Err(e) => tracing::warn!(error = %e, "reclaiming stale jobs failed"),
            }
            if ctx.blobs.is_some() {
                match export::sweep_expired(&ctx).await {
                    Ok(0) => {}
                    Ok(n) => tracing::info!(expired = n, "expired export artifacts"),
                    Err(e) => tracing::warn!(error = %e, "export expiry sweep failed"),
                }
            }
            last_reclaim = Some(tokio::time::Instant::now());
        }
        match run_one_in(&ctx, &worker).await {
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
