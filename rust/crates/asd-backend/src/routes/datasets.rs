//! owner: WP-X.11
//!
//! Research datasets (ADR-0015 R-7..R-9; design note
//! `docs/spec/modules/asd-research-toolkit/datasets.md`):
//!
//! - `POST /v1/research/datasets` (`ExperimentWrite`): freezes a dataset from a cohort of the
//!   caller's org. In one `REPEATABLE READ` transaction: the cohort's sessions through the
//!   analysis membership query (research consent, not withdrawn, real capture, the cohort's
//!   filters; ages placed in the published profile's bands), the exclusion rules in order
//!   (a session without a placeable age first, `no_age`; then `quality_gates` and
//!   `explicit_list`; the first rule that matches names the exclusion), the seeded split
//!   stratified by age band ([`assign_split`]), labels from the signed clinical review's
//!   disposition when `labels: review_outcomes`, lineage (cohort hash, published profile hash
//!   and versions, code version, parent dataset, run), the data dictionary, and the
//!   [`content_hash`]. Members are inserted once and never updated (triggers of migration
//!   0029) except the erasure marking. `400 cohort-empty` / `cohort-too-large`
//!   (> [`MAX_DATASET_SESSIONS`]), `409 conflict` on a taken name. Audit `dataset.create`.
//! - `GET /v1/research/datasets?status&cursor&limit` (`AnalysisRead`): newest first. A
//!   superadmin names the org with `?org_id=`.
//! - `GET /v1/research/datasets/{id}` (`AnalysisRead`).
//! - `GET /v1/research/datasets/{id}/members?split&cursor&limit` (`AnalysisRead` and
//!   `SessionRead`, like run rows, PRV-2): by session id; an erased member shows only its id
//!   and `excluded: erased`. Audit `dataset.members_read` once per page.
//! - `POST /v1/research/datasets/{id}/archive` (`ExperimentWrite`): `409 invalid-transition`
//!   the second time. Audit `dataset.archive`.
//!
//! Analysis runs take `dataset_id` instead of a cohort ([`run_source`], [`snapshot_ids`]):
//! the run reads the frozen members (one erased or withdrawn since is excluded
//! `session_gone`), stores `dataset_id` and `dataset_hash`.
//!
//! Pseudonyms: `P-` (participant) and `R-` (reviewer) are the first hex digits of
//! sha256(salt, kind, id) with a random salt drawn for the freeze and never stored, so they
//! are stable within one dataset and cannot be linked across datasets or recomputed.
//!
//! Not yet applied (schema only): `tukey` and `mad_z` rules (WP-X.22), `labels: import`
//! (WP-X.16); both are `400 validation-failed`.

use std::collections::{BTreeMap, BTreeSet};

use asd_contract::api_v1::ConfigDocumentKind;
use asd_contract::datasets::{
    assign_split, check_fractions, content_hash, data_dictionary, reasons, split_counts, Column,
    CreateDatasetRequest, Dataset, DatasetMember, DatasetMemberPage, DatasetPage, DatasetRef,
    DatasetStatus, Exclusion, ExclusionRule, Label, LabelSource, LabelSourceKind, Lineage, Split,
    SplitName, SplitStratumCount, EXCLUDED_ERASED, MAX_DATASET_SESSIONS, MAX_EXCLUSION_RULES,
    MAX_EXPLICIT_SESSIONS, MAX_REASON_CHARS, MAX_RULE_ID_CHARS,
};
use asd_contract::problem as slugs;
use asd_contract::research::{CohortDefinition, MAX_DESCRIPTION_CHARS, MAX_NAME_CHARS};
use asd_core::analysis::{AnalysisProfile, LevelRequest};
use axum::extract::rejection::QueryRejection;
use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use http::StatusCode;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::PgConnection;
use uuid::Uuid;

use crate::app::AppState;
use crate::audit::{record_read, AuditEvent};
use crate::auth::extractors::{ApiJson, RequestMeta, Require, UserPrincipal};
use crate::auth::rbac::{self, perm, Action};
use crate::config_docs::lifecycle::body_hash;
use crate::error::{AppError, AppResult};
use crate::jobs::analysis as job;
use crate::problem::InvalidParam;
use crate::read::cursor::{self, Cursor};
use crate::routes::sessions::query_or_problem;

/// `lineage.code_version` of datasets frozen by this build.
const CODE_VERSION: &str = concat!("asd-backend/", env!("CARGO_PKG_VERSION"));

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/research/datasets",
            post(create_dataset).get(list_datasets),
        )
        .route("/research/datasets/{id}", get(get_dataset))
        .route("/research/datasets/{id}/members", get(list_members))
        .route("/research/datasets/{id}/archive", post(archive_dataset))
}

// ---- helpers ---------------------------------------------------------------------------------

fn invalid(name: &str, reason: impl Into<String>) -> InvalidParam {
    InvalidParam {
        name: name.into(),
        reason: reason.into(),
    }
}

