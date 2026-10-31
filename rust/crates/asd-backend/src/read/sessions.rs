//! owner: WP-B.3
//!
//! `GET /v1/orgs/{id}/sessions` (filters `flag, quality, from, to, bundle_version,
//! reviewed, tracking`, cursor over `(started_at, id)`, newest first) and
//! `GET /v1/sessions/{id}`. Bodies are `asd-contract::api_v1::{SessionPage, SessionDetail}`
//! (API-3).
//!
//! Tracking quality (SCH-4, WP-G.11): list items carry `tracking_verdict` and
//! `tracking_accuracy_deg`; `tracking=good|fair|poor|unusable` filters on the verdict. The
//! detail adds `tracking_quality`, the stored `rawPayload.trackingQuality` in snake_case,
//! read only for a session whose tracking columns were derived (1.7 with a record), so an
//! unknown `trackingQuality` key in a 1.6 record is never surfaced.
//!
//! `reviewed` = the session has a review item a human closed (`done` or `dismissed`).

use std::collections::BTreeMap;

use asd_contract::api_v1::{
    SessionAttentionFlag, SessionDetail, SessionPage, SessionSummary, SessionTrackingQuality,
};
use asd_contract::session_v1_7::TrackingQuality;
use asd_core::types::tracking::TrackingVerdict;
use chrono::{DateTime, Duration, NaiveDate, Utc};
use serde::Deserialize;
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use super::cursor::{self, Cursor};
use crate::error::{AppError, AppResult};
use crate::problem::InvalidParam;

/// Raw query parameters (all strings, parsed with problem+json errors).
#[derive(Debug, Default, Deserialize)]
pub struct SessionQuery {
    pub flag: Option<String>,
    pub quality: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub bundle_version: Option<String>,
    pub reviewed: Option<String>,
    pub tracking: Option<String>,
    pub cursor: Option<String>,
    pub limit: Option<String>,
}

/// Parsed, validated filters.
#[derive(Debug, Default, PartialEq)]
pub struct SessionFilters {
    pub flag: Option<String>,
    pub quality: Option<String>,
    pub from: Option<DateTime<Utc>>,
    /// Inclusive upper bound.
    pub to: Option<DateTime<Utc>>,
    pub bundle_version: Option<String>,
    pub reviewed: Option<bool>,
    /// Tracking verdict (`good|fair|poor|unusable`).
    pub tracking: Option<String>,
    pub cursor: Option<Cursor>,
    pub limit: i64,
}

fn bad(name: &str, reason: &str) -> InvalidParam {
    InvalidParam {
        name: name.into(),
        reason: reason.into(),
    }
}

/// RFC 3339, or a calendar date `YYYY-MM-DD` (UTC): the day's first instant for `from`,
/// its last microsecond for `to`.
fn parse_bound(raw: &str, upper: bool) -> Option<DateTime<Utc>> {
    if let Ok(t) = DateTime::parse_from_rfc3339(raw) {
        return Some(t.with_timezone(&Utc));
    }
    let day = NaiveDate::parse_from_str(raw, "%Y-%m-%d").ok()?;
    let start = day.and_hms_opt(0, 0, 0)?.and_utc();
    Some(if upper {
        start + Duration::days(1) - Duration::microseconds(1)
    } else {
        start
    })
}

