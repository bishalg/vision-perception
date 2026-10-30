//! owner: WP-B.4
//!
//! Config document lifecycle (CFG-1): `draft -> candidate -> published -> retired`.
//!
//! | Step     | From                      | Who (RBAC)      | Four-eyes rule                                  |
//! |----------|---------------------------|-----------------|-------------------------------------------------|
//! | create   | -                         | `ConfigWrite`   | -                                               |
//! | approve  | `draft`                   | `ConfigPublish` | approver != author                              |
//! | publish  | `candidate`               | `ConfigPublish` | publisher != author; seeded: != approver        |
//! | retire   | draft, candidate, published| `ConfigPublish` | -                                               |
//!
//! Publishing retires the previously published document of the same kind in the same
//! transaction (one published document per kind, enforced by a partial unique index in
//! `0004_config_approval.sql`). Every step writes one audit row in its transaction
//! (AUTH-5); snapshots carry kind, version and status, never the body.

use asd_contract::api_v1::{ConfigDocument, ConfigDocumentKind, ConfigDocumentStatus};
use asd_contract::config::ScoringProfile;
use asd_core::types::{NormativeBaselineSet, StimulusCatalog};
use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use sqlx::PgConnection;
use uuid::Uuid;

use super::{conflict, kind_str, status_from_str, ConfigApiError, ConfigApiResult};
use crate::audit::AuditEvent;
use crate::auth::extractors::{RequestMeta, UserPrincipal};
use crate::error::AppError;
use crate::problem::InvalidParam;
use crate::read::cursor::{self, Cursor};

/// Serialises every `publish` (any kind) so "retire the old, publish the new" never races.
const PUBLISH_LOCK: i64 = 0x4153_4443_4647_0001;

/// A `config_documents` row.
#[derive(Debug, Clone)]
pub struct DocRow {
    pub id: Uuid,
    pub kind: String,
    pub version: String,
    pub status: String,
    pub body: Value,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub published_at: Option<DateTime<Utc>>,
    pub notes: Option<String>,
    pub approved_by: Option<Uuid>,
}

impl DocRow {
    pub fn status(&self) -> ConfigApiResult<ConfigDocumentStatus> {
        status_from_str(&self.status)
            .ok_or_else(|| AppError::Internal("stored config status not in the contract".into()))
            .map_err(ConfigApiError::from)
    }

    pub fn to_dto(&self) -> ConfigApiResult<ConfigDocument> {
        Ok(ConfigDocument {
            id: self.id.to_string(),
            kind: super::kind_from_str(&self.kind).ok_or_else(|| {
                AppError::Internal("stored config kind not in the contract".into())
            })?,
            version: self.version.clone(),
            status: self.status()?,
            body: self.body.clone(),
            created_by: self.created_by.map(|u| u.to_string()),
            created_at: cursor::ts(self.created_at),
            published_at: self.published_at.map(cursor::ts),
            notes: self.notes.clone(),
        })
    }

    fn snapshot(&self) -> Value {
        json!({"kind": self.kind, "version": self.version, "status": self.status})
    }
}

// ---- validation (pure) ------------------------------------------------------------------

fn param(name: &str, reason: impl Into<String>) -> InvalidParam {
    InvalidParam {
        name: name.into(),
        reason: reason.into(),
    }
}

/// `1..=64` characters of `[A-Za-z0-9._-]` (it appears in URLs and bundle summaries).
pub fn validate_version(version: &str) -> Result<(), InvalidParam> {
    let ok = !version.is_empty()
        && version.len() <= 64
        && version
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'));
    if ok {
        Ok(())
    } else {
        Err(param(
            "version",
            "must be 1 to 64 characters of letters, digits, '.', '_' or '-'",
        ))
    }
}

/// Range checks on a scoring profile; the type itself only checks the shape.
pub fn scoring_profile_problems(p: &ScoringProfile) -> Vec<String> {
    let mut problems = Vec::new();
    let unit = |name: &str, v: f64, problems: &mut Vec<String>| {
        if !(0.0..=1.0).contains(&v) {
            problems.push(format!("{name} must be in [0, 1]"));
        }
    };
    unit("flag.greenFloor", p.flag.green_floor, &mut problems);
    unit("flag.yellowFloor", p.flag.yellow_floor, &mut problems);
    unit(
        "flag.lowConfidenceShare",
        p.flag.low_confidence_share,
        &mut problems,
    );
    unit(
        "flag.atypicalShareForRed",
        p.flag.atypical_share_for_red,
        &mut problems,
    );
    unit(
        "lowConfidenceValidRatio",
        p.low_confidence_valid_ratio,
        &mut problems,
    );
    unit(
        "qualityGate.minConfidence",
        p.quality_gate.min_confidence,
        &mut problems,
    );
    unit(
        "qualityGate.blinkThreshold",
        p.quality_gate.blink_threshold,
        &mut problems,
    );
    if p.flag.yellow_floor > p.flag.green_floor {
        problems.push("flag.yellowFloor must not exceed flag.greenFloor".into());
    }
    let q = &p.quality_gate;
    if !(q.min_distance_cm > 0.0 && q.min_distance_cm < q.max_distance_cm) {
        problems.push("qualityGate needs 0 < minDistanceCm < maxDistanceCm".into());
    }
    if !(q.max_yaw_rad > 0.0 && q.max_pitch_rad > 0.0) {
        problems.push("qualityGate.maxYawRad and maxPitchRad must be positive".into());
    }
    if p.schema_version.trim().is_empty() {
        problems.push("schemaVersion is required".into());
    }
    problems
}

