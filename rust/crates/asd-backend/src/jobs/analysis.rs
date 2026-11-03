//! owner: WP-X.3
//!
//! The `analysis_run` job (`docs/spec/modules/asd-research-toolkit/technical.md` RUN-3..5,
//! RUN-8, RUN-9, RPX-3, PRV-1, GOV-5): applies an experiment profile to a cohort against the
//! published profile and writes the run's summary and a sealed per-session result blob.
//!
//! Phases (a phased handler, [`JobContext`] bound to the job):
//!
//! 1. **Membership** ([`membership`]): the cohort SQL inside one `REPEATABLE READ READ ONLY`
//!    snapshot. Research consent, "not withdrawn" and real capture are always applied
//!    (COH-1); a session without a placeable age is excluded `no_age`; at `level: frames` a
//!    session without a research log is excluded `no_frame_log` (`frames_swept` when the
//!    retention sweep deleted it). `membership_hash` = sha256 of the sorted ids, one per
//!    line, of every cohort member (evaluated or excluded). The budget is checked again.
//! 2. **Evaluate** in pages of `ASD_ANALYSIS_PAGE_SIZE`: before each page the cancel request
//!    (`jobs.cancel_requested_at`) and the deadline (`ASD_ANALYSIS_MAX_MINUTES`) are
//!    checked, and `jobs.progress` is written. Each session is evaluated twice with
//!    `asd_core::analysis::evaluate` (published profile + published baselines, experiment
//!    profile + its baselines) at its level (RPX-3: `frames` when the research log is read
//!    and opened, else `score`), the draft side gets `param_not_applied: <id>` for every
//!    changed registry knob its level cannot apply, the row goes to the `Aggregator` and
//!    to the JSONL buffer.
//! 3. **Seal** the rows under the org data key (`key_id (16 bytes) || ciphertext`, AAD
//!    `analysis_runs:result:<org>:<run>`, [`seal_result`] / [`open_result`]) and store
//!    them at [`result_key`]; more than `ASD_ANALYSIS_MAX_RESULT_MIB` fails the run
//!    `result-too-large`.
//! 4. **Record**: `summary` (PRV-1 applied with the org's `small_cell_k`), counts,
//!    `finished_at`, and the system audit `analysis.run_complete` (counts, no values).
//!
//! A cancelled run ends `cancelled` with nothing stored (the blob is written only after the
//! last check); the deadline fails it `budget_exceeded`. GOV-5: this module only reads
//! sessions, participants, organizations and config documents, and writes `analysis_runs`,
//! `jobs` (through the runner and [`JobContext`]), the blob store and `audit_log`.

use std::collections::{BTreeMap, HashMap};
use std::time::Instant;

use asd_contract::api_v1::{ConfigDocumentKind, ScoringStatus};
use asd_contract::params::{self, Reprocess};
use asd_contract::research::{CohortDefinition, LevelRequest as WireLevel};
use asd_core::analysis::{
    changed_params, evaluate, suppress, AggregatorOptions, AnalysisProfile, Evaluation, Level,
    LevelRequest, ResearchInput, RowSide, SessionInput,
};
use asd_core::analysis::{Aggregator, SessionRow};
use asd_core::gaze::raw_log;
use asd_core::gaze::reprocess::ReprocessOptions;
use asd_core::norms::bracket_id_for;
use asd_core::types::{AgeBracket, NormativeBaselineSet, PerVideoScoreReport, StimulusCatalog};
use bytes::Bytes;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::PgConnection;
use uuid::Uuid;

use super::runner::JobError;
use super::{Job, JobContext, KIND_ANALYSIS};
use crate::audit::AuditEvent;
use crate::config::AnalysisLimits;
use crate::config_docs::{kind_str, seed};
use crate::crypto::{Aad, Keyring};
use crate::error::{AppError, AppResult};
use crate::routes::blobs::{read_research_log, ResearchLogRow};
use crate::routes::research::{check_definition, org_small_cell_k};

/// What the blob store holds for a result: ciphertext, never JSON.
pub const RESULT_CONTENT_TYPE: &str = "application/octet-stream";
/// Length of the data-key id prefixed to a sealed result.
const KEY_ID_LEN: usize = 16;
/// Failure reason at the deadline (RUN-9).
pub const BUDGET_EXCEEDED: &str = "budget_exceeded";

/// Exclusion reasons of a run (RUN-3, RPX-3).
pub mod reasons {
    /// No age, or an age outside every band of the experiment profile.
    pub const NO_AGE: &str = "no_age";
    /// `level: frames` and no research log was ever uploaded.
    pub const NO_FRAME_LOG: &str = "no_frame_log";
    /// `level: frames` and the retention sweep (or an erasure) deleted the log.
    pub const FRAMES_SWEPT: &str = "frames_swept";
    /// The stored log could not be read, opened or parsed.
    pub const FRAME_LOG_UNREADABLE: &str = "frame_log_unreadable";
    /// The session was deleted (erasure) after the membership snapshot.
    pub const SESSION_GONE: &str = "session_gone";
}