fn path_uuid(raw: &str) -> AppResult<Uuid> {
    Uuid::parse_str(raw).map_err(|_| AppError::NotFound)
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

fn unhex(s: &str) -> AppResult<Vec<u8>> {
    (0..s.len())
        .step_by(2)
        .map(|i| {
            s.get(i..i + 2)
                .and_then(|b| u8::from_str_radix(b, 16).ok())
                .ok_or_else(|| AppError::Internal("content hash is not hex".into()))
        })
        .collect()
}

fn decode<T: DeserializeOwned>(what: &str, v: Value) -> AppResult<T> {
    serde_json::from_value(v).map_err(|e| AppError::Internal(format!("stored {what}: {e}")))
}

fn to_value<T: serde::Serialize>(what: &str, v: &T) -> AppResult<Value> {
    serde_json::to_value(v).map_err(|e| AppError::Internal(format!("{what}: {e}")))
}

/// The org a list call reads: the caller's, or `?org_id=` for the vendor.
fn caller_org(principal: &UserPrincipal, requested: Option<&str>) -> AppResult<Uuid> {
    let requested = match requested.filter(|s| !s.is_empty()) {
        Some(raw) => Some(Uuid::parse_str(raw).map_err(|_| AppError::NotFound)?),
        None => None,
    };
    match (principal.org_id, requested) {
        (Some(own), None) => Ok(own),
        (_, Some(org)) => {
            principal.ensure_org(org)?;
            Ok(org)
        }
        (None, None) => Err(AppError::Validation(vec![invalid(
            "org_id",
            "required for a caller without an org",
        )])),
    }
}

/// A short pseudonym: `prefix` and the first `len` hex digits of sha256(salt, kind, id).
fn pseudonym(salt: &[u8; 32], prefix: &str, id: &str, len: usize) -> String {
    let mut h = Sha256::new();
    h.update(salt);
    h.update(prefix.as_bytes());
    h.update(b":");
    h.update(id.as_bytes());
    let digest = h.finalize();
    format!("{prefix}-{}", &hex(&digest)[..len])
}

fn fresh_salt() -> [u8; 32] {
    let a = Uuid::new_v4().into_bytes();
    let b = Uuid::new_v4().into_bytes();
    let mut out = [0u8; 32];
    out[..16].copy_from_slice(&a);
    out[16..].copy_from_slice(&b);
    out
}

fn split_str(s: SplitName) -> &'static str {
    match s {
        SplitName::Train => "train",
        SplitName::Validation => "validation",
        SplitName::Holdout => "holdout",
    }
}

fn parse_split(s: &str) -> Option<SplitName> {
    serde_json::from_value(Value::from(s)).ok()
}

// ---- rows ------------------------------------------------------------------------------------

/// A `datasets` row.
struct DatasetRow {
    id: Uuid,
    org_id: Uuid,
    name: String,
    description: Option<String>,
    status: String,
    cohort_id: Option<Uuid>,
    frozen_at: DateTime<Utc>,
    content_hash: Vec<u8>,
    member_count: i32,
    erased_count: i32,
    exclusion_rules: Value,
    exclusions: Value,
    label_source: Option<Value>,
    split: Option<Value>,
    split_counts: Value,
    lineage: Value,
    data_dictionary: Value,
    created_by: Uuid,
    created_at: DateTime<Utc>,
    archived_at: Option<DateTime<Utc>>,
}

impl DatasetRow {
    fn dto(&self) -> AppResult<Dataset> {
        let count = |n: i32| u32::try_from(n).unwrap_or(0);
        Ok(Dataset {
            id: self.id.to_string(),
            org_id: self.org_id.to_string(),
            name: self.name.clone(),
            description: self.description.clone(),
            status: decode("dataset status", Value::from(self.status.as_str()))?,
            cohort_id: self.cohort_id.map(|u| u.to_string()),
            frozen_at: cursor::ts(self.frozen_at),
            content_hash: hex(&self.content_hash),
            member_count: count(self.member_count),
            erased_count: count(self.erased_count),
            exclusion_rules: decode("exclusion rules", self.exclusion_rules.clone())?,
            exclusions: decode("exclusions", self.exclusions.clone())?,
            label_source: self
                .label_source
                .clone()
                .map(|v| decode::<LabelSource>("label source", v))
                .transpose()?,
            split: self
                .split
                .clone()
                .map(|v| decode::<Split>("split", v))
                .transpose()?,
            split_counts: decode::<Vec<SplitStratumCount>>(
                "split counts",
                self.split_counts.clone(),
            )?,
            lineage: decode::<Lineage>("lineage", self.lineage.clone())?,
            data_dictionary: decode::<Vec<Column>>(
                "data dictionary",
                self.data_dictionary.clone(),
            )?,
            created_by: Some(self.created_by.to_string()),
            created_at: cursor::ts(self.created_at),
            archived_at: self.archived_at.map(cursor::ts),
        })
    }
}

async fn load_dataset(
    conn: &mut PgConnection,
    id: Uuid,
    for_update: bool,
) -> AppResult<DatasetRow> {
    let row = if for_update {
        sqlx::query_as!(
            DatasetRow,
            "SELECT id, org_id, name, description, status, cohort_id,
                    frozen_at, content_hash, member_count, erased_count, exclusion_rules,
                    exclusions, label_source, split, split_counts, lineage, data_dictionary,
                    created_by, created_at, archived_at
             FROM datasets WHERE id = $1 FOR UPDATE",
            id
        )
        .fetch_optional(&mut *conn)
        .await?
    } else {
        sqlx::query_as!(
            DatasetRow,
            "SELECT id, org_id, name, description, status, cohort_id,
                    frozen_at, content_hash, member_count, erased_count, exclusion_rules,
                    exclusions, label_source, split, split_counts, lineage, data_dictionary,
                    created_by, created_at, archived_at
             FROM datasets WHERE id = $1",
            id
        )
        .fetch_optional(&mut *conn)
        .await?
    };
    row.ok_or(AppError::NotFound)
}

// ---- runs from a dataset -----------------------------------------------------------------------

/// What an analysis run takes from a dataset.
pub struct RunSource {
    pub reference: DatasetRef,
    pub hash: Vec<u8>,
    pub cohort_id: Option<Uuid>,
    /// The cohort definition used at freeze time (a record in the run definition).
    pub cohort: CohortDefinition,
}