/// The body must decode as the kind's contract type (and pass its own checks), so a
/// published document always composes into a valid bundle.
pub fn validate_body(kind: ConfigDocumentKind, body: &Value) -> Result<(), Vec<InvalidParam>> {
    let decode_err = |e: serde_json::Error| vec![param("body", e.to_string())];
    let problems = match kind {
        ConfigDocumentKind::StimulusCatalog => {
            let c: StimulusCatalog = serde_json::from_value(body.clone()).map_err(decode_err)?;
            let mut p = Vec::new();
            if c.clips.is_empty() {
                p.push("clips must not be empty".to_owned());
            }
            p
        }
        ConfigDocumentKind::NormativeBaselines => {
            let set: NormativeBaselineSet =
                serde_json::from_value(body.clone()).map_err(decode_err)?;
            asd_core::baselines::validate::<&str>(&set, &[])
        }
        ConfigDocumentKind::ScoringProfile => {
            let p: ScoringProfile = serde_json::from_value(body.clone()).map_err(decode_err)?;
            scoring_profile_problems(&p)
        }
    };
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems.into_iter().map(|p| param("body", p)).collect())
    }
}

/// Which lifecycle step a four-eyes check is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Approve,
    Publish,
}

/// The four-eyes rule. `author = None` is a system-seeded document (CFG-2): it has no
/// human author, so approver and publisher must be two different people instead.
pub fn four_eyes(
    step: Step,
    actor: Uuid,
    author: Option<Uuid>,
    approver: Option<Uuid>,
) -> Result<(), &'static str> {
    if author == Some(actor) {
        return Err(match step {
            Step::Approve => "the author of a document cannot approve it",
            Step::Publish => "the author of a document cannot publish it",
        });
    }
    if step == Step::Publish && author.is_none() && approver == Some(actor) {
        return Err("a system-seeded document is published by someone other than its approver");
    }
    Ok(())
}

// ---- database ---------------------------------------------------------------------------

async fn load(
    conn: &mut PgConnection,
    kind: ConfigDocumentKind,
    version: &str,
    for_update: bool,
) -> ConfigApiResult<DocRow> {
    let kind = kind_str(kind);
    let row = if for_update {
        sqlx::query_as!(
            DocRow,
            "SELECT id, kind, version, status, body, created_by, created_at, published_at,
                    notes, approved_by
             FROM config_documents WHERE kind = $1 AND version = $2 FOR UPDATE",
            kind,
            version
        )
        .fetch_optional(&mut *conn)
        .await?
    } else {
        sqlx::query_as!(
            DocRow,
            "SELECT id, kind, version, status, body, created_by, created_at, published_at,
                    notes, approved_by
             FROM config_documents WHERE kind = $1 AND version = $2",
            kind,
            version
        )
        .fetch_optional(&mut *conn)
        .await?
    };
    row.ok_or(ConfigApiError::App(AppError::NotFound))
}

pub async fn get(
    conn: &mut PgConnection,
    kind: ConfigDocumentKind,
    version: &str,
) -> ConfigApiResult<DocRow> {
    load(conn, kind, version, false).await
}

/// One page of documents of `kind`, newest first.
pub async fn list(
    conn: &mut PgConnection,
    kind: ConfigDocumentKind,
    status: Option<ConfigDocumentStatus>,
    after: Option<&Cursor>,
    limit: i64,
) -> ConfigApiResult<(Vec<DocRow>, Option<String>)> {
    let (after_at, after_id) = match after {
        Some(c) => (
            Some(c.at),
            Some(Uuid::parse_str(&c.id).map_err(|_| {
                AppError::Validation(vec![param("cursor", "not a cursor from this API")])
            })?),
        ),
        None => (None, None),
    };
    let mut rows = sqlx::query_as!(
        DocRow,
        "SELECT id, kind, version, status, body, created_by, created_at, published_at,
                notes, approved_by
         FROM config_documents
         WHERE kind = $1
           AND ($2::text IS NULL OR status = $2)
           AND ($3::timestamptz IS NULL OR (created_at, id) < ($3, $4))
         ORDER BY created_at DESC, id DESC
         LIMIT $5",
        kind_str(kind),
        status.map(super::status_str),
        after_at,
        after_id,
        limit + 1
    )
    .fetch_all(&mut *conn)
    .await?;
    let next = if rows.len() > usize::try_from(limit).unwrap_or(usize::MAX) {
        rows.pop();
        rows.last()
            .map(|r| Cursor::new(r.created_at, r.id.to_string()).encode())
    } else {
        None
    };
    Ok((rows, next))
}