/// `analysis/{org}/{run}/sessions.jsonl.sealed` (RUN-3).
pub fn result_key(org: Uuid, run: Uuid) -> String {
    format!("analysis/{org}/{run}/sessions.jsonl.sealed")
}

fn result_aad(org: Uuid, run: Uuid) -> Aad {
    Aad::new("analysis_runs", "result", org, run)
}

/// Seals the per-session JSONL of `run` under the org's active data key.
pub async fn seal_result(
    keyring: &Keyring,
    org: Uuid,
    run: Uuid,
    plaintext: &[u8],
) -> AppResult<Vec<u8>> {
    let sealed = keyring
        .encrypt(org, &result_aad(org, run), plaintext)
        .await?;
    let mut out = Vec::with_capacity(KEY_ID_LEN + sealed.ciphertext.len());
    out.extend_from_slice(sealed.key_id.as_bytes());
    out.extend_from_slice(&sealed.ciphertext);
    Ok(out)
}

/// Opens what [`seal_result`] produced (the sessions endpoint and the export read it).
pub async fn open_result(
    keyring: &Keyring,
    org: Uuid,
    run: Uuid,
    sealed: &[u8],
) -> AppResult<Vec<u8>> {
    if sealed.len() <= KEY_ID_LEN {
        return Err(AppError::Internal(
            "sealed analysis result is truncated".into(),
        ));
    }
    let (id, ciphertext) = sealed.split_at(KEY_ID_LEN);
    let key_id = Uuid::from_slice(id)
        .map_err(|_| AppError::Internal("sealed analysis result has no key id".into()))?;
    Ok(keyring
        .decrypt(org, &result_aad(org, run), key_id, ciphertext)
        .await?)
}

// ---- documents -------------------------------------------------------------------------------

/// The newest published document of `kind` as `(version, body)`, else the bundled default
/// (`bundled-<schema>`), the documented fallback of every number.
pub async fn published_document(
    conn: &mut PgConnection,
    kind: ConfigDocumentKind,
) -> AppResult<(String, Value)> {
    let row = sqlx::query!(
        "SELECT version, body FROM config_documents
         WHERE kind = $1 AND status = 'published'
         ORDER BY published_at DESC LIMIT 1",
        kind_str(kind)
    )
    .fetch_optional(&mut *conn)
    .await?;
    match row {
        Some(r) => Ok((r.version, r.body)),
        None => bundled(kind),
    }
}

fn bundled(kind: ConfigDocumentKind) -> AppResult<(String, Value)> {
    seed::seed_documents()?
        .into_iter()
        .find(|(k, _, _)| *k == kind)
        .map(|(_, v, b)| (v, b))
        .ok_or_else(|| AppError::Internal("no bundled default for a config kind".into()))
}

/// The body of `kind`/`version`: a stored document (any status), else the bundled default
/// of that version; `None` when neither exists.
pub async fn document(
    conn: &mut PgConnection,
    kind: ConfigDocumentKind,
    version: &str,
) -> AppResult<Option<Value>> {
    let body = sqlx::query_scalar!(
        "SELECT body FROM config_documents WHERE kind = $1 AND version = $2",
        kind_str(kind),
        version
    )
    .fetch_optional(&mut *conn)
    .await?;
    if body.is_some() {
        return Ok(body);
    }
    let (v, b) = bundled(kind)?;
    Ok((v == version).then_some(b))
}

// ---- membership (COH-1, RUN-3) -----------------------------------------------------------------

/// One evaluated cohort session.
#[derive(Debug, Clone, PartialEq)]
pub struct Member {
    pub id: String,
    pub age_months: Option<i32>,
    pub band: Option<String>,
    pub device_id: Option<Uuid>,
    /// The level it is evaluated at.
    pub level: Level,
    /// The research log columns (present when `level` is `frames`).
    pub log: Option<ResearchLogRow>,
}

/// The cohort as a run sees it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Membership {
    /// Evaluated sessions, sorted by id.
    pub members: Vec<Member>,
    /// Reason -> cohort sessions not evaluated.
    pub excluded: BTreeMap<String, u32>,
    /// Every cohort session (evaluated or excluded), sorted.
    pub ids: Vec<String>,
    pub includes_synthetic: bool,
}

impl Membership {
    pub fn frames_n(&self) -> u32 {
        count(self.members.iter().filter(|m| m.level == Level::Frames))
    }

    pub fn score_n(&self) -> u32 {
        count(self.members.iter().filter(|m| m.level == Level::Score))
    }

