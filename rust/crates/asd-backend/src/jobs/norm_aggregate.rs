//! owner: WP-B.5
//!
//! Norm aggregation job (CORE-8 wrapped as a job, 02 §5) and promotion of a run to a
//! `draft` baselines document.
//!
//! The statistics live in `asd_core::norms` (median, P10/P90, the n >= 12 cell guard,
//! zero-spread withholding, imputed-row exclusion). This module only
//!
//! 1. selects the *eligible* stored sessions (below),
//! 2. adapts each stored `rawPayload` to the record shape `norms::extract_observations`
//!    reads (`{"per_video": {"ageMonths", "videos"}}`), and
//! 3. stores the candidate set it returns.
//!
//! ## Eligibility (a session feeds a norm only when all hold)
//!
//! - the participant has not withdrawn consent;
//! - `capture_source = 'truedepth'`, or `'synthetic'` when the run asks for
//!   `source_filter.include_synthetic` (dev/test servers only, `ASD_ALLOW_SYNTHETIC`);
//! - the quality verdict is `optimal` or `acceptable` and `valid_frame_ratio >= 0.80`
//!   (the ING-3 `low_quality` threshold);
//! - no review item of the session is still `open` or `in_review`, and none is marked
//!   "exclude from norms" (label `exclude_from_norms = true`, or a `done` item whose
//!   decision has `exclude_from_norms: true`);
//! - a device session additionally needs the participant's norm-collection consent
//!   (`rawPayload.normCollection.consented == true`) **and** a `norm_candidate` review item
//!   that a reviewer closed as `done`. Synthetic rows have no consent to check.
//!
//! Imputed per-clip rows are dropped by `norms::extract_observations` unless the run sets
//! `include_imputed` (default false).

use asd_core::norms::{self, AggregateParams, Buckets, CandidateBaselineSet};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::PgConnection;
use uuid::Uuid;

use super::runner::JobError;
use super::Job;
use crate::audit::AuditEvent;
use crate::auth::extractors::{RequestMeta, UserPrincipal};
use crate::error::{AppError, AppResult};
use crate::problem::InvalidParam;

/// `ING-3` low-quality threshold, reused as the validity floor for norm data.
pub const MIN_VALID_FRAME_RATIO: f64 = 0.80;
/// Smallest `min_n` a run may use: the clinical floor of CORE-8.
pub const MIN_N_FLOOR: usize = norms::DEFAULT_MIN_N;
pub const MIN_N_CEILING: usize = 100_000;
/// Sessions read per page; bounds memory regardless of table size.
const PAGE: i64 = 500;

fn default_min_n() -> usize {
    norms::DEFAULT_MIN_N
}

/// Which stored sessions are considered.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceFilter {
    /// One org's sessions. Org-scoped callers always get their own org; a superadmin may
    /// omit it to aggregate across every org.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub org_id: Option<Uuid>,
    /// Include `capture_source = 'synthetic'` rows (default false; needs
    /// `ASD_ALLOW_SYNTHETIC`). Synthetic-derived norms are for pipeline checks only.
    #[serde(default)]
    pub include_synthetic: bool,
    /// Only sessions started at or after this instant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<DateTime<Utc>>,
    /// Only sessions started before this instant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<DateTime<Utc>>,
}

/// `POST /v1/jobs/norm-aggregate` body, also stored as the job's (and run's) params.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NormParams {
    #[serde(default = "default_min_n")]
    pub min_n: usize,
    #[serde(default)]
    pub include_imputed: bool,
    #[serde(default)]
    pub source_filter: SourceFilter,
}

impl Default for NormParams {
    fn default() -> Self {
        Self {
            min_n: default_min_n(),
            include_imputed: false,
            source_filter: SourceFilter::default(),
        }
    }
}