/// Inserts a `draft`. A taken `(kind, version)` is `409 config-version-exists`.
pub async fn create(
    conn: &mut PgConnection,
    actor: &UserPrincipal,
    meta: &RequestMeta,
    kind: ConfigDocumentKind,
    version: &str,
    body: &Value,
    notes: Option<&str>,
) -> ConfigApiResult<DocRow> {
    validate_version(version).map_err(|p| AppError::Validation(vec![p]))?;
    validate_body(kind, body).map_err(AppError::Validation)?;
    let row = sqlx::query_as!(
        DocRow,
        "INSERT INTO config_documents (kind, version, status, body, created_by, notes)
         VALUES ($1, $2, 'draft', $3, $4, $5)
         ON CONFLICT (kind, version) DO NOTHING
         RETURNING id, kind, version, status, body, created_by, created_at, published_at,
                   notes, approved_by",
        kind_str(kind),
        version,
        body,
        actor.user_id,
        notes
    )
    .fetch_optional(&mut *conn)
    .await?
    .ok_or_else(|| {
        conflict(
            "config-version-exists",
            format!("{} version `{version}` already exists", kind_str(kind)),
        )
    })?;
    AuditEvent::user(actor, "config.document.create", "config_document", row.id)
        .after(row.snapshot())
        .meta(meta)
        .record(conn)
        .await?;
    Ok(row)
}

fn wrong_state(row: &DocRow, step: &str, expected: &str) -> ConfigApiError {
    conflict(
        "invalid-transition",
        format!(
            "cannot {step} a {} document (expected {expected})",
            row.status
        ),
    )
}

/// `draft -> candidate`.
pub async fn approve(
    conn: &mut PgConnection,
    actor: &UserPrincipal,
    meta: &RequestMeta,
    kind: ConfigDocumentKind,
    version: &str,
) -> ConfigApiResult<DocRow> {
    let before = load(conn, kind, version, true).await?;
    if before.status()? != ConfigDocumentStatus::Draft {
        return Err(wrong_state(&before, "approve", "draft"));
    }
    four_eyes(Step::Approve, actor.user_id, before.created_by, None)
        .map_err(ConfigApiError::FourEyes)?;
    // Drafts can also come from outside `create` (norm-run promote, WP-B.5): check the
    // body here too, so a candidate always composes.
    if let Err(problems) = validate_body(kind, &before.body) {
        let reasons: Vec<String> = problems.into_iter().map(|p| p.reason).collect();
        return Err(conflict(
            "config-invalid",
            format!("the document body is not valid: {}", reasons.join("; ")),
        ));
    }
    let row = sqlx::query_as!(
        DocRow,
        "UPDATE config_documents
         SET status = 'candidate', approved_by = $2, approved_at = now()
         WHERE id = $1
         RETURNING id, kind, version, status, body, created_by, created_at, published_at,
                   notes, approved_by",
        before.id,
        actor.user_id
    )
    .fetch_one(&mut *conn)
    .await?;
    AuditEvent::user(actor, "config.document.approve", "config_document", row.id)
        .before(before.snapshot())
        .after(row.snapshot())
        .meta(meta)
        .record(conn)
        .await?;
    Ok(row)
}

/// `candidate -> published`; the previously published document of the kind is retired.
pub async fn publish(
    conn: &mut PgConnection,
    actor: &UserPrincipal,
    meta: &RequestMeta,
    kind: ConfigDocumentKind,
    version: &str,
) -> ConfigApiResult<DocRow> {
    sqlx::query!("SELECT pg_advisory_xact_lock($1)", PUBLISH_LOCK)
        .execute(&mut *conn)
        .await?;
    let before = load(conn, kind, version, true).await?;
    if before.status()? != ConfigDocumentStatus::Candidate {
        return Err(wrong_state(&before, "publish", "candidate"));
    }
    four_eyes(
        Step::Publish,
        actor.user_id,
        before.created_by,
        before.approved_by,
    )
    .map_err(ConfigApiError::FourEyes)?;
    let retired = sqlx::query_scalar!(
        "UPDATE config_documents SET status = 'retired'
         WHERE kind = $1 AND status = 'published'
         RETURNING version",
        kind_str(kind)
    )
    .fetch_all(&mut *conn)
    .await?;
    let row = sqlx::query_as!(
        DocRow,
        "UPDATE config_documents
         SET status = 'published', published_at = now(), published_by = $2
         WHERE id = $1
         RETURNING id, kind, version, status, body, created_by, created_at, published_at,
                   notes, approved_by",
        before.id,
        actor.user_id
    )
    .fetch_one(&mut *conn)
    .await?;
    let mut after = row.snapshot();
    after["retired"] = json!(retired);
    AuditEvent::user(actor, "config.document.publish", "config_document", row.id)
        .before(before.snapshot())
        .after(after)
        .meta(meta)
        .record(conn)
        .await?;
    Ok(row)
}