    /// sha256 of the sorted ids, one per line (RUN-5).
    pub fn hash(&self) -> Vec<u8> {
        let mut h = Sha256::new();
        for id in &self.ids {
            h.update(id.as_bytes());
            h.update(b"\n");
        }
        h.finalize().to_vec()
    }
}

fn count<T>(it: impl Iterator<Item = T>) -> u32 {
    u32::try_from(it.count()).unwrap_or(u32::MAX)
}

fn wire<T: serde::Serialize>(v: &T) -> String {
    match serde_json::to_value(v) {
        Ok(Value::String(s)) => s,
        _ => String::new(),
    }
}

/// The core level request of a wire level.
pub fn core_level(level: WireLevel) -> LevelRequest {
    match level {
        WireLevel::Auto => LevelRequest::Auto,
        WireLevel::Score => LevelRequest::Score,
        WireLevel::Frames => LevelRequest::Frames,
    }
}

/// Resolves the cohort `def` of `org` with the band edges of `bands` (the experiment
/// profile's, COH-3) and picks each session's level (RPX-3). Only `SELECT`s.
pub async fn membership(
    conn: &mut PgConnection,
    org: Uuid,
    def: &CohortDefinition,
    bands: &[AgeBracket],
    level: LevelRequest,
    allow_synthetic: bool,
) -> AppResult<Membership> {
    let ((from, to), devices) =
        check_definition(def, allow_synthetic).map_err(AppError::Validation)?;
    let statuses: Vec<String> = if def.scoring_status.is_empty() {
        vec![wire(&ScoringStatus::Scored)]
    } else {
        def.scoring_status.iter().map(wire).collect()
    };
    let quality: Vec<String> = def.quality_verdicts.iter().map(wire).collect();
    let tracking: Vec<String> = def.tracking_verdicts.iter().map(wire).collect();
    let synthetic = def.include_synthetic && allow_synthetic;
    let rows = sqlx::query!(
        r#"SELECT s.id, s.age_months, s.device_id,
                  s.capture_source = 'synthetic' AS "synthetic!",
                  s.research_frames_blob_key, s.research_frames_bytes,
                  s.research_frames_sha256,
                  s.research_frames_purged_at IS NOT NULL AS "purged!"
           FROM sessions s JOIN participants p ON p.id = s.participant_id
           WHERE s.org_id = $1
             AND ($2::timestamptz IS NULL OR s.received_at >= $2)
             AND ($3::timestamptz IS NULL OR s.received_at <= $3)
             AND (cardinality($4::uuid[]) = 0 OR s.device_id = ANY($4))
             AND (s.capture_source = 'truedepth'
                  OR ($5::bool AND s.capture_source = 'synthetic'))
             AND (NOT $6::bool OR s.research_frames_blob_key IS NOT NULL)
             AND COALESCE(s.summary -> 'normCollection' -> 'consented' = 'true'::jsonb, false)
             AND p.withdrawn_at IS NULL
             AND COALESCE(s.scoring_status, 'scored') = ANY($7::text[])
             AND (cardinality($8::text[]) = 0 OR s.quality_verdict = ANY($8::text[]))
             AND (cardinality($9::text[]) = 0 OR s.tracking_verdict = ANY($9::text[]))
             AND ($10::float8 IS NULL OR s.valid_frame_ratio >= $10)
           ORDER BY s.id"#,
        org,
        from,
        to,
        &devices,
        synthetic,
        def.require_frames,
        &statuses,
        &quality,
        &tracking,
        def.min_valid_ratio,
    )
    .fetch_all(&mut *conn)
    .await?;

    let selected: Vec<&str> = def.age_bands.iter().map(String::as_str).collect();
    let mut out = Membership::default();
    for r in rows {
        let band = r
            .age_months
            .and_then(|a| bracket_id_for(i64::from(a), bands));
        if let Some(b) = &band {
            if !selected.is_empty() && !selected.contains(&b.as_str()) {
                continue; // outside the selected bands: not a member
            }
        }
        out.ids.push(r.id.clone());
        out.includes_synthetic |= r.synthetic;
        if band.is_none() {
            *out.excluded.entry(reasons::NO_AGE.into()).or_insert(0) += 1;
            continue;
        }
        let has_log = r.research_frames_blob_key.is_some();
        let level = match (level, has_log) {
            (LevelRequest::Score, _) | (LevelRequest::Auto, false) => Level::Score,
            (LevelRequest::Auto | LevelRequest::Frames, true) => Level::Frames,
            (LevelRequest::Frames, false) => {
                let reason = if r.purged {
                    reasons::FRAMES_SWEPT
                } else {
                    reasons::NO_FRAME_LOG
                };
                *out.excluded.entry(reason.into()).or_insert(0) += 1;
                continue;
            }
        };
        let log = (level == Level::Frames).then(|| ResearchLogRow {
            session_id: r.id.clone(),
            org_id: org,
            blob_key: r.research_frames_blob_key.clone(),
            bytes: r.research_frames_bytes,
            sha256: r.research_frames_sha256.clone(),
            purged: r.purged,
        });
        out.members.push(Member {
            id: r.id,
            age_months: r.age_months,
            band,
            device_id: r.device_id,
            level,
            log,
        });
    }
    Ok(out)
}