impl SessionQuery {
    pub fn parse(self) -> AppResult<SessionFilters> {
        let mut errors = Vec::new();
        let non_empty = |v: Option<String>| v.filter(|s| !s.is_empty());

        let flag = non_empty(self.flag);
        if let Some(f) = &flag {
            if serde_json::from_value::<SessionAttentionFlag>(Value::String(f.clone())).is_err() {
                errors.push(bad("flag", "must be green, yellow, red or unreliable"));
            }
        }
        let mut bound = |name: &str, raw: Option<String>, upper: bool| {
            non_empty(raw).and_then(|r| {
                let parsed = parse_bound(&r, upper);
                if parsed.is_none() {
                    errors.push(bad(name, "must be RFC 3339 or YYYY-MM-DD"));
                }
                parsed
            })
        };
        let from = bound("from", self.from, false);
        let to = bound("to", self.to, true);
        let tracking = non_empty(self.tracking);
        if let Some(t) = &tracking {
            if serde_json::from_value::<TrackingVerdict>(Value::String(t.clone())).is_err() {
                errors.push(bad("tracking", "must be good, fair, poor or unusable"));
            }
        }
        let reviewed = match non_empty(self.reviewed).as_deref() {
            None => None,
            Some("true") => Some(true),
            Some("false") => Some(false),
            Some(_) => {
                errors.push(bad("reviewed", "must be true or false"));
                None
            }
        };
        let limit = match non_empty(self.limit) {
            None => cursor::limit(None),
            Some(raw) => raw
                .parse::<i64>()
                .map_err(|_| AppError::Validation(vec![bad("limit", "must be an integer")]))
                .and_then(|n| cursor::limit(Some(n))),
        };
        let cursor = non_empty(self.cursor)
            .map(|c| Cursor::decode(&c))
            .transpose();

        let (limit, cursor) = match (limit, cursor) {
            (Ok(l), Ok(c)) if errors.is_empty() => (l, c),
            (l, c) => {
                for e in [l.err(), c.err()].into_iter().flatten() {
                    if let AppError::Validation(p) = e {
                        errors.extend(p);
                    }
                }
                return Err(AppError::Validation(errors));
            }
        };
        Ok(SessionFilters {
            flag,
            quality: non_empty(self.quality),
            from,
            to,
            bundle_version: non_empty(self.bundle_version),
            reviewed,
            tracking,
            cursor,
            limit,
        })
    }
}

pub(crate) fn parse_verdict(s: Option<String>) -> AppResult<Option<TrackingVerdict>> {
    s.map(|s| {
        serde_json::from_value(Value::String(s)).map_err(|_| {
            AppError::Internal("stored tracking_verdict is not in the contract".into())
        })
    })
    .transpose()
}

fn parse_flag(s: &str) -> AppResult<SessionAttentionFlag> {
    serde_json::from_value(Value::String(s.to_owned()))
        .map_err(|_| AppError::Internal("stored attention_flag is not in the contract".into()))
}

/// Columns shared by the list and the detail.
struct SummaryColumns {
    id: String,
    participant_id: Uuid,
    participant_token: String,
    started_at: DateTime<Utc>,
    duration_sec: f64,
    battery: Option<String>,
    capture_source: String,
    quality_verdict: String,
    attention_flag: String,
    composite_score: Option<f64>,
    composite_confidence: Option<f64>,
    bundle_version: Option<String>,
    device_id: Option<Uuid>,
    reviewed: bool,
    tracking_verdict: Option<String>,
    tracking_accuracy_deg: Option<f64>,
}

impl SummaryColumns {
    fn into_dto(self) -> AppResult<SessionSummary> {
        Ok(SessionSummary {
            id: self.id,
            participant_id: self.participant_id.to_string(),
            participant_token: self.participant_token,
            started_at: cursor::ts(self.started_at),
            duration_sec: self.duration_sec,
            battery: self.battery,
            capture_source: serde_json::from_value(Value::String(self.capture_source)).ok(),
            quality_verdict: self.quality_verdict,
            attention_flag: parse_flag(&self.attention_flag)?,
            composite_score: self.composite_score,
            composite_confidence: self.composite_confidence,
            bundle_version: self.bundle_version,
            device_id: self.device_id.map(|d| d.to_string()),
            reviewed: Some(self.reviewed),
            tracking_verdict: parse_verdict(self.tracking_verdict)?,
            tracking_accuracy_deg: self.tracking_accuracy_deg,
        })
    }
}