/// The dataset `id` of `org` as a run source; `400` when it is not the org's or archived.
pub async fn run_source(conn: &mut PgConnection, org: Uuid, id: Uuid) -> AppResult<RunSource> {
    let row = sqlx::query!(
        "SELECT org_id, status, cohort_id, cohort_definition, content_hash
         FROM datasets WHERE id = $1",
        id
    )
    .fetch_optional(&mut *conn)
    .await?
    .filter(|d| d.org_id == org)
    .ok_or_else(|| {
        AppError::Validation(vec![invalid("dataset_id", "no such dataset in this org")])
    })?;
    if row.status != "frozen" {
        return Err(AppError::Validation(vec![invalid(
            "dataset_id",
            "the dataset is archived",
        )]));
    }
    Ok(RunSource {
        reference: DatasetRef {
            id: id.to_string(),
            content_hash: hex(&row.content_hash),
        },
        hash: row.content_hash,
        cohort_id: row.cohort_id,
        cohort: decode("dataset cohort", row.cohort_definition)?,
    })
}

/// The members of dataset `id` a run reads: every session included at freeze time, sorted
/// (one erased since is not found and counts `session_gone` in the run). Empty when the
/// dataset is not `org`'s.
pub async fn snapshot_ids(conn: &mut PgConnection, org: Uuid, id: Uuid) -> AppResult<Vec<String>> {
    Ok(sqlx::query_scalar!(
        "SELECT m.session_id FROM dataset_members m JOIN datasets d ON d.id = m.dataset_id
         WHERE m.dataset_id = $1 AND d.org_id = $2
         ORDER BY m.session_id",
        id,
        org
    )
    .fetch_all(&mut *conn)
    .await?)
}

/// The definition a snapshot membership runs with: no filter but the synthetic switch (the
/// members are fixed; research consent and withdrawal still apply in the query).
pub fn snapshot_definition(cohort: &CohortDefinition) -> CohortDefinition {
    CohortDefinition {
        include_synthetic: cohort.include_synthetic,
        ..CohortDefinition::default()
    }
}

// ---- create ----------------------------------------------------------------------------------

/// Shape checks of a create request that need no data.
fn check_request(
    req: &CreateDatasetRequest,
) -> Result<(String, Option<String>), Vec<InvalidParam>> {
    let mut errors = Vec::new();
    let name = req.name.trim().to_owned();
    let n = name.chars().count();
    if n == 0 || n > MAX_NAME_CHARS || name.chars().any(char::is_control) {
        errors.push(invalid(
            "name",
            format!("must be 1 to {MAX_NAME_CHARS} characters without control characters"),
        ));
    }
    let description = req
        .description
        .as_deref()
        .map(str::trim)
        .filter(|d| !d.is_empty())
        .map(str::to_owned);
    if description
        .as_deref()
        .is_some_and(|d| d.chars().count() > MAX_DESCRIPTION_CHARS)
    {
        errors.push(invalid(
            "description",
            format!("must be at most {MAX_DESCRIPTION_CHARS} characters"),
        ));
    }
    if req.exclusion_rules.len() > MAX_EXCLUSION_RULES {
        errors.push(invalid(
            "exclusion_rules",
            format!("at most {MAX_EXCLUSION_RULES} rules"),
        ));
    }
    let mut ids = BTreeSet::new();
    for (i, rule) in req.exclusion_rules.iter().enumerate() {
        let at = |f: &str| format!("exclusion_rules[{i}].{f}");
        let id = rule.id();
        if id.trim().is_empty()
            || id.chars().count() > MAX_RULE_ID_CHARS
            || id.chars().any(char::is_control)
            || !ids.insert(id)
        {
            errors.push(invalid(
                &at("id"),
                format!("must be a distinct id of 1 to {MAX_RULE_ID_CHARS} characters"),
            ));
        }
        match rule {
            ExclusionRule::QualityGates {
                min_valid_ratio, ..
            } => {
                if min_valid_ratio.is_some_and(|r| !(r.is_finite() && (0.0..=1.0).contains(&r))) {
                    errors.push(invalid(&at("min_valid_ratio"), "must be in [0, 1]"));
                }
            }
            ExclusionRule::Tukey { .. } | ExclusionRule::MadZ { .. } => errors.push(invalid(
                &at("kind"),
                "metric outlier rules are applied from WP-X.22; use quality_gates or explicit_list",
            )),
            ExclusionRule::ExplicitList { sessions, .. } => {
                if sessions.is_empty() || sessions.len() > MAX_EXPLICIT_SESSIONS {
                    errors.push(invalid(
                        &at("sessions"),
                        format!("must name 1 to {MAX_EXPLICIT_SESSIONS} sessions"),
                    ));
                }
                let mut seen = BTreeSet::new();
                for (j, s) in sessions.iter().enumerate() {
                    if s.session_id.trim().is_empty() || !seen.insert(&s.session_id) {
                        errors.push(invalid(
                            &format!("exclusion_rules[{i}].sessions[{j}].session_id"),
                            "must be a distinct session id",
                        ));
                    }
                    let r = s.reason.trim();
                    if r.is_empty()
                        || r.chars().count() > MAX_REASON_CHARS
                        || r.chars().any(char::is_control)
                    {
                        errors.push(invalid(
                            &format!("exclusion_rules[{i}].sessions[{j}].reason"),
                            format!("must be 1 to {MAX_REASON_CHARS} characters"),
                        ));
                    }
                }
            }
        }
    }
    if req.labels == Some(LabelSourceKind::Import) {
        errors.push(invalid(
            "labels",
            "label import arrives with WP-X.16; use review_outcomes",
        ));
    }
    if let Some(split) = &req.split {
        if let Err(e) = check_fractions(&split.fractions) {
            errors.push(invalid("split.fractions", e));
        }
    }
    if errors.is_empty() {
        Ok((name, description))
    } else {
        Err(errors)
    }
}