// ---- budget (RUN-9, SWP-2) ---------------------------------------------------------------------

/// Why a run is refused before (or stopped at the start of) its evaluation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BudgetError {
    pub slug: &'static str,
    pub detail: String,
}

/// RUN-9: sessions and evaluations (`sessions x cells`) per level against the limits.
pub fn check_budget(
    limits: &AnalysisLimits,
    score_n: u32,
    frames_n: u32,
    cells: u64,
    sweep: bool,
) -> Result<(), BudgetError> {
    let slug = asd_contract::problem::COHORT_TOO_LARGE;
    let total = u64::from(score_n) + u64::from(frames_n);
    if total > u64::from(limits.max_sessions) {
        return Err(BudgetError {
            slug,
            detail: format!(
                "the cohort has {total} sessions; a run takes at most {} (ASD_ANALYSIS_MAX_SESSIONS)",
                limits.max_sessions
            ),
        });
    }
    if frames_n > limits.max_frame_sessions {
        return Err(BudgetError {
            slug,
            detail: format!(
                "{frames_n} sessions at the frames level; a run takes at most {} \
                 (ASD_ANALYSIS_MAX_FRAME_SESSIONS); use level score or a smaller cohort",
                limits.max_frame_sessions
            ),
        });
    }
    let score_eval = u64::from(score_n).saturating_mul(cells);
    let frames_eval = u64::from(frames_n).saturating_mul(cells);
    if score_eval > limits.max_evaluations || frames_eval > limits.max_frame_evaluations {
        return Err(BudgetError {
            slug: if sweep {
                asd_contract::problem::SWEEP_TOO_LARGE
            } else {
                slug
            },
            detail: format!(
                "{score_eval} score-level and {frames_eval} frames-level evaluations; the limits \
                 are {} and {} (ASD_ANALYSIS_MAX_EVALUATIONS, ASD_ANALYSIS_MAX_FRAME_EVALUATIONS)",
                limits.max_evaluations, limits.max_frame_evaluations
            ),
        });
    }
    Ok(())
}

// ---- the job -----------------------------------------------------------------------------------

/// The `analysis_runs` columns the job reads.
struct RunRow {
    org_id: Uuid,
    status: String,
    definition: Value,
    published_profile_version: String,
    baselines_version: String,
    catalog_version: String,
    seed: i64,
    job_id: Option<Uuid>,
}

/// Job params: `{"run_id": "<uuid>", "rerun_of": "<uuid>"?}`.
fn param_uuid(job: &Job, key: &str) -> Result<Option<Uuid>, JobError> {
    match job.params.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v
            .as_str()
            .and_then(|s| Uuid::parse_str(s).ok())
            .map(Some)
            .ok_or_else(|| JobError::Permanent(format!("analysis job params: bad {key}"))),
    }
}

/// Runs one `analysis_run` job; returns the job's `result`.
pub async fn run_analysis(ctx: &JobContext, job: &Job) -> Result<Value, JobError> {
    let run_id = param_uuid(job, "run_id")?
        .ok_or_else(|| JobError::Permanent("analysis job params: missing run_id".into()))?;
    let result = execute(ctx, job, run_id).await;
    if let Err(error) = &result {
        record_end(ctx, job, run_id, error).await?;
    }
    result
}

/// Records a run that did not succeed: `cancelled`, `failed` (final error) or back to
/// `queued` (the runner retries the job).
async fn record_end(
    ctx: &JobContext,
    job: &Job,
    run_id: Uuid,
    error: &JobError,
) -> Result<(), JobError> {
    let status = match error {
        JobError::Cancelled(_) => "cancelled",
        e if e.is_final_for(KIND_ANALYSIS, job.attempts) => "failed",
        _ => "queued",
    };
    let message = (status == "failed").then(|| error.message().to_owned());
    let mut tx = ctx.pool.begin().await?;
    let org = sqlx::query_scalar!(
        "UPDATE analysis_runs
         SET status = $2, error = $3,
             finished_at = CASE WHEN $2 = 'queued' THEN NULL ELSE now() END
         WHERE id = $1 AND status IN ('queued', 'running')
         RETURNING org_id",
        run_id,
        status,
        message,
    )
    .fetch_optional(&mut *tx)
    .await?;
    if let (Some(org), true) = (org, status != "queued") {
        AuditEvent::system("analysis.run_complete", "analysis_run", run_id)
            .org(org)
            .after(json!({"status": status, "job_id": job.id, "error": message}))
            .record(&mut tx)
            .await?;
    }
    tx.commit().await?;
    Ok(())
}

