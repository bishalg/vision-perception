//! owner: WP-X.3
//!
//! The `analysis_run` job (RUN-3): membership snapshot, paging, level choice, two
//! `asd_core::analysis::evaluate` calls per session, aggregates, the sealed per-session blob
//! and the summary.
//!
//! Phase 8 step 0 stub: the kind is registered and routed by the runner (phased, with a
//! heartbeat and a [`JobContext`] bound to the job, so [`JobContext::progress`] and
//! [`JobContext::cancelled`] work), and the handler fails permanently until WP-X.3 fills it.

use serde_json::Value;

use super::runner::JobError;
use super::{Job, JobContext};

/// Runs one `analysis_run` job; returns the job's `result`.
pub async fn run_analysis(_ctx: &JobContext, _job: &Job) -> Result<Value, JobError> {
    Err(JobError::Permanent("not implemented".into()))
}
