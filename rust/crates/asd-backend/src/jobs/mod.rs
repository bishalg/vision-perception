//! owner: WP-B.5
//!
//! The job queue (02 §5): rows of the `jobs` table, claimed by `asd-backend worker` with
//! `FOR UPDATE SKIP LOCKED` ([`runner`]) and executed by a handler per `kind`
//! ([`norm_aggregate`] is the first). The API only ever *enqueues* a job; it never runs one,
//! so a slow aggregation cannot hold a request open.
//!
//! Lifecycle: `queued -> running -> succeeded | failed` (`cancelled` is reserved for a
//! later cancel endpoint). RUN-8 (WP-X.0): `jobs.progress` and `jobs.cancel_requested_at`
//! are written and read through [`JobContext::progress`] / [`JobContext::cancelled`]; the
//! cancel route and the `cancelled` outcome come with WP-X.3. A job that fails transiently goes back to `queued` until
//! [`runner::MAX_ATTEMPTS`] is reached; a job whose params can never succeed fails at once.

//!
//! WP-I.6 (G5): handlers that do network I/O (export rendering and upload) are *phased*:
//! they get a [`JobContext`] (pool, blob store, keyring, config) instead of one open
//! transaction, open short transactions themselves, and the runner heartbeats
//! `jobs.heartbeat_at` while they run. Queued jobs wait for `jobs.run_after` (delayed
//! retry, [`runner::JobError::RetryAfter`]).

use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use crate::app::AppState;
use crate::blob::BlobStore;
use crate::config::Config;
use crate::crypto::Keyring;
use crate::error::{AppError, AppResult};
use asd_contract::api_v1::JobProgress;

pub mod analysis; // WP-X.3 (stub from phase 8 step 0)
pub mod deliver; // WP-I.7
pub mod erasure; // WP-B.9
pub mod export;
pub mod housekeeping; // WP-B.12
pub mod norm_aggregate;
pub mod retention; // WP-B.9
pub mod runner;

/// `jobs.kind` of a norm aggregation (`POST /v1/jobs/norm-aggregate`).
pub const KIND_NORM_AGGREGATE: &str = "norm_aggregate";
/// `jobs.kind` of an export (`POST /v1/orgs/{id}/exports`, EXJ-3).
pub const KIND_EXPORT: &str = "export";
/// `jobs.kind` of one delivery of an export to a destination (DLV-1, WP-I.7).
pub const KIND_DELIVER: &str = "export_deliver";
/// `jobs.kind` of one erasure request's execution (WP-B.9).
pub const KIND_ERASURE: &str = "erasure";
/// `jobs.kind` of a retention sweep, nightly or on request (WP-B.9).
pub const KIND_RETENTION_SWEEP: &str = "retention_sweep";
/// `jobs.kind` of the daily housekeeping sweep (WP-B.12).
pub const KIND_HOUSEKEEPING: &str = "housekeeping_sweep";
/// `jobs.kind` of an analysis run (`POST /v1/orgs/{id}/analysis-runs`, RUN-3; WP-X.3).
pub const KIND_ANALYSIS: &str = "analysis_run";
/// Every kind the runner executes.
pub const KINDS: &[&str] = &[
    KIND_NORM_AGGREGATE,
    KIND_EXPORT,
    KIND_DELIVER,
    KIND_ERASURE,
    KIND_RETENTION_SWEEP,
    KIND_HOUSEKEEPING,
    KIND_ANALYSIS,
];

/// How often a running phased job refreshes `jobs.heartbeat_at` by default: well inside
/// the 15-minute visibility timeout.
pub const DEFAULT_HEARTBEAT_EVERY: Duration = Duration::from_secs(60);

/// What a handler may use besides its job row (G5). Database-only contexts (the norm
/// tests' `runner::run_pending(pool, ..)`) have no blob store, keyring or config; a
/// handler that needs one fails permanently with a clear reason.
#[derive(Clone)]
pub struct JobContext {
    pub pool: PgPool,
    pub blobs: Option<Arc<dyn BlobStore>>,
    pub keyring: Option<Arc<Keyring>>,
    pub config: Option<Arc<Config>>,
    /// Heartbeat period of phased jobs (tests shorten it).
    pub heartbeat_every: Duration,
    /// `(id, attempts)` of the job this context runs, set by the runner for phased handlers
    /// ([`JobContext::for_job`]); `None` in a context not yet bound to a job.
    pub current: Option<(Uuid, i32)>,
}