fn permanent(what: &str) -> impl Fn(serde_json::Error) -> JobError + '_ {
    move |e| JobError::Permanent(format!("{what}: {e}"))
}

/// Everything a run evaluates with, decoded once.
struct Inputs {
    published: AnalysisProfile,
    published_baselines: NormativeBaselineSet,
    draft: AnalysisProfile,
    draft_baselines: NormativeBaselineSet,
    catalog: StimulusCatalog,
    /// Changed registry ids and what applying each needs (RPX-3).
    changed: Vec<(String, Reprocess)>,
}

async fn load_doc(
    conn: &mut PgConnection,
    kind: ConfigDocumentKind,
    version: &str,
) -> Result<Value, JobError> {
    document(conn, kind, version).await?.ok_or_else(|| {
        JobError::Permanent(format!(
            "the {} document {version} named by the run is gone",
            kind_str(kind)
        ))
    })
}

async fn load_inputs(
    conn: &mut PgConnection,
    run: &RunRow,
    definition: &Value,
) -> Result<Inputs, JobError> {
    let published_body = load_doc(
        conn,
        ConfigDocumentKind::ScoringProfile,
        &run.published_profile_version,
    )
    .await?;
    let published: AnalysisProfile =
        serde_json::from_value(published_body).map_err(permanent("published profile"))?;
    let (_, published_baselines_body) =
        published_document(conn, ConfigDocumentKind::NormativeBaselines).await?;
    let published_baselines: NormativeBaselineSet =
        serde_json::from_value(published_baselines_body)
            .map_err(permanent("published baselines"))?;
    let draft: AnalysisProfile = serde_json::from_value(definition["profile"].clone())
        .map_err(permanent("experiment profile"))?;
    let draft_baselines: NormativeBaselineSet = serde_json::from_value(
        load_doc(
            conn,
            ConfigDocumentKind::NormativeBaselines,
            &run.baselines_version,
        )
        .await?,
    )
    .map_err(permanent("experiment baselines"))?;
    let catalog: StimulusCatalog = serde_json::from_value(
        load_doc(
            conn,
            ConfigDocumentKind::StimulusCatalog,
            &run.catalog_version,
        )
        .await?,
    )
    .map_err(permanent("stimulus catalog"))?;
    let changed = changed_params(&draft, &published)
        .into_iter()
        .filter_map(|id| params::spec(&id).map(|s| (id, s.reprocess)))
        .collect();
    Ok(Inputs {
        published,
        published_baselines,
        draft,
        draft_baselines,
        catalog,
        changed,
    })
}

/// Whether a knob whose registry `reprocess` is `r` is applied at `level` (RPX-3).
pub fn applies(r: Reprocess, level: Level) -> bool {
    match r {
        Reprocess::Score => true,
        Reprocess::Metrics | Reprocess::Frames => level == Level::Frames,
        Reprocess::CaptureOnly => false,
    }
}

async fn stop_requested(ctx: &JobContext, deadline: Instant) -> Result<(), JobError> {
    if ctx.cancelled().await? {
        return Err(JobError::Cancelled("cancelled on request".into()));
    }
    if Instant::now() >= deadline {
        return Err(JobError::Permanent(BUDGET_EXCEEDED.into()));
    }
    Ok(())
}

