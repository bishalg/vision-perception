//! owner: WP-B.5
//!
//! The job queue (02 §5): rows of the `jobs` table, claimed by `asd-backend worker` with
//! `FOR UPDATE SKIP LOCKED` ([`runner`]) and executed by a handler per `kind`
//! ([`norm_aggregate`] is the first). The API only ever *enqueues* a job; it never runs one,
//! so a slow aggregation cannot hold a request open.
//!
//! Lifecycle: `queued -> running -> succeeded | failed` (`cancelled` is reserved for a
//! later cancel endpoint). A job that fails transiently goes back to `queued` until
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
use crate::error::AppResult;

pub mod export;
pub mod norm_aggregate;
pub mod runner;

/// `jobs.kind` of a norm aggregation (`POST /v1/jobs/norm-aggregate`).
pub const KIND_NORM_AGGREGATE: &str = "norm_aggregate";
/// `jobs.kind` of an export (`POST /v1/orgs/{id}/exports`, EXJ-3).
pub const KIND_EXPORT: &str = "export";
/// Every kind the runner executes.
pub const KINDS: &[&str] = &[KIND_NORM_AGGREGATE, KIND_EXPORT];

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
        }
    }

    pub fn with_heartbeat_every(mut self, every: Duration) -> Self {
        self.heartbeat_every = every;
        self
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