impl NormParams {
    pub fn validate(&self) -> Result<(), Vec<InvalidParam>> {
        let mut errors = Vec::new();
        if !(MIN_N_FLOOR..=MIN_N_CEILING).contains(&self.min_n) {
            errors.push(InvalidParam {
                name: "min_n".into(),
                reason: format!("must be between {MIN_N_FLOOR} and {MIN_N_CEILING}"),
            });
        }
        if let (Some(from), Some(to)) = (self.source_filter.from, self.source_filter.to) {
            if from >= to {
                errors.push(InvalidParam {
                    name: "source_filter.from".into(),
                    reason: "must be before source_filter.to".into(),
                });
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    fn aggregate_params(&self) -> AggregateParams {
        AggregateParams {
            min_n: self.min_n,
            include_imputed: self.include_imputed,
            brackets: norms::default_age_brackets(),
        }
    }
}

/// The record `norms::extract_observations` reads, built from a stored `rawPayload`:
/// `perVideo[]` are the per-clip reports (`stimulusId`, `rows[]`) and `ageMonths` the
/// child's age at capture. `None` when either is missing (the session cannot be placed in
/// an age bracket or has no scored clips).
pub fn norm_record(per_video: Option<Value>, age_months: Option<Value>) -> Option<Value> {
    let videos = per_video.filter(|v| v.as_array().is_some_and(|a| !a.is_empty()))?;
    let age = age_months.filter(Value::is_number)?;
    Some(json!({"per_video": {"ageMonths": age, "videos": videos}}))
}

/// Runs the aggregation for `job` on `tx` (a `REPEATABLE READ` transaction, so every page
/// sees the same snapshot) and stores the `norm_runs` row plus one system audit row.
/// Returns the job's `result`.
pub async fn execute(tx: &mut PgConnection, job: &Job) -> Result<Value, JobError> {
    let params: NormParams = serde_json::from_value(job.params.clone())
        .map_err(|e| JobError::Permanent(format!("invalid norm-aggregate params: {e}")))?;
    params
        .validate()
        .map_err(|e| JobError::Permanent(format!("invalid norm-aggregate params: {e:?}")))?;
    let agg = params.aggregate_params();
    let filter = &params.source_filter;
    let org = job.org_id.or(filter.org_id);

    let mut buckets = Buckets::new();
    let (mut considered, mut used) = (0usize, 0usize);
    let mut after = String::new();
    loop {
        let rows = sqlx::query!(
            r#"SELECT s.id,
                      s.summary -> 'perVideo' AS per_video,
                      s.summary -> 'ageMonths' AS age_months
               FROM sessions s
               JOIN participants p ON p.id = s.participant_id
               WHERE s.id > $1
                 AND ($2::uuid IS NULL OR s.org_id = $2)
                 AND ($3::timestamptz IS NULL OR s.started_at >= $3)
                 AND ($4::timestamptz IS NULL OR s.started_at < $4)
                 AND p.withdrawn_at IS NULL
                 AND s.quality_verdict IN ('optimal', 'acceptable')
                 AND s.valid_frame_ratio >= $5
                 AND (s.capture_source = 'truedepth'
                      OR ($6 AND s.capture_source = 'synthetic'))
                 AND NOT EXISTS (
                       SELECT 1 FROM review_items r
                       WHERE r.session_id = s.id
                         AND (r.status IN ('open', 'in_review')
                              OR (r.status = 'done'
                                  AND r.decision -> 'exclude_from_norms' = 'true'::jsonb)))
                 AND NOT EXISTS (
                       SELECT 1 FROM labels l
                       WHERE l.session_id = s.id AND l.key = 'exclude_from_norms'
                         AND l.value = 'true'::jsonb)
                 AND (s.capture_source = 'synthetic'
                      OR (s.summary -> 'normCollection' -> 'consented' = 'true'::jsonb
                          AND EXISTS (
                                SELECT 1 FROM review_items r
                                WHERE r.session_id = s.id AND r.reason = 'norm_candidate'
                                  AND r.status = 'done')))
               ORDER BY s.id
               LIMIT $7"#,
            after,
            org,
            filter.from,
            filter.to,
            MIN_VALID_FRAME_RATIO,
            filter.include_synthetic,
            PAGE,
        )
        .fetch_all(&mut *tx)
        .await?;
        let Some(last) = rows.last() else { break };
        after = last.id.clone();
        let mut records = Vec::with_capacity(rows.len());
        for row in rows {
            considered += 1;
            if let Some(record) = norm_record(row.per_video, row.age_months) {
                used += 1;
                records.push(record);
            }
        }
        norms::extract_observations(&records, &agg, &mut buckets);
    }
    let candidate = norms::build_candidate_baseline(&buckets, &agg);
    let qualifying = norms::qualifying_cells(&candidate);
    let total_cells = total_cells(&candidate);

    let source = sqlx::query_scalar!(
        "SELECT id FROM config_documents
         WHERE kind = 'normative_baselines' AND status = 'published'
         ORDER BY published_at DESC NULLS LAST, created_at DESC LIMIT 1"
    )
    .fetch_optional(&mut *tx)
    .await?;
    let mut stored_params = job.params.clone();
    stored_params["brackets"] = json!(agg.brackets.iter().map(|b| &b.id).collect::<Vec<_>>());
    let cells = serde_json::to_value(&candidate)
        .map_err(|e| JobError::Permanent(format!("candidate does not serialise: {e}")))?;
    let n_total = i32::try_from(used).unwrap_or(i32::MAX);
    let run_id = sqlx::query_scalar!(
        r#"INSERT INTO norm_runs
             (baselines_source_id, job_id, params, cells, n_total, created_by, org_id)
           VALUES ($1, $2, $3, $4, $5, $6, $7)
           RETURNING id"#,
        source,
        job.id,
        stored_params,
        cells,
        n_total,
        job.created_by,
        org,
    )
    .fetch_one(&mut *tx)
    .await?;

    let stats = json!({
        "norm_run_id": run_id,
        "sessions_considered": considered,
        "sessions_used": used,
        "sessions_skipped": considered - used,
        "qualifying_cells": qualifying,
        "total_cells": total_cells,
    });
    let mut audit = AuditEvent::system("norm_run.create", "norm_run", run_id)
        .after(json!({"job_id": job.id, "created_by": job.created_by, "stats": stats}));
    if let Some(org) = org {
        audit = audit.org(org);
    }
    audit.record(&mut *tx).await.map_err(JobError::from)?;
    Ok(stats)
}

/// Cells in the candidate, withheld ones included.
pub fn total_cells(set: &CandidateBaselineSet) -> usize {
    set.videos
        .values()
        .flat_map(|v| &v.segments)
        .flat_map(|s| &s.expectations)
        .map(|e| e.by_age.len())
        .sum()
}

/// What `POST /norm-runs/{id}/promote` created.
#[derive(Debug, Clone, PartialEq)]
pub struct Promoted {
    pub config_document_id: Uuid,
    pub version: String,
}

/// Writes the run's candidate set as a **draft** `normative_baselines` document, links the
/// run to it and audits. Never publishes: publishing is B.4's `ConfigPublish` action.
pub async fn promote(
    tx: &mut PgConnection,
    run_id: Uuid,
    user: &UserPrincipal,
    meta: &RequestMeta,
) -> AppResult<Promoted> {
    // Row lock: two concurrent promotes of one run cannot both create a document.
    let run = sqlx::query!(
        r#"SELECT org_id, cells, n_total, promoted_to_config_id, created_at
           FROM norm_runs WHERE id = $1 FOR UPDATE"#,
        run_id,
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::NotFound)?;
    if !run_visible(user, run.org_id) {
        return Err(AppError::NotFound);
    }
    if run.promoted_to_config_id.is_some() {
        return Err(AppError::Conflict {
            slug: "norm-run-promoted",
            detail: "this norm run was already promoted to a draft baselines document".into(),
        });
    }
    let candidate: CandidateBaselineSet = serde_json::from_value(run.cells.clone())
        .map_err(|_| AppError::Internal("stored norm run cells are not a candidate set".into()))?;
    let qualifying = norms::qualifying_cells(&candidate);
    if qualifying == 0 {
        return Err(AppError::Conflict {
            slug: "no-qualifying-cells",
            detail: "every cell of this run was withheld (n below min_n or zero spread)".into(),
        });
    }

    let version = format!(
        "{}-run-{}",
        run.created_at.format("%Y%m%d"),
        &run_id.simple().to_string()[..8]
    );
    let notes = format!(
        "Candidate norms from norm run {run_id} ({} sessions, {qualifying} qualifying cells). \
         Draft only: needs clinical review before it can be published.",
        run.n_total
    );
    let doc = sqlx::query_scalar!(
        r#"INSERT INTO config_documents (kind, version, status, body, created_by, notes)
           VALUES ('normative_baselines', $1, 'draft', $2, $3, $4)
           ON CONFLICT (kind, version) DO NOTHING
           RETURNING id"#,
        version,
        run.cells,
        user.user_id,
        notes,
    )
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::Conflict {
        slug: "conflict",
        detail: "a baselines document with this version already exists".into(),
    })?;
    sqlx::query!(
        "UPDATE norm_runs SET promoted_to_config_id = $2 WHERE id = $1",
        run_id,
        doc,
    )
    .execute(&mut *tx)
    .await?;

    let mut audit = AuditEvent::user(user, "norm_run.promote", "norm_run", run_id)
        .after(json!({
            "config_document_id": doc,
            "kind": "normative_baselines",
            "version": version,
            "status": "draft",
            "qualifying_cells": qualifying,
        }))
        .meta(meta);
    if let Some(org) = run.org_id {
        audit = audit.org(org);
    }
    audit.record(&mut *tx).await?;
    Ok(Promoted {
        config_document_id: doc,
        version,
    })
}

/// A run scoped to an org is visible to that org's users and to a superadmin; an
/// all-orgs run (`org_id` NULL) only to a superadmin.
pub fn run_visible(user: &UserPrincipal, run_org: Option<Uuid>) -> bool {
    user.is_superadmin() || (run_org.is_some() && run_org == user.org_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn params_default_to_the_core_defaults() {
        let p: NormParams = serde_json::from_value(json!({})).unwrap();
        assert_eq!(p, NormParams::default());
        assert_eq!(p.min_n, 12);
        assert!(!p.include_imputed);
        assert!(p.validate().is_ok());
    }

    #[test]
    fn min_n_below_the_floor_and_unknown_fields_are_rejected() {
        let p = NormParams {
            min_n: 11,
            ..Default::default()
        };
        assert_eq!(p.validate().unwrap_err()[0].name, "min_n");
        assert!(serde_json::from_value::<NormParams>(json!({"min_n": 12, "x": 1})).is_err());
        assert!(
            serde_json::from_value::<NormParams>(json!({"source_filter": {"org": "x"}})).is_err()
        );
    }

    #[test]
    fn inverted_time_window_is_rejected() {
        let p: NormParams = serde_json::from_value(json!({"source_filter": {
            "from": "2026-02-01T00:00:00Z", "to": "2026-01-01T00:00:00Z"}}))
        .unwrap();
        assert_eq!(p.validate().unwrap_err()[0].name, "source_filter.from");
    }

    #[test]
    fn record_needs_scored_clips_and_a_numeric_age() {
        let videos = json!([{"stimulusId": "s", "rows": []}]);
        assert!(norm_record(Some(videos.clone()), Some(json!(30))).is_some());
        assert!(norm_record(Some(videos.clone()), None).is_none());
        assert!(norm_record(Some(videos), Some(json!("30"))).is_none());
        assert!(norm_record(Some(json!([])), Some(json!(30))).is_none());
        assert!(norm_record(None, Some(json!(30))).is_none());
    }
}
