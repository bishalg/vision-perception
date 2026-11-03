//! owner: WP-X.8
//!
//! `analysis_results` (RUN-7, PRV-3): loads a finished analysis run and its sealed
//! per-session rows and calls `asd_export::analysis`. The research frame log is never read
//! here (PRV-5): only the run's own result blob.
//!
//! **Stored shapes** (read from what WP-X.3 writes): `analysis_runs.summary` is the contract
//! `RunSummary`; `result_blob_key` points at the per-session rows as JSON Lines, one contract
//! `AnalysisSessionRow` per line, sealed as `key_id (16 bytes) || AEAD ciphertext` under the
//! org data key with AAD `("analysis_runs", "result", org, run id)` (the research log's
//! layout, `routes::blobs::seal_research_log`).
//!
//! The export request's `analysis` block has no column of its own: it is stored in the
//! `exports.filters` JSON under the key `analysis` ([`split_filters`] separates the two).

use std::collections::BTreeMap;

use asd_contract::export::AnalysisExport;
use asd_contract::research::{AnalysisSessionRow, RunSummary};
use asd_export::analysis::{Provenance, SessionInput};
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use super::hex;
use crate::blob::BlobStore;
use crate::crypto::{Aad, Keyring};
use crate::error::{AppError, AppResult};
use crate::problem::InvalidParam;

/// Key of the analysis block inside the stored `exports.filters`.
pub const FILTERS_KEY: &str = "analysis";
const KEY_ID_LEN: usize = 16;

/// `exports.filters` as stored: the session filters and the analysis block, apart.
pub fn split_filters(stored: &Value) -> (Value, Option<AnalysisExport>) {
    let mut filters = stored.clone();
    let analysis = filters
        .as_object_mut()
        .and_then(|m| m.remove(FILTERS_KEY))
        .and_then(|v| serde_json::from_value(v).ok());
    (filters, analysis)
}

/// The stored form of a request's filters plus its analysis block.
pub fn join_filters(filters: Value, analysis: &AnalysisExport) -> Value {
    let mut out = filters;
    if let Some(m) = out.as_object_mut() {
        m.insert(
            FILTERS_KEY.to_owned(),
            serde_json::to_value(analysis).expect("plain data"),
        );
    }
    out
}

/// A finished run's record.
pub struct RunRecord {
    pub provenance: Provenance,
    pub summary: RunSummary,
    pub blob_key: Option<String>,
}

fn bad(field: &str, detail: &str) -> AppError {
    AppError::Validation(vec![InvalidParam {
        name: field.to_owned(),
        reason: detail.to_owned(),
    }])
}

/// The run `run` of `org`: `404` for another org's or a missing run, `400` when it has not
/// `succeeded` (or has no summary).
pub async fn load_run(db: &PgPool, org: Uuid, run: Uuid) -> AppResult<RunRecord> {
    let r = sqlx::query!(
        r#"SELECT kind, status, definition_hash, cohort_hash, membership_hash, profile_hash,
                  published_profile_version, baselines_version, catalog_version, bundle_version,
                  core_version, contract_version, pipeline_version, registry_version, seed,
                  level_requested, sessions_n, summary, result_blob_key, finished_at
           FROM analysis_runs WHERE id = $1 AND org_id = $2"#,
        run,
        org,
    )
    .fetch_optional(db)
    .await?
    .ok_or(AppError::NotFound)?;
    if r.status != "succeeded" {
        return Err(bad("analysis.run_id", "the run has not succeeded"));
    }
    let summary: RunSummary = r
        .summary
        .and_then(|v| serde_json::from_value(v).ok())
        .ok_or_else(|| AppError::Internal("a succeeded run has no readable summary".into()))?;
    Ok(RunRecord {
        provenance: Provenance {
            run_id: run.to_string(),
            kind: r.kind,
            definition_hash: hex(&r.definition_hash),
            cohort_hash: hex(&r.cohort_hash),
            membership_hash: r.membership_hash.as_deref().map(hex),
            profile_hash: hex(&r.profile_hash),
            published_profile_version: r.published_profile_version,
            baselines_version: r.baselines_version,
            catalog_version: r.catalog_version,
            bundle_version: r.bundle_version,
            core_version: r.core_version,
            contract_version: r.contract_version,
            pipeline_version: r.pipeline_version,
            registry_version: r.registry_version,
            seed: r.seed,
            level_requested: r.level_requested,
            sessions_n: r.sessions_n.and_then(|n| u32::try_from(n).ok()),
            finished_at: r.finished_at.map(crate::read::cursor::ts),
        },
        summary,
        blob_key: r.result_blob_key,
    })
}