/// What the quality gates read of a session.
struct SessionFacts {
    participant_id: Uuid,
    valid_frame_ratio: Option<f64>,
    quality_verdict: Option<String>,
    tracking_verdict: Option<String>,
    clips: i64,
}

fn wire<T: serde::Serialize>(v: &T) -> String {
    match serde_json::to_value(v) {
        Ok(Value::String(s)) => s,
        _ => String::new(),
    }
}

/// The first rule that excludes `session`, as `(rule id, reason)`.
fn excluded_by(
    rules: &[ExclusionRule],
    session: &str,
    f: &SessionFacts,
) -> Option<(String, String)> {
    for rule in rules {
        match rule {
            ExclusionRule::QualityGates {
                id,
                min_valid_ratio,
                exclude_quality_verdicts,
                exclude_tracking_verdicts,
                min_clips_completed,
            } => {
                let reason = if min_valid_ratio
                    .is_some_and(|min| f.valid_frame_ratio.is_none_or(|v| v < min))
                {
                    Some(reasons::VALID_RATIO)
                } else if f
                    .quality_verdict
                    .as_deref()
                    .is_some_and(|q| exclude_quality_verdicts.iter().any(|v| wire(v) == q))
                {
                    Some(reasons::QUALITY_VERDICT)
                } else if f
                    .tracking_verdict
                    .as_deref()
                    .is_some_and(|t| exclude_tracking_verdicts.iter().any(|v| wire(v) == t))
                {
                    Some(reasons::TRACKING_VERDICT)
                } else if min_clips_completed.is_some_and(|min| f.clips < i64::from(min)) {
                    Some(reasons::CLIPS)
                } else {
                    None
                };
                if let Some(r) = reason {
                    return Some((id.clone(), r.to_owned()));
                }
            }
            ExclusionRule::ExplicitList { id, sessions } => {
                if let Some(s) = sessions.iter().find(|s| s.session_id == session) {
                    return Some((id.clone(), s.reason.trim().to_owned()));
                }
            }
            // Refused by `check_request`.
            ExclusionRule::Tukey { .. } | ExclusionRule::MadZ { .. } => {}
        }
    }
    None
}

