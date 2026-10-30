//! owner: WP-B.3
//!
//! `GET /v1/sessions/{id}/clips` (CON-19): one item per `session_clip_scores` row in play
//! order, with `rows[]` = the clip's per-metric expected-vs-recorded cells (from the stored
//! `details.rows`) and the flat `metric/expected/recorded/deviation` columns taken from the
//! worst-status row (ties: largest |deviation|). `baseline_ref` is the stored column.

use asd_contract::api_v1::{ClipList, ClipScore};
use asd_core::types::{ExpectationStatus, PerVideoScoreRow};
use serde_json::Value;
use sqlx::PgPool;

use crate::error::AppResult;
use crate::ingest::derive::{metric_row, worst_row};

/// `details.rows` of a stored clip; empty when absent or not decodable.
fn detail_rows(details: Option<&Value>) -> Vec<PerVideoScoreRow> {
    details
        .and_then(|d| d.get("rows"))
        .and_then(|rows| serde_json::from_value(rows.clone()).ok())
        .unwrap_or_default()
}

/// Clips of a session the caller may read.
pub async fn list(db: &PgPool, session_id: &str) -> AppResult<ClipList> {
    let stored = sqlx::query!(
        r#"SELECT clip_id, clip_order, status, score, weight, dwell_social, dwell_geometric,
                  dwell_eye, dwell_mouth, orienting_latency_ms, gaze_cue_accuracy,
                  fixation_count, mean_fixation_ms, baseline_ref, details
           FROM session_clip_scores WHERE session_id = $1
           ORDER BY clip_order, clip_id"#,
        session_id,
    )
    .fetch_all(db)
    .await?;

    let items = stored
        .into_iter()
        .map(|c| {
            let rows = detail_rows(c.details.as_ref());
            let worst = worst_row(&rows).map(|i| metric_row(&rows[i]));
            let status = c
                .status
                .and_then(|s| serde_json::from_value::<ExpectationStatus>(Value::String(s)).ok());
            ClipScore {
                clip_id: c.clip_id,
                order: i64::from(c.clip_order),
                status,
                score: c.score,
                weight: c.weight,
                dwell_social: c.dwell_social,
                dwell_geometric: c.dwell_geometric,
                dwell_eye: c.dwell_eye,
                dwell_mouth: c.dwell_mouth,
                orienting_latency_ms: c.orienting_latency_ms,
                gaze_cue_accuracy: c.gaze_cue_accuracy,
                fixation_count: c.fixation_count.map(i64::from),
                mean_fixation_ms: c.mean_fixation_ms,
                metric: worst.as_ref().map(|w| w.metric.clone()),
                expected: worst.as_ref().and_then(|w| w.expected),
                recorded: worst.as_ref().map(|w| w.recorded),
                deviation: worst.as_ref().and_then(|w| w.deviation),
                baseline_ref: c.baseline_ref,
                rows: rows.iter().map(metric_row).collect(),
            }
        })
        .collect();
    Ok(ClipList { items })
}