/// Opens and parses the run's sealed rows.
pub async fn load_rows(
    blobs: &dyn BlobStore,
    keyring: &Keyring,
    org: Uuid,
    run: Uuid,
    key: &str,
) -> Result<Vec<AnalysisSessionRow>, String> {
    let stored = blobs
        .get(key)
        .await
        .map_err(|e| format!("blob-store: {e}"))?
        .ok_or_else(|| "the run's result object is gone".to_owned())?;
    let sealed = &stored.bytes;
    if sealed.len() <= KEY_ID_LEN {
        return Err("the run's result object is truncated".into());
    }
    let (id, ciphertext) = sealed.split_at(KEY_ID_LEN);
    let key_id = Uuid::from_slice(id).map_err(|_| "the run's result has no key id".to_owned())?;
    let aad = Aad::new("analysis_runs", "result", org, run);
    let plain = keyring
        .decrypt(org, &aad, key_id, ciphertext)
        .await
        .map_err(|_| "the run's result object cannot be opened".to_owned())?;
    plain
        .split(|b| *b == b'\n')
        .filter(|l| !l.is_empty())
        .map(|l| serde_json::from_slice(l).map_err(|_| "a result row is malformed".to_owned()))
        .collect()
}

/// What the database knows about the exported sessions.
pub struct SessionFacts {
    pub inputs: Vec<SessionInput>,
    /// `(participant, session id)` of every included session (`export_items`).
    pub items: Vec<(Uuid, String)>,
    /// Rows left out: `withdrawn` (the participant withdrew since the run) and
    /// `session_gone` (erased).
    pub excluded: BTreeMap<String, u64>,
}

/// Joins the rows with `sessions`: month of `started_at`, owner; drops withdrawn
/// participants' and erased sessions' rows.
pub async fn session_facts(
    db: &PgPool,
    org: Uuid,
    rows: Vec<AnalysisSessionRow>,
) -> Result<SessionFacts, sqlx::Error> {
    let ids: Vec<String> = rows.iter().map(|r| r.session_id.clone()).collect();
    let found = sqlx::query!(
        r#"SELECT s.id, s.participant_id,
                  to_char(s.started_at AT TIME ZONE 'UTC', 'YYYY-MM') AS month,
                  (p.withdrawn_at IS NOT NULL) AS "withdrawn!"
           FROM sessions s JOIN participants p ON p.id = s.participant_id
           WHERE s.org_id = $1 AND s.id = ANY($2)"#,
        org,
        &ids,
    )
    .fetch_all(db)
    .await?;
    let by_id: BTreeMap<_, _> = found.into_iter().map(|f| (f.id.clone(), f)).collect();
    let mut out = SessionFacts {
        inputs: Vec::new(),
        items: Vec::new(),
        excluded: BTreeMap::new(),
    };
    for row in rows {
        match by_id.get(&row.session_id) {
            None => *out.excluded.entry("session_gone".into()).or_default() += 1,
            Some(f) if f.withdrawn => *out.excluded.entry("withdrawn".into()).or_default() += 1,
            Some(f) => {
                out.items.push((f.participant_id, row.session_id.clone()));
                out.inputs.push(SessionInput {
                    session_month: f.month.clone(),
                    row,
                });
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use asd_contract::export::{AnalysisEncoding, AnalysisLayout};

    #[test]
    fn filters_and_analysis_split_and_join() {
        let a = AnalysisExport {
            run_id: "r".into(),
            layout: AnalysisLayout::Sessions,
            encoding: AnalysisEncoding::Parquet,
        };
        let stored = join_filters(serde_json::json!({}), &a);
        let (filters, back) = split_filters(&stored);
        assert_eq!(filters, serde_json::json!({}));
        assert_eq!(back, Some(a));
        assert_eq!(split_filters(&serde_json::json!({})).1, None);
    }
}