async fn create_dataset(
    auth: Require<perm::ExperimentWrite>,
    meta: RequestMeta,
    State(state): State<AppState>,
    ApiJson(req): ApiJson<CreateDatasetRequest>,
) -> AppResult<(StatusCode, Json<Dataset>)> {
    let org = auth.principal.org_id.ok_or(AppError::Forbidden)?;
    let (name, description) = check_request(&req).map_err(AppError::Validation)?;
    let cohort_id = Uuid::parse_str(&req.cohort_id)
        .map_err(|_| AppError::Validation(vec![invalid("cohort_id", "must be a cohort id")]))?;
    let uuid_field = |name: &str, raw: &Option<String>| -> AppResult<Option<Uuid>> {
        raw.as_deref()
            .map(|r| {
                Uuid::parse_str(r)
                    .map_err(|_| AppError::Validation(vec![invalid(name, "must be an id (UUID)")]))
            })
            .transpose()
    };
    let parent = uuid_field("parent_dataset_id", &req.parent_dataset_id)?;
    let run = uuid_field("run_id", &req.run_id)?;

    let mut tx = state.db.begin().await?;
    // One snapshot for the cohort, the facts, the labels and the documents.
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ")
        .execute(&mut *tx)
        .await?;
    let cohort = sqlx::query!(
        "SELECT org_id, definition, definition_hash, archived_at IS NOT NULL AS \"archived!\"
         FROM cohorts WHERE id = $1",
        cohort_id
    )
    .fetch_optional(&mut *tx)
    .await?
    .filter(|c| c.org_id == org)
    .ok_or_else(|| {
        AppError::Validation(vec![invalid("cohort_id", "no such cohort in this org")])
    })?;
    if cohort.archived {
        return Err(AppError::Validation(vec![invalid(
            "cohort_id",
            "the cohort is archived",
        )]));
    }
    let definition: CohortDefinition = decode("cohort", cohort.definition.clone())?;
    if let Some(p) = parent {
        let ok = sqlx::query_scalar!(
            r#"SELECT EXISTS (SELECT 1 FROM datasets WHERE id = $1 AND org_id = $2) AS "ok!""#,
            p,
            org
        )
        .fetch_one(&mut *tx)
        .await?;
        if !ok {
            return Err(AppError::Validation(vec![invalid(
                "parent_dataset_id",
                "no such dataset in this org",
            )]));
        }
    }
    if let Some(r) = run {
        let ok = sqlx::query_scalar!(
            r#"SELECT EXISTS (SELECT 1 FROM analysis_runs WHERE id = $1 AND org_id = $2) AS "ok!""#,
            r,
            org
        )
        .fetch_one(&mut *tx)
        .await?;
        if !ok {
            return Err(AppError::Validation(vec![invalid(
                "run_id",
                "no such analysis run in this org",
            )]));
        }
    }

    // The published documents: the profile's bands place the ages (COH-3).
    let (profile_version, profile_body) =
        job::published_document(&mut tx, ConfigDocumentKind::ScoringProfile).await?;
    let (baselines_version, _) =
        job::published_document(&mut tx, ConfigDocumentKind::NormativeBaselines).await?;
    let (catalog_version, _) =
        job::published_document(&mut tx, ConfigDocumentKind::StimulusCatalog).await?;
    let profile: AnalysisProfile = serde_json::from_value(profile_body.clone())
        .map_err(|e| AppError::Internal(format!("published profile: {e}")))?;

    let membership = job::membership(
        &mut tx,
        org,
        &definition,
        &profile.age_bands,
        LevelRequest::Score,
        state.config.allow_synthetic,
        None,
    )
    .await?;
    if membership.ids.is_empty() {
        return Err(AppError::BadRequest {
            slug: slugs::COHORT_EMPTY,
            detail: "the cohort selects no research-consented session".into(),
        });
    }
    if membership.ids.len() > MAX_DATASET_SESSIONS as usize {
        return Err(AppError::BadRequest {
            slug: slugs::COHORT_TOO_LARGE,
            detail: format!(
                "the cohort has {} sessions; a dataset holds at most {MAX_DATASET_SESSIONS}",
                membership.ids.len()
            ),
        });
    }
    let snapshot: BTreeSet<&str> = membership.ids.iter().map(String::as_str).collect();
    let mut unknown = Vec::new();
    for (i, rule) in req.exclusion_rules.iter().enumerate() {
        if let ExclusionRule::ExplicitList { sessions, .. } = rule {
            for (j, s) in sessions.iter().enumerate() {
                if !snapshot.contains(s.session_id.as_str()) {
                    unknown.push(invalid(
                        &format!("exclusion_rules[{i}].sessions[{j}].session_id"),
                        "not a session of the cohort",
                    ));
                }
            }
        }
    }
    if !unknown.is_empty() {
        return Err(AppError::Validation(unknown));
    }
    let facts: BTreeMap<String, SessionFacts> = sqlx::query!(
        r#"SELECT s.id, s.participant_id, s.valid_frame_ratio, s.quality_verdict,
                  s.tracking_verdict,
                  (SELECT count(*) FROM session_clip_scores c WHERE c.session_id = s.id)
                      AS "clips!"
           FROM sessions s WHERE s.org_id = $1 AND s.id = ANY($2)"#,
        org,
        &membership.ids,
    )
    .fetch_all(&mut *tx)
    .await?
    .into_iter()
    .map(|r| {
        (
            r.id,
            SessionFacts {
                participant_id: r.participant_id,
                valid_frame_ratio: Some(r.valid_frame_ratio),
                quality_verdict: Some(r.quality_verdict),
                tracking_verdict: r.tracking_verdict,
                clips: r.clips,
            },
        )
    })
    .collect();

    // Exclusions in rule order; members with their band (R-7).
    let placed: BTreeMap<&str, &job::Member> = membership
        .members
        .iter()
        .map(|m| (m.id.as_str(), m))
        .collect();
    let mut exclusions = Vec::new();
    let mut included: Vec<(&job::Member, &SessionFacts)> = Vec::new();
    for id in &membership.ids {
        let Some(f) = facts.get(id) else { continue };
        let Some(m) = placed.get(id.as_str()) else {
            exclusions.push(Exclusion {
                session_id: id.clone(),
                rule_id: None,
                reason: reasons::NO_AGE.into(),
            });
            continue;
        };
        match excluded_by(&req.exclusion_rules, id, f) {
            Some((rule, reason)) => exclusions.push(Exclusion {
                session_id: id.clone(),
                rule_id: Some(rule),
                reason,
            }),
            None => included.push((m, f)),
        }
    }

    // Split (seeded, stratified by age band).
    let split = req.split.map(|s| Split {
        seed: s.seed.unwrap_or_else(|| {
            let b = Uuid::new_v4().into_bytes();
            i64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]])
        }),
        fractions: s.fractions,
        strata: s.strata,
    });
    let parts = split.map(|s| {
        let keyed: Vec<(String, Option<String>)> = included
            .iter()
            .map(|(m, _)| (m.id.clone(), m.band.clone()))
            .collect();
        assign_split(&keyed, s.seed, &s.fractions)
    });

    // Labels from the signed clinical reviews (R-8).
    let salt = fresh_salt();
    let mut labels: BTreeMap<String, Label> = BTreeMap::new();
    let label_source = match req.labels {
        Some(LabelSourceKind::ReviewOutcomes) => {
            let ids: Vec<String> = included.iter().map(|(m, _)| m.id.clone()).collect();
            let rows = sqlx::query!(
                r#"SELECT cr.session_id, v.disposition AS "disposition!", s.signer_id,
                          s.signed_at
                   FROM clinical_reviews cr
                   JOIN clinical_review_versions v
                     ON v.review_id = cr.id AND v.status = 'signed'
                        AND v.disposition IS NOT NULL
                   JOIN clinical_signatures s ON s.version_id = v.id AND s.role = 'sign'
                   WHERE cr.org_id = $1 AND cr.session_id = ANY($2)"#,
                org,
                &ids,
            )
            .fetch_all(&mut *tx)
            .await?;
            for r in rows {
                labels.insert(
                    r.session_id,
                    Label {
                        value: r.disposition,
                        source: LabelSourceKind::ReviewOutcomes,
                        taken_at: cursor::ts(r.signed_at),
                        reviewer_pseudonym: Some(pseudonym(
                            &salt,
                            "R",
                            &r.signer_id.to_string(),
                            8,
                        )),
                    },
                );
            }
            Some(LabelSource {
                kind: LabelSourceKind::ReviewOutcomes,
                taken_at: cursor::ts(Utc::now()),
                count: u32::try_from(labels.len()).unwrap_or(u32::MAX),
            })
        }
        // `import` is refused by `check_request`.
        _ => None,
    };

    let members: Vec<DatasetMember> = included
        .iter()
        .map(|(m, f)| DatasetMember {
            session_id: m.id.clone(),
            participant_pseudonym: Some(pseudonym(&salt, "P", &f.participant_id.to_string(), 12)),
            age_months: m.age_months,
            age_band: m.band.clone(),
            split: parts.as_ref().and_then(|p| p.get(&m.id).copied()),
            label: labels.get(&m.id).cloned(),
            excluded: None,
        })
        .collect();
    let lineage = Lineage {
        cohort_id: Some(cohort_id.to_string()),
        cohort_hash: hex(&cohort.definition_hash),
        parent_dataset_id: parent.map(|p| p.to_string()),
        run_id: run.map(|r| r.to_string()),
        profile_hash: hex(&body_hash(&profile_body)),
        profile_version,
        catalog_version,
        baselines_version,
        code_version: CODE_VERSION.into(),
    };
    let label_kind = label_source.as_ref().map(|l| l.kind);
    let hash = content_hash(
        &members,
        &exclusions,
        &req.exclusion_rules,
        split.as_ref(),
        label_kind,
        &lineage,
    );
    let counts = split_counts(&members);
    let id = Uuid::new_v4();
    let member_count = i32::try_from(members.len()).unwrap_or(i32::MAX);
    let row = sqlx::query_as!(
        DatasetRow,
        "INSERT INTO datasets
           (id, org_id, name, description, cohort_id, cohort_definition, parent_dataset_id,
            run_id, content_hash, member_count, exclusion_rules, exclusions, label_source,
            split, split_counts, lineage, data_dictionary, created_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)
         RETURNING id, org_id, name, description, status, cohort_id,
                   frozen_at, content_hash, member_count, erased_count, exclusion_rules,
                   exclusions, label_source, split, split_counts, lineage, data_dictionary,
                   created_by, created_at, archived_at",
        id,
        org,
        name,
        description,
        cohort_id,
        cohort.definition,
        parent,
        run,
        unhex(&hash)?,
        member_count,
        to_value("exclusion rules", &req.exclusion_rules)?,
        to_value("exclusions", &exclusions)?,
        label_source
            .as_ref()
            .map(|l| to_value("label source", l))
            .transpose()?,
        split.as_ref().map(|s| to_value("split", s)).transpose()?,
        to_value("split counts", &counts)?,
        to_value("lineage", &lineage)?,
        to_value("data dictionary", &data_dictionary())?,
        auth.principal.user_id,
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| match &e {
        sqlx::Error::Database(d) if d.constraint() == Some("datasets_org_name_key") => {
            AppError::Conflict {
                slug: slugs::CONFLICT,
                detail: "a dataset of this org already has this name".into(),
            }
        }
        _ => e.into(),
    })?;

    let session_ids: Vec<String> = members.iter().map(|m| m.session_id.clone()).collect();
    let pseudonyms: Vec<Option<String>> = members
        .iter()
        .map(|m| m.participant_pseudonym.clone())
        .collect();
    let ages: Vec<Option<i32>> = members.iter().map(|m| m.age_months).collect();
    let bands: Vec<Option<String>> = members.iter().map(|m| m.age_band.clone()).collect();
    let splits: Vec<Option<String>> = members
        .iter()
        .map(|m| m.split.map(|s| split_str(s).to_owned()))
        .collect();
    let label_texts: Vec<Option<String>> = members
        .iter()
        .map(|m| {
            m.label
                .as_ref()
                .map(|l| to_value("label", l).map(|v| v.to_string()))
                .transpose()
        })
        .collect::<AppResult<_>>()?;
    sqlx::query!(
        "INSERT INTO dataset_members
           (dataset_id, session_id, participant_pseudonym, age_months, age_band, split, label)
         SELECT $1, s, p, a, b, sp, l::jsonb
         FROM unnest($2::text[], $3::text[], $4::int4[], $5::text[], $6::text[], $7::text[])
              AS u(s, p, a, b, sp, l)",
        id,
        &session_ids,
        &pseudonyms as &[Option<String>],
        &ages as &[Option<i32>],
        &bands as &[Option<String>],
        &splits as &[Option<String>],
        &label_texts as &[Option<String>],
    )
    .execute(&mut *tx)
    .await?;

    let mut by_rule: BTreeMap<String, u32> = BTreeMap::new();
    for e in &exclusions {
        *by_rule
            .entry(e.rule_id.clone().unwrap_or_else(|| e.reason.clone()))
            .or_insert(0) += 1;
    }
    AuditEvent::user(&auth.principal, "dataset.create", "dataset", id)
        .org(org)
        .after(json!({
            "cohort_id": cohort_id,
            "content_hash": hash,
            "members": members.len(),
            "excluded": by_rule,
            "labels": label_source.as_ref().map(|l| l.count),
            "split_seed": split.map(|s| s.seed),
            "parent_dataset_id": parent,
            "run_id": run,
        }))
        .meta(&meta)
        .record(&mut tx)
        .await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(row.dto()?)))
}