impl JobContext {
    /// A context with only the database.
    pub fn db_only(pool: PgPool) -> Self {
        Self {
            pool,
            blobs: None,
            keyring: None,
            config: None,
            heartbeat_every: DEFAULT_HEARTBEAT_EVERY,
            current: None,
        }
    }

    /// Everything the API process has: the worker builds its context the same way.
    pub fn from_state(state: &AppState) -> Self {
        Self {
            pool: state.db.clone(),
            blobs: Some(state.blobs.clone()),
            keyring: state.keyring.clone(),
            config: Some(state.config.clone()),
            heartbeat_every: DEFAULT_HEARTBEAT_EVERY,
            current: None,
        }
    }

    pub fn with_heartbeat_every(mut self, every: Duration) -> Self {
        self.heartbeat_every = every;
        self
    }

    /// This context bound to `job` (for [`progress`](Self::progress) and
    /// [`cancelled`](Self::cancelled)).
    pub fn for_job(&self, job: &Job) -> Self {
        Self {
            current: Some((job.id, job.attempts)),
            ..self.clone()
        }
    }

    fn current_job(&self) -> AppResult<(Uuid, i32)> {
        self.current
            .ok_or_else(|| AppError::Internal("job context is not bound to a job".into()))
    }

    /// RUN-8: records `{done, total, phase}` in `jobs.progress` and refreshes the heartbeat.
    /// Fenced by `attempts` like the heartbeat: a reclaimed run writes nothing (and gets
    /// `Ok`, since the runner's own fence reports the reclaim).
    pub async fn progress(&self, done: u64, total: u64, phase: &str) -> AppResult<()> {
        let (id, attempts) = self.current_job()?;
        let progress = serde_json::to_value(JobProgress {
            done,
            total,
            phase: phase.to_owned(),
        })
        .map_err(|e| AppError::Internal(format!("job progress: {e}")))?;
        sqlx::query!(
            "UPDATE jobs SET progress = $2, heartbeat_at = now()
             WHERE id = $1 AND status = 'running' AND attempts = $3",
            id,
            progress,
            attempts,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// RUN-8: whether `POST /v1/jobs/{id}/cancel` asked this job to stop. Handlers check it
    /// between pages; a vanished job counts as cancelled.
    pub async fn cancelled(&self) -> AppResult<bool> {
        let (id, _) = self.current_job()?;
        let requested = sqlx::query_scalar!(
            r#"SELECT cancel_requested_at IS NOT NULL AS "requested!" FROM jobs WHERE id = $1"#,
            id,
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(requested.unwrap_or(true))
    }
}

/// A `jobs` row.
#[derive(Debug, Clone, PartialEq)]
pub struct Job {
    pub id: Uuid,
    pub kind: String,
    pub status: String,
    pub params: Value,
    pub result: Option<Value>,
    pub attempts: i32,
    pub org_id: Option<Uuid>,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
}

/// Queues a job inside the caller's transaction (so the audit row and the job commit
/// together) and returns it.
pub async fn enqueue(
    conn: &mut PgConnection,
    kind: &str,
    org_id: Option<Uuid>,
    params: &Value,
    created_by: Option<Uuid>,
) -> AppResult<Job> {
    let r = sqlx::query!(
        r#"INSERT INTO jobs (kind, org_id, params, created_by)
           VALUES ($1, $2, $3, $4)
           RETURNING id, kind, status, params, result, attempts, org_id, created_by,
                     created_at, started_at, finished_at, error"#,
        kind,
        org_id,
        params,
        created_by,
    )
    .fetch_one(&mut *conn)
    .await?;
    Ok(Job {
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
    })
}

/// One job by id.
pub async fn get(conn: &mut PgConnection, id: Uuid) -> AppResult<Option<Job>> {
    let r = sqlx::query!(
        r#"SELECT id, kind, status, params, result, attempts, org_id, created_by,
                  created_at, started_at, finished_at, error
           FROM jobs WHERE id = $1"#,
        id,
    )
    .fetch_optional(&mut *conn)
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