async fn execute(ctx: &JobContext, job: &Job, run_id: Uuid) -> Result<Value, JobError> {
    let (Some(blobs), Some(keyring), Some(config)) = (&ctx.blobs, &ctx.keyring, &ctx.config) else {
        return Err(JobError::Permanent(
            "analysis runs need the blob store, ASD_DATA_KEK and the configuration".into(),
        ));
    };
    let limits = &config.analysis;
    let deadline = Instant::now() + limits.max_duration;

    let run = sqlx::query_as!(
        RunRow,
        "SELECT org_id, status, definition, published_profile_version, baselines_version,
                catalog_version, seed, job_id
         FROM analysis_runs WHERE id = $1",
        run_id
    )
    .fetch_optional(&ctx.pool)
    .await?
    .ok_or_else(|| JobError::Permanent("the analysis run is gone".into()))?;
    if run.job_id != Some(job.id) {
        return Err(JobError::Permanent("the run belongs to another job".into()));
    }
    match run.status.as_str() {
        "succeeded" => return Ok(json!({"run_id": run_id, "already": "succeeded"})),
        "cancelled" => return Err(JobError::Cancelled("cancelled before it started".into())),
        "failed" => return Err(JobError::Permanent("the run already failed".into())),
        _ => {}
    }
    let org = run.org_id;
    sqlx::query!(
        "UPDATE analysis_runs SET status = 'running', started_at = now(), error = NULL
         WHERE id = $1 AND status IN ('queued', 'running')",
        run_id
    )
    .execute(&ctx.pool)
    .await?;
    stop_requested(ctx, deadline).await?;
    ctx.progress(0, 0, "membership").await?;

    let def: asd_contract::research::AnalysisDefinition =
        serde_json::from_value(run.definition.clone()).map_err(permanent("run definition"))?;

    // (1) Membership in one read-only snapshot, and the documents.
    let mut tx = ctx.pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let inputs = load_inputs(&mut tx, &run, &run.definition).await?;
    let members = membership(
        &mut tx,
        org,
        &def.cohort,
        &inputs.draft.age_bands,
        core_level(def.level),
        config.allow_synthetic,
    )
    .await?;
    let previous: Option<Vec<String>> = match param_uuid(job, "rerun_of")? {
        Some(old) => sqlx::query_scalar!(
            r#"SELECT j.result -> 'membership' AS "m"
               FROM analysis_runs r JOIN jobs j ON j.id = r.job_id
               WHERE r.id = $1 AND r.org_id = $2"#,
            old,
            org
        )
        .fetch_optional(&mut *tx)
        .await?
        .flatten()
        .and_then(|m| serde_json::from_value(m).ok()),
        None => None,
    };
    tx.commit().await?;
    sqlx::query!(
        "UPDATE analysis_runs SET membership_hash = $2 WHERE id = $1",
        run_id,
        members.hash()
    )
    .execute(&ctx.pool)
    .await?;
    if let Err(b) = check_budget(
        limits,
        members.score_n(),
        members.frames_n(),
        1,
        def.sweep.is_some(),
    ) {
        return Err(JobError::Permanent(format!("{}: {}", b.slug, b.detail)));
    }

    // (2) Evaluate in pages.
    let mut options = AggregatorOptions {
        metrics: def.options.metrics.clone(),
        bootstrap: def.options.bootstrap,
        ..AggregatorOptions::default()
    };
    options.metrics.retain(|m| !m.is_empty());
    let mut agg = Aggregator::new(&inputs.draft.age_bands, options);
    let mut excluded = members.excluded.clone();
    let mut rows: Vec<u8> = Vec::new();
    let total = members.members.len() as u64;
    let page_size = usize::try_from(limits.page_size)
        .unwrap_or(usize::MAX)
        .max(1);
    let mut catalogs: HashMap<String, Option<StimulusCatalog>> = HashMap::new();
    let mut done = 0u64;
    for page in members.members.chunks(page_size) {
        stop_requested(ctx, deadline).await?;
        ctx.progress(done, total, "evaluate").await?;
        let ids: Vec<String> = page.iter().map(|m| m.id.clone()).collect();
        let stored: HashMap<String, (Value, f64)> = sqlx::query!(
            "SELECT id, summary, valid_frame_ratio FROM sessions
             WHERE org_id = $1 AND id = ANY($2::text[])",
            org,
            &ids
        )
        .fetch_all(&ctx.pool)
        .await?
        .into_iter()
        .map(|r| (r.id, (r.summary, r.valid_frame_ratio)))
        .collect();
        for member in page {
            done += 1;
            let Some((summary, valid)) = stored.get(&member.id) else {
                *excluded.entry(reasons::SESSION_GONE.into()).or_insert(0) += 1;
                continue;
            };
            let research = match &member.log {
                Some(log) => {
                    match research_input(ctx, blobs.as_ref(), keyring, log, &inputs, &mut catalogs)
                        .await
                    {
                        Some(r) => Some(r),
                        None => {
                            *excluded
                                .entry(reasons::FRAME_LOG_UNREADABLE.into())
                                .or_insert(0) += 1;
                            continue;
                        }
                    }
                }
                None => None,
            };
            let input = session_input(member, summary, *valid, research);
            let level = match member.level {
                Level::Frames => LevelRequest::Frames,
                Level::Score => LevelRequest::Score,
            };
            let published = evaluate(
                &input,
                &inputs.published,
                &inputs.published_baselines,
                level,
            );
            let draft = evaluate(&input, &inputs.draft, &inputs.draft_baselines, level);
            let row = session_row(member, &published, &draft, &inputs.changed);
            agg.push(&row);
            append_row(&mut rows, member, &row)?;
            if rows.len() > limits.max_result_bytes {
                return Err(JobError::Permanent(
                    asd_contract::problem::RESULT_TOO_LARGE.into(),
                ));
            }
        }
    }
    for (reason, n) in &excluded {
        for _ in 0..*n {
            agg.exclude(reason);
        }
    }

    // (3) Seal and store, after the last cancel check.
    stop_requested(ctx, deadline).await?;
    ctx.progress(done, total, "seal").await?;
    let sealed = seal_result(keyring, org, run_id, &rows).await?;
    if sealed.len() > limits.max_result_bytes {
        return Err(JobError::Permanent(
            asd_contract::problem::RESULT_TOO_LARGE.into(),
        ));
    }
    let key = result_key(org, run_id);
    let result_bytes = i32::try_from(sealed.len())
        .map_err(|_| JobError::Permanent(asd_contract::problem::RESULT_TOO_LARGE.into()))?;
    blobs
        .put_object_large(&key, RESULT_CONTENT_TYPE, Bytes::from(sealed))
        .await
        .map_err(|e| JobError::Transient(format!("storing the analysis result: {e}")))?;

    // (4) Summary and record.
    let mut summary = agg.finish(run.seed as u64);
    summary.includes_synthetic = members.includes_synthetic;
    let k = org_small_cell_k(&ctx.pool, org).await?;
    suppress(&mut summary, k);
    let counts = summary.counts.clone();
    let summary_json = serde_json::to_value(&summary).map_err(permanent("summary"))?;
    let excluded_json = serde_json::to_value(&counts.excluded).map_err(permanent("excluded"))?;
    let progress = json!({"done": done, "total": total, "phase": "done"});
    let drift = previous.map(|old| {
        let old: std::collections::BTreeSet<&String> = old.iter().collect();
        let new: std::collections::BTreeSet<&String> = members.ids.iter().collect();
        json!({
            "added": new.difference(&old).count(),
            "removed": old.difference(&new).count(),
        })
    });

    let mut tx = ctx.pool.begin().await?;
    let updated = sqlx::query!(
        "UPDATE analysis_runs
         SET status = 'succeeded', summary = $2, sessions_n = $3, sessions_frames_n = $4,
             sessions_score_n = $5, excluded = $6, result_blob_key = $7, result_bytes = $8,
             progress = $9, finished_at = now(), error = NULL
         WHERE id = $1 AND status = 'running'",
        run_id,
        summary_json,
        i32::try_from(counts.sessions).unwrap_or(i32::MAX),
        i32::try_from(counts.by_level.frames).unwrap_or(i32::MAX),
        i32::try_from(counts.by_level.score).unwrap_or(i32::MAX),
        excluded_json,
        key,
        result_bytes,
        progress,
    )
    .execute(&mut *tx)
    .await?;
    if updated.rows_affected() != 1 {
        return Err(JobError::Permanent(
            "the run changed status while it ran".into(),
        ));
    }
    AuditEvent::system("analysis.run_complete", "analysis_run", run_id)
        .org(org)
        .after(json!({
            "status": "succeeded",
            "job_id": job.id,
            "sessions": counts.sessions,
            "by_level": {"frames": counts.by_level.frames, "score": counts.by_level.score},
            "excluded": counts.excluded,
            "result_bytes": result_bytes,
        }))
        .record(&mut tx)
        .await?;
    tx.commit().await?;
    ctx.progress(done, total, "done").await?;

    let mut result = json!({
        "run_id": run_id,
        "sessions_n": counts.sessions,
        "membership": members.ids,
    });
    if let Some(d) = drift {
        result["membership_drift"] = d;
    }
    Ok(result)
}