// ---- reads -------------------------------------------------------------------------------------

#[derive(Debug, Default, Deserialize)]
struct ListQuery {
    org_id: Option<String>,
    status: Option<String>,
    cursor: Option<String>,
    limit: Option<String>,
}

fn parse_limit(raw: Option<&str>) -> AppResult<i64> {
    let limit = match raw.filter(|s| !s.is_empty()) {
        Some(s) => Some(
            s.parse::<i64>()
                .map_err(|_| AppError::Validation(vec![invalid("limit", "must be an integer")]))?,
        ),
        None => None,
    };
    cursor::limit(limit)
}

async fn list_datasets(
    auth: Require<perm::AnalysisRead>,
    State(state): State<AppState>,
    query: Result<Query<ListQuery>, QueryRejection>,
) -> AppResult<Json<DatasetPage>> {
    let q = query_or_problem(query)?;
    let org = caller_org(&auth.principal, q.org_id.as_deref())?;
    let limit = parse_limit(q.limit.as_deref())?;
    let status = match q.status.as_deref().filter(|s| !s.is_empty()) {
        Some(s) => Some(
            serde_json::from_value::<DatasetStatus>(Value::from(s))
                .map(|s| wire(&s))
                .map_err(|_| {
                    AppError::Validation(vec![invalid("status", "must be frozen or archived")])
                })?,
        ),
        None => None,
    };
    let after = match q.cursor.as_deref().filter(|s| !s.is_empty()) {
        Some(raw) => {
            let c = Cursor::decode(raw)?;
            let id = Uuid::parse_str(&c.id).map_err(|_| {
                AppError::Validation(vec![invalid("cursor", "not a cursor from this API")])
            })?;
            Some((c.at, id))
        }
        None => None,
    };
    let (after_at, after_id) = after.unzip();
    let mut rows = sqlx::query_as!(
        DatasetRow,
        "SELECT id, org_id, name, description, status, cohort_id,
                frozen_at, content_hash, member_count, erased_count, exclusion_rules,
                exclusions, label_source, split, split_counts, lineage, data_dictionary,
                created_by, created_at, archived_at
         FROM datasets
         WHERE org_id = $1
           AND ($2::text IS NULL OR status = $2)
           AND ($3::timestamptz IS NULL OR (created_at, id) < ($3, $4::uuid))
         ORDER BY created_at DESC, id DESC
         LIMIT $5",
        org,
        status,
        after_at,
        after_id,
        limit + 1,
    )
    .fetch_all(&state.db)
    .await?;
    let next_cursor = if rows.len() > usize::try_from(limit).unwrap_or(usize::MAX) {
        rows.pop();
        rows.last()
            .map(|r| Cursor::new(r.created_at, r.id.to_string()).encode())
    } else {
        None
    };
    Ok(Json(DatasetPage {
        items: rows.iter().map(DatasetRow::dto).collect::<AppResult<_>>()?,
        next_cursor,
    }))
}

