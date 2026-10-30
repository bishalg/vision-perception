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

use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::PgConnection;
use uuid::Uuid;

use crate::error::AppResult;

pub mod norm_aggregate;
pub mod runner;

/// `jobs.kind` of a norm aggregation (`POST /v1/jobs/norm-aggregate`).
pub const KIND_NORM_AGGREGATE: &str = "norm_aggregate";

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