/// Any state except `retired` -> `retired`. Bundles already composed keep serving.
pub async fn retire(
    conn: &mut PgConnection,
    actor: &UserPrincipal,
    meta: &RequestMeta,
    kind: ConfigDocumentKind,
    version: &str,
) -> ConfigApiResult<DocRow> {
    let before = load(conn, kind, version, true).await?;
    if before.status()? == ConfigDocumentStatus::Retired {
        return Err(wrong_state(
            &before,
            "retire",
            "draft, candidate or published",
        ));
    }
    let row = sqlx::query_as!(
        DocRow,
        "UPDATE config_documents SET status = 'retired' WHERE id = $1
         RETURNING id, kind, version, status, body, created_by, created_at, published_at,
                   notes, approved_by",
        before.id
    )
    .fetch_one(&mut *conn)
    .await?;
    AuditEvent::user(actor, "config.document.retire", "config_document", row.id)
        .before(before.snapshot())
        .after(row.snapshot())
        .meta(meta)
        .record(conn)
        .await?;
    Ok(row)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn four_eyes_rule() {
        let (a, b, c) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
        // Human author: anyone else may approve and publish, the approver included.
        assert!(four_eyes(Step::Approve, a, Some(a), None).is_err());
        assert!(four_eyes(Step::Approve, b, Some(a), None).is_ok());
        assert!(four_eyes(Step::Publish, a, Some(a), Some(b)).is_err());
        assert!(four_eyes(Step::Publish, b, Some(a), Some(b)).is_ok());
        assert!(four_eyes(Step::Publish, c, Some(a), Some(b)).is_ok());
        // System-seeded: two different people.
        assert!(four_eyes(Step::Approve, a, None, None).is_ok());
        assert!(four_eyes(Step::Publish, a, None, Some(a)).is_err());
        assert!(four_eyes(Step::Publish, b, None, Some(a)).is_ok());
    }

    #[test]
    fn versions() {
        for ok in [
            "1",
            "1.0.0",
            "2026.10.30-1",
            "bundled_1.0.0",
            &"a".repeat(64),
        ] {
            assert!(validate_version(ok).is_ok(), "{ok}");
        }
        for bad in ["", "a b", "a/b", "ü", &"a".repeat(65), "a?b"] {
            assert!(validate_version(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn default_scoring_profile_is_valid_and_ranges_are_checked() {
        let p = ScoringProfile::default();
        assert!(scoring_profile_problems(&p).is_empty());
        assert!(validate_body(
            ConfigDocumentKind::ScoringProfile,
            &serde_json::to_value(&p).unwrap()
        )
        .is_ok());

        let mut bad = p.clone();
        bad.flag.green_floor = 1.5;
        bad.quality_gate.min_distance_cm = 80.0;
        let problems = scoring_profile_problems(&bad);
        assert!(
            problems.iter().any(|m| m.contains("greenFloor")),
            "{problems:?}"
        );
        assert!(
            problems.iter().any(|m| m.contains("minDistanceCm")),
            "{problems:?}"
        );

        let mut swapped = p;
        swapped.flag.yellow_floor = 0.9;
        assert!(!scoring_profile_problems(&swapped).is_empty());
    }

    #[test]
    fn bodies_must_match_their_kind() {
        let catalog: Value =
            serde_json::from_str(include_str!("../../seed/stimulus_catalog.json")).unwrap();
        let baselines: Value =
            serde_json::from_str(include_str!("../../seed/normative_baselines.json")).unwrap();
        assert!(validate_body(ConfigDocumentKind::StimulusCatalog, &catalog).is_ok());
        assert!(validate_body(ConfigDocumentKind::NormativeBaselines, &baselines).is_ok());
        assert!(validate_body(ConfigDocumentKind::NormativeBaselines, &catalog).is_err());
        assert!(validate_body(ConfigDocumentKind::StimulusCatalog, &baselines).is_err());
        assert!(validate_body(ConfigDocumentKind::ScoringProfile, &json!({})).is_err());
    }
}