async fn get_dataset(
    auth: Require<perm::AnalysisRead>,
    State(state): State<AppState>,
    Path(raw): Path<String>,
) -> AppResult<Json<Dataset>> {
    let mut conn = state.db.acquire().await?;
    let row = load_dataset(&mut conn, path_uuid(&raw)?, false).await?;
    auth.ensure_org(row.org_id)?;
    Ok(Json(row.dto()?))
}

#[derive(Debug, Default, Deserialize)]
struct MembersQuery {
    split: Option<String>,
    cursor: Option<String>,
    limit: Option<String>,
}

/// A `dataset_members` row.
struct MemberRow {
    session_id: String,
    participant_pseudonym: Option<String>,
    age_months: Option<i32>,
    age_band: Option<String>,
    split: Option<String>,
    label: Option<Value>,
    erased: bool,
}

async fn list_members(
    auth: Require<perm::AnalysisRead>,
    meta: RequestMeta,
    State(state): State<AppState>,
    Path(raw): Path<String>,
    query: Result<Query<MembersQuery>, QueryRejection>,
) -> AppResult<Json<DatasetMemberPage>> {
    let id = path_uuid(&raw)?;
    let q = query_or_problem(query)?;
    let limit = parse_limit(q.limit.as_deref())?;
    let split = match q.split.as_deref().filter(|s| !s.is_empty()) {
        Some(s) => Some(
            parse_split(s)
                .map(|p| split_str(p).to_owned())
                .ok_or_else(|| {
                    AppError::Validation(vec![invalid(
                        "split",
                        "must be train, validation or holdout",
                    )])
                })?,
        ),
        None => None,
    };
    let mut conn = state.db.acquire().await?;
    let dataset = load_dataset(&mut conn, id, false).await?;
    auth.ensure_org(dataset.org_id)?;
    // PRV-2: rows are per session, so the caller must also read sessions of the org.
    rbac::authorize(&auth.principal, Action::SessionRead, Some(dataset.org_id))?;
    let after = match q.cursor.as_deref().filter(|s| !s.is_empty()) {
        Some(c) => {
            let c = Cursor::decode(c)?;
            if c.at != dataset.frozen_at {
                return Err(AppError::Validation(vec![invalid(
                    "cursor",
                    "not a cursor of this dataset",
                )]));
            }
            Some(c.id)
        }
        None => None,
    };
    let mut rows = sqlx::query_as!(
        MemberRow,
        r#"SELECT session_id, participant_pseudonym, age_months, age_band, split, label,
                  erased_at IS NOT NULL AS "erased!"
           FROM dataset_members
           WHERE dataset_id = $1
             AND ($2::text IS NULL OR split = $2)
             AND ($3::text IS NULL OR session_id > $3)
           ORDER BY session_id
           LIMIT $4"#,
        id,
        split,
        after,
        limit + 1,
    )
    .fetch_all(&mut *conn)
    .await?;
    drop(conn);
    record_read(
        &state.db,
        &auth.principal,
        &meta,
        dataset.org_id,
        "dataset.members_read",
        "dataset",
        &id.to_string(),
        Some(json!({"after": after, "limit": limit, "split": split})),
    )
    .await?;
    let next_cursor = if rows.len() > usize::try_from(limit).unwrap_or(usize::MAX) {
        rows.pop();
        rows.last()
            .map(|r| Cursor::new(dataset.frozen_at, r.session_id.clone()).encode())
    } else {
        None
    };
    let items = rows
        .into_iter()
        .map(|r| {
            Ok(DatasetMember {
                session_id: r.session_id,
                participant_pseudonym: r.participant_pseudonym,
                age_months: r.age_months,
                age_band: r.age_band,
                split: r.split.as_deref().and_then(parse_split),
                label: r.label.map(|l| decode::<Label>("label", l)).transpose()?,
                excluded: r.erased.then(|| EXCLUDED_ERASED.to_owned()),
            })
        })
        .collect::<AppResult<_>>()?;
    Ok(Json(DatasetMemberPage { items, next_cursor }))
}