/// One page of an org's sessions. The caller has checked org access.
pub async fn list(db: &PgPool, org: Uuid, f: &SessionFilters) -> AppResult<SessionPage> {
    let (after_at, after_id) = match &f.cursor {
        Some(c) => (Some(c.at), Some(c.id.clone())),
        None => (None, None),
    };
    let rows = sqlx::query!(
        r#"SELECT s.id, s.participant_id, s.participant_token, s.started_at, s.duration_sec,
                  s.battery, s.capture_source, s.quality_verdict, s.attention_flag,
                  s.composite_score, s.composite_confidence, s.bundle_version, s.device_id,
                  s.tracking_verdict, s.tracking_accuracy_deg,
                  EXISTS (SELECT 1 FROM review_items r
                          WHERE r.session_id = s.id AND r.status IN ('done', 'dismissed'))
                      AS "reviewed!"
           FROM sessions s
           WHERE s.org_id = $1
             AND ($2::text IS NULL OR s.attention_flag = $2)
             AND ($3::text IS NULL OR s.quality_verdict = $3)
             AND ($4::timestamptz IS NULL OR s.started_at >= $4)
             AND ($5::timestamptz IS NULL OR s.started_at <= $5)
             AND ($6::text IS NULL OR s.bundle_version = $6)
             AND ($7::bool IS NULL OR EXISTS (
                     SELECT 1 FROM review_items r
                     WHERE r.session_id = s.id AND r.status IN ('done', 'dismissed')) = $7)
             AND ($8::timestamptz IS NULL OR (s.started_at, s.id) < ($8, $9::text))
             AND ($11::text IS NULL OR s.tracking_verdict = $11)
           ORDER BY s.started_at DESC, s.id DESC
           LIMIT $10"#,
        org,
        f.flag,
        f.quality,
        f.from,
        f.to,
        f.bundle_version,
        f.reviewed,
        after_at,
        after_id,
        f.limit + 1,
        f.tracking,
    )
    .fetch_all(db)
    .await?;

    let total = sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!" FROM sessions s
           WHERE s.org_id = $1
             AND ($2::text IS NULL OR s.attention_flag = $2)
             AND ($3::text IS NULL OR s.quality_verdict = $3)
             AND ($4::timestamptz IS NULL OR s.started_at >= $4)
             AND ($5::timestamptz IS NULL OR s.started_at <= $5)
             AND ($6::text IS NULL OR s.bundle_version = $6)
             AND ($7::bool IS NULL OR EXISTS (
                     SELECT 1 FROM review_items r
                     WHERE r.session_id = s.id AND r.status IN ('done', 'dismissed')) = $7)
             AND ($8::text IS NULL OR s.tracking_verdict = $8)"#,
        org,
        f.flag,
        f.quality,
        f.from,
        f.to,
        f.bundle_version,
        f.reviewed,
        f.tracking,
    )
    .fetch_one(db)
    .await?;

    let limit = usize::try_from(f.limit).unwrap_or(usize::MAX);
    let has_more = rows.len() > limit;
    let mut items = Vec::with_capacity(rows.len().min(limit));
    let mut last = None;
    for r in rows.into_iter().take(limit) {
        last = Some(Cursor::new(r.started_at, r.id.clone()));
        items.push(
            SummaryColumns {
                id: r.id,
                participant_id: r.participant_id,
                participant_token: r.participant_token,
                started_at: r.started_at,
                duration_sec: r.duration_sec,
                battery: r.battery,
                capture_source: r.capture_source,
                quality_verdict: r.quality_verdict,
                attention_flag: r.attention_flag,
                composite_score: r.composite_score,
                composite_confidence: r.composite_confidence,
                bundle_version: r.bundle_version,
                device_id: r.device_id,
                reviewed: r.reviewed,
                tracking_verdict: r.tracking_verdict,
                tracking_accuracy_deg: r.tracking_accuracy_deg,
            }
            .into_dto()?,
        );
    }
    Ok(SessionPage {
        items,
        next_cursor: if has_more {
            last.map(|c| c.encode())
        } else {
            None
        },
        total: Some(u64::try_from(total).unwrap_or(0)),
    })
}