/// The research input of a frames-level member: fetched, opened, digest-checked and parsed;
/// `None` (logged) when any step fails.
async fn research_input(
    ctx: &JobContext,
    blobs: &dyn crate::blob::BlobStore,
    keyring: &Keyring,
    log: &ResearchLogRow,
    inputs: &Inputs,
    catalogs: &mut HashMap<String, Option<StimulusCatalog>>,
) -> Option<ResearchInput> {
    let bytes = match read_research_log(blobs, keyring, log).await {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!(session = %log.session_id, error = %e, "research log unreadable");
            return None;
        }
    };
    let text = String::from_utf8(bytes).ok()?;
    let parsed = match raw_log::parse(&text) {
        Ok(l) => l,
        Err(e) => {
            tracing::warn!(session = %log.session_id, error = %e, "research log does not parse");
            return None;
        }
    };
    // The catalog version the header names (RPX-1), else the run's catalog.
    let named = parsed
        .header
        .clips
        .iter()
        .find_map(|c| c.catalog_version.clone());
    let catalog = match named {
        Some(v) => {
            if !catalogs.contains_key(&v) {
                let loaded = match ctx.pool.acquire().await {
                    Ok(mut conn) => document(&mut conn, ConfigDocumentKind::StimulusCatalog, &v)
                        .await
                        .ok()
                        .flatten()
                        .and_then(|b| serde_json::from_value(b).ok()),
                    Err(_) => None,
                };
                catalogs.insert(v.clone(), loaded);
            }
            catalogs
                .get(&v)
                .cloned()
                .flatten()
                .unwrap_or_else(|| inputs.catalog.clone())
        }
        None => inputs.catalog.clone(),
    };
    Some(ResearchInput {
        log: parsed,
        catalog,
        options: ReprocessOptions::default(),
    })
}