// ---- archive -----------------------------------------------------------------------------------

async fn archive_dataset(
    auth: Require<perm::ExperimentWrite>,
    meta: RequestMeta,
    State(state): State<AppState>,
    Path(raw): Path<String>,
) -> AppResult<Json<Dataset>> {
    let id = path_uuid(&raw)?;
    let mut tx = state.db.begin().await?;
    let before = load_dataset(&mut tx, id, true).await?;
    auth.ensure_org(before.org_id)?;
    if before.status != "frozen" {
        return Err(AppError::Conflict {
            slug: slugs::INVALID_TRANSITION,
            detail: "the dataset is already archived".into(),
        });
    }
    let row = sqlx::query_as!(
        DatasetRow,
        "UPDATE datasets SET status = 'archived', archived_at = now() WHERE id = $1
         RETURNING id, org_id, name, description, status, cohort_id,
                   frozen_at, content_hash, member_count, erased_count, exclusion_rules,
                   exclusions, label_source, split, split_counts, lineage, data_dictionary,
                   created_by, created_at, archived_at",
        id
    )
    .fetch_one(&mut *tx)
    .await?;
    AuditEvent::user(&auth.principal, "dataset.archive", "dataset", id)
        .org(row.org_id)
        .before(json!({"status": before.status}))
        .after(json!({"status": row.status, "content_hash": hex(&row.content_hash)}))
        .meta(&meta)
        .record(&mut tx)
        .await?;
    tx.commit().await?;
    Ok(Json(row.dto()?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use asd_contract::datasets::{ExplicitExclusion, SplitFractions, SplitRequest};

    fn req() -> CreateDatasetRequest {
        CreateDatasetRequest {
            name: " Toddlers ".into(),
            description: Some("  ".into()),
            cohort_id: Uuid::nil().to_string(),
            parent_dataset_id: None,
            run_id: None,
            exclusion_rules: Vec::new(),
            labels: None,
            split: None,
        }
    }

    #[test]
    fn requests_are_checked() {
        assert_eq!(
            check_request(&req()).unwrap(),
            ("Toddlers".to_owned(), None)
        );
        let mut r = req();
        r.exclusion_rules = vec![
            ExclusionRule::Tukey {
                id: "t".into(),
                metric: "m".into(),
                k: 1.5,
            },
            ExclusionRule::ExplicitList {
                id: "t".into(),
                sessions: vec![ExplicitExclusion {
                    session_id: "S".into(),
                    reason: " ".into(),
                }],
            },
        ];
        r.labels = Some(LabelSourceKind::Import);
        r.split = Some(SplitRequest {
            seed: None,
            fractions: SplitFractions {
                train: 0.5,
                validation: 0.5,
                holdout: 0.5,
            },
            ..SplitRequest::default()
        });
        let names: Vec<String> = check_request(&r)
            .unwrap_err()
            .into_iter()
            .map(|p| p.name)
            .collect();
        assert_eq!(
            names,
            [
                "exclusion_rules[0].kind",
                "exclusion_rules[1].id",
                "exclusion_rules[1].sessions[0].reason",
                "labels",
                "split.fractions",
            ]
        );
    }

    #[test]
    fn quality_gates_name_the_first_failed_threshold() {
        let rules = vec![
            ExclusionRule::QualityGates {
                id: "q".into(),
                min_valid_ratio: Some(0.6),
                exclude_quality_verdicts: Vec::new(),
                exclude_tracking_verdicts: vec![asd_core::types::tracking::TrackingVerdict::Poor],
                min_clips_completed: None,
            },
            ExclusionRule::ExplicitList {
                id: "e".into(),
                sessions: vec![ExplicitExclusion {
                    session_id: "S-2".into(),
                    reason: "duplicate".into(),
                }],
            },
        ];
        let facts = |ratio: Option<f64>, tracking: &str| SessionFacts {
            participant_id: Uuid::nil(),
            valid_frame_ratio: ratio,
            quality_verdict: Some("optimal".into()),
            tracking_verdict: Some(tracking.into()),
            clips: 0,
        };
        assert_eq!(
            excluded_by(&rules, "S-1", &facts(Some(0.5), "good")),
            Some(("q".into(), reasons::VALID_RATIO.into()))
        );
        assert_eq!(
            excluded_by(&rules, "S-1", &facts(None, "good")),
            Some(("q".into(), reasons::VALID_RATIO.into()))
        );
        assert_eq!(
            excluded_by(&rules, "S-1", &facts(Some(0.9), "poor")),
            Some(("q".into(), reasons::TRACKING_VERDICT.into()))
        );
        assert_eq!(
            excluded_by(&rules, "S-2", &facts(Some(0.9), "good")),
            Some(("e".into(), "duplicate".into()))
        );
        assert_eq!(excluded_by(&rules, "S-1", &facts(Some(0.9), "good")), None);
    }

    #[test]
    fn pseudonyms_are_salted_and_stable_within_a_salt() {
        let a = fresh_salt();
        let b = fresh_salt();
        let p = pseudonym(&a, "P", "x", 12);
        assert_eq!(p.len(), 14);
        assert!(p.starts_with("P-"));
        assert_eq!(p, pseudonym(&a, "P", "x", 12));
        assert_ne!(p, pseudonym(&b, "P", "x", 12));
        assert_ne!(pseudonym(&a, "R", "x", 12)[2..], p[2..]);
        assert_eq!(unhex(&hex(&[0, 255, 16])).unwrap(), vec![0, 255, 16]);
    }
}
