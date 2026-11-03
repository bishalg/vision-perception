//! owner: WP-B.23
//!
//! `audit_csv` (HIP-3): the request rules of an audit-log export and the row query. The
//! encoder is `asd_export::tabular::audit`; this module only reads rows.
//!
//! Rows of the requesting org only, oldest first (`id` order, so the file is stable),
//! within `from`/`to` (the session-filter grammar, `to` inclusive), of one `actor_id`
//! and/or an `action` prefix. `before`/`after` are never selected as values: only their
//! SHA-256 leaves the database, so clinical text in a payload cannot reach the file.

use asd_contract::export::{ExportFilters, ExportMode, ExportPurpose, MAX_EXPORT_ROWS};
use asd_export::tabular::audit::AuditRow;
use uuid::Uuid;

use super::filters::ParsedFilters;
use crate::error::{AppError, AppResult};
use crate::problem::InvalidParam;
use crate::read::cursor;

/// Longest `action_prefix` (audit actions are short dotted names).
pub const MAX_ACTION_PREFIX_CHARS: usize = 64;

fn bad(name: &str, reason: &str) -> InvalidParam {
    InvalidParam {
        name: name.into(),
        reason: reason.into(),
    }
}

/// Request rules of an `audit_csv` export beyond the shared ones: de-identified mode,
/// `quality_improvement` purpose, no destinations, and filters limited to `from`, `to`,
/// `actor_id` (a UUID) and `action_prefix`.
pub fn validate(
    mode: ExportMode,
    purpose: ExportPurpose,
    filters: &ExportFilters,
    destinations: usize,
) -> Vec<InvalidParam> {
    let mut errors = Vec::new();
    if mode != ExportMode::Deidentified {
        errors.push(bad(
            "mode",
            "audit_csv exports are deidentified only (HIP-3)",
        ));
    }
    if purpose != ExportPurpose::QualityImprovement {
        errors.push(bad(
            "purpose",
            "audit_csv exports have purpose quality_improvement (HIP-3)",
        ));
    }
    if destinations > 0 {
        errors.push(bad(
            "destination_ids",
            "audit_csv exports are download only",
        ));
    }
    let only_audit = ExportFilters {
        from: filters.from.clone(),
        to: filters.to.clone(),
        actor_id: filters.actor_id.clone(),
        action_prefix: filters.action_prefix.clone(),
        ..ExportFilters::default()
    };
    if &only_audit != filters {
        errors.push(bad(
            "filters",
            "audit_csv exports filter by from, to, actor_id and action_prefix only",
        ));
    }
    if let Some(a) = &filters.actor_id {
        if Uuid::parse_str(a).is_err() {
            errors.push(bad("filters.actor_id", "must be a user id (UUID)"));
        }
    }
    if let Some(p) = &filters.action_prefix {
        if p.is_empty()
            || p.chars().count() > MAX_ACTION_PREFIX_CHARS
            || p.chars().any(char::is_control)
        {
            errors.push(bad(
                "filters.action_prefix",
                "must be 1..64 characters without control characters",
            ));
        }
    }
    errors
}

/// The org's audit rows for `filters` (`parsed` is `filters::parse` of the same filters).
/// `export-too-large` as a permanent error message when more than the export limit match.
pub async fn load(
    db: &sqlx::PgPool,
    org: Uuid,
    filters: &ExportFilters,
    parsed: &ParsedFilters,
) -> AppResult<Vec<AuditRow>> {
    let limit = i64::try_from(MAX_EXPORT_ROWS).expect("small") + 1;
    let rows = sqlx::query!(
        r#"SELECT id, ts, actor_type, actor_id, action, target_type, target_id,
                  encode(sha256(convert_to(before::text, 'UTF8')), 'hex') AS before_sha256,
                  encode(sha256(convert_to(after::text, 'UTF8')), 'hex') AS after_sha256
           FROM audit_log
           WHERE org_id = $1
             AND ($2::text IS NULL OR actor_id = $2)
             AND ($3::text IS NULL OR starts_with(action, $3))
             AND ($4::timestamptz IS NULL OR ts >= $4)
             AND ($5::timestamptz IS NULL OR ts <= $5)
           ORDER BY id
           LIMIT $6"#,
        org,
        filters.actor_id.as_deref(),
        filters.action_prefix.as_deref(),
        parsed.from,
        parsed.to,
        limit,
    )
    .fetch_all(db)
    .await?;
    if rows.len() > MAX_EXPORT_ROWS {
        return Err(AppError::Internal(format!(
            "export-too-large: more than {MAX_EXPORT_ROWS} audit rows match; narrow the filters"
        )));
    }
    Ok(rows
        .into_iter()
        .map(|r| AuditRow {
            id: r.id,
            ts: cursor::ts(r.ts),
            actor_type: r.actor_type,
            actor_id: r.actor_id,
            action: r.action,
            target_type: r.target_type,
            target_id: r.target_id,
            before_sha256: r.before_sha256,
            after_sha256: r.after_sha256,
        })
        .collect())
}