/// The stored session as the evaluation reads it (RPX-2).
fn session_input(
    member: &Member,
    summary: &Value,
    valid: f64,
    research: Option<ResearchInput>,
) -> SessionInput {
    let per_video = summary.get("perVideo");
    let reports: Vec<PerVideoScoreReport> = per_video
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();
    SessionInput {
        session_id: member.id.clone(),
        age_months: member.age_months.map(i64::from),
        reports,
        has_reports: per_video.is_some(),
        valid_frame_ratio: Some(valid),
        tracking: summary
            .get("trackingQuality")
            .and_then(|t| serde_json::from_value(t.clone()).ok()),
        // Not part of any result row; empty keeps the evaluation independent of the clock.
        scored_at_utc: String::new(),
        research,
    }
}

/// One aggregated row: both sides, the draft side's warnings plus `param_not_applied` for
/// every changed knob the level cannot apply (RPX-3), each id once.
fn session_row(
    member: &Member,
    published: &Evaluation,
    draft: &Evaluation,
    changed: &[(String, Reprocess)],
) -> SessionRow {
    let mut warnings = draft.warnings.clone();
    for (id, r) in changed {
        let w = format!("param_not_applied: {id}");
        if !applies(*r, draft.level) && !warnings.contains(&w) {
            warnings.push(w);
        }
    }
    SessionRow {
        level: member.level,
        age_months: member.age_months.map(i64::from),
        published: RowSide::from_evaluation(published),
        draft: RowSide::from_evaluation(draft),
        warnings,
    }
}

/// Appends the `AnalysisSessionRow` JSON line of `row` (RUN-3, PRV-2: no token, MRN, name
/// or device model).
fn append_row(out: &mut Vec<u8>, member: &Member, row: &SessionRow) -> Result<(), JobError> {
    let delta = |p: Option<f64>, d: Option<f64>| match (p, d) {
        (Some(p), Some(d)) => Some(d - p),
        _ => None,
    };
    let metrics: BTreeMap<&String, Option<f64>> = row
        .draft
        .metrics
        .iter()
        .map(|(k, d)| {
            (
                k,
                delta(row.published.metrics.get(k).copied().flatten(), *d),
            )
        })
        .collect();
    let line = json!({
        "session_id": member.id,
        "level": row.level,
        "age_months": member.age_months,
        "age_band": member.band,
        "device_id": member.device_id,
        "published": row.published,
        "draft": row.draft,
        "delta": {
            "composite": delta(row.published.composite, row.draft.composite),
            "metrics": metrics,
        },
        "warnings": row.warnings,
    });
    serde_json::to_writer(&mut *out, &line).map_err(permanent("result row"))?;
    out.push(b'\n');
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budget_follows_the_limits() {
        let l = AnalysisLimits::default();
        assert!(check_budget(&l, 2000, 0, 1, false).is_ok());
        assert_eq!(
            check_budget(&l, 2001, 0, 1, false).unwrap_err().slug,
            "cohort-too-large"
        );
        assert!(check_budget(&l, 0, 500, 1, false).is_ok());
        assert_eq!(
            check_budget(&l, 0, 501, 1, false).unwrap_err().slug,
            "cohort-too-large"
        );
        // 625 cells x 400 sessions = 250 000 > 200 000.
        assert_eq!(
            check_budget(&l, 400, 0, 625, true).unwrap_err().slug,
            "sweep-too-large"
        );
        assert!(check_budget(&l, 320, 0, 625, true).is_ok());
        assert_eq!(
            check_budget(&l, 0, 40, 625, true).unwrap_err().slug,
            "sweep-too-large"
        );
    }

    #[test]
    fn knobs_apply_by_level() {
        assert!(applies(Reprocess::Score, Level::Score));
        assert!(!applies(Reprocess::Metrics, Level::Score));
        assert!(!applies(Reprocess::Frames, Level::Score));
        assert!(applies(Reprocess::Frames, Level::Frames));
        assert!(applies(Reprocess::Metrics, Level::Frames));
        assert!(!applies(Reprocess::CaptureOnly, Level::Frames));
    }

    #[test]
    fn membership_hash_is_over_sorted_ids() {
        let m = Membership {
            ids: vec!["a".into(), "b".into()],
            ..Membership::default()
        };
        let expected = Sha256::digest(b"a\nb\n").to_vec();
        assert_eq!(m.hash(), expected);
        assert_eq!(result_key(Uuid::nil(), Uuid::nil()).matches('/').count(), 3);
    }
}