/// The session's org (for the access check) and its detail; `None` when unknown.
/// `presigned` = `BLOB_MODE=presigned`, where upload completion is not observed and
/// `report_pdf_available` is `null` until a key is recorded.
pub async fn detail(
    db: &PgPool,
    id: &str,
    presigned: bool,
) -> AppResult<Option<(Uuid, SessionDetail)>> {
    let Some(r) = sqlx::query!(
        r#"SELECT s.id, s.org_id, s.participant_id, s.participant_token, s.started_at,
                  s.duration_sec, s.battery, s.capture_source, s.quality_verdict,
                  s.attention_flag, s.composite_score, s.composite_confidence,
                  s.bundle_version, s.device_id, s.age_pack, s.lang, s.schema_version,
                  s.valid_frame_ratio, s.calibrated_distance_cm, s.social_preference,
                  s.eye_dwell, s.mouth_dwell, s.first_eye_latency_ms, s.affect,
                  s.report_pdf_key, s.tracking_verdict, s.tracking_accuracy_deg,
                  CASE WHEN s.tracking_verdict IS NOT NULL
                       THEN s.summary -> 'trackingQuality' END AS tracking_quality,
                  EXISTS (SELECT 1 FROM review_items r
                          WHERE r.session_id = s.id AND r.status IN ('done', 'dismissed'))
                      AS "reviewed!"
           FROM sessions s WHERE s.id = $1"#,
        id,
    )
    .fetch_optional(db)
    .await?
    else {
        return Ok(None);
    };

    let affect: Option<BTreeMap<String, Value>> =
        r.affect.and_then(|a| serde_json::from_value(a).ok());
    let report_pdf_available = match (&r.report_pdf_key, presigned) {
        (Some(_), _) => Some(true),
        (None, true) => None,
        (None, false) => Some(false),
    };
    let summary = SummaryColumns {
        id: r.id,
        participant_id: r.participant_id,
        participant_token: r.participant_token,
        started_at: r.started_at,
        duration_sec: r.duration_sec,
        battery: r.battery,
        capture_source: r.capture_source,
        quality_verdict: r.quality_verdict,
        attention_flag: r.attention_flag,
        composite_score: r.composite_score,
        composite_confidence: r.composite_confidence,
        bundle_version: r.bundle_version,
        device_id: r.device_id,
        reviewed: r.reviewed,
        tracking_verdict: r.tracking_verdict,
        tracking_accuracy_deg: r.tracking_accuracy_deg,
    }
    .into_dto()?;
    let tracking_quality = r
        .tracking_quality
        .map(|v| {
            serde_json::from_value::<TrackingQuality>(v)
                .map(|tq| SessionTrackingQuality::from(&tq))
                .map_err(|e| AppError::Internal(format!("stored trackingQuality: {e}")))
        })
        .transpose()?;
    Ok(Some((
        r.org_id,
        SessionDetail {
            summary,
            org_id: r.org_id.to_string(),
            age_pack: r.age_pack,
            lang: r.lang,
            schema_version: Some(r.schema_version),
            valid_frame_ratio: Some(r.valid_frame_ratio),
            calibrated_distance_cm: r.calibrated_distance_cm,
            social_preference: Some(r.social_preference),
            eye_dwell: r.eye_dwell,
            mouth_dwell: r.mouth_dwell,
            first_eye_latency_ms: r.first_eye_latency_ms,
            affect,
            report_pdf_available,
            tracking_quality,
        },
    )))
}

/// `(org_id, report blob key)` of a session: the recorded key, else the ADR-B3 key a
/// pre-signed upload would have used.
pub async fn report_key(db: &PgPool, id: &str) -> AppResult<Option<(Uuid, String)>> {
    let row = sqlx::query!(
        "SELECT org_id, participant_token, report_pdf_key FROM sessions WHERE id = $1",
        id
    )
    .fetch_optional(db)
    .await?;
    Ok(row.map(|r| {
        let key = r.report_pdf_key.unwrap_or_else(|| {
            crate::routes::blobs::BlobKind::Report.key(r.org_id, &r.participant_token, id)
        });
        (r.org_id, key)
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn date_bounds_cover_whole_days() {
        let from = parse_bound("2026-10-02", false).unwrap();
        let to = parse_bound("2026-10-04", true).unwrap();
        assert_eq!(cursor::ts(from), "2026-10-02T00:00:00.000Z");
        assert_eq!(to.to_rfc3339(), "2026-10-04T23:59:59.999999+00:00");
        assert!(parse_bound("2026-10-02T10:00:00+02:00", false).is_some());
        assert!(parse_bound("last week", false).is_none());
    }

    #[test]
    fn query_errors_are_collected() {
        let q = SessionQuery {
            flag: Some("purple".into()),
            reviewed: Some("maybe".into()),
            limit: Some("500".into()),
            tracking: Some("excellent".into()),
            ..Default::default()
        };
        let Err(AppError::Validation(p)) = q.parse() else {
            panic!("expected validation error")
        };
        let names: Vec<_> = p.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["flag", "tracking", "reviewed", "limit"]);
    }
}
