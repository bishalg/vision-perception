//! owner: WP-B.23
//!
//! The audit log as canonical CSV (HIP-3, `audit_csv`): one table, one row per audit entry,
//! the same RFC 4180 dialect and formula guard ([`super::csv::guard`], CSV-2) as the other
//! CSV exports.
//!
//! The column set is **ids and hashes only**: who (actor type and id), what (action), on
//! what (target type and id) and when, plus the SHA-256 of the entry's `before` and `after`
//! payloads so a reader can tell two entries changed the same data without the file ever
//! carrying the payloads, which can hold clinical text. There is no `before`/`after`, `ip`
//! or `user_agent` column. Output is deterministic (no clock, no randomness; XC-8).

use crate::ExportError;

/// The header row, in order.
pub const COLUMNS: [&str; 9] = [
    "audit_id",
    "ts",
    "actor_type",
    "actor_id",
    "action",
    "target_type",
    "target_id",
    "before_sha256",
    "after_sha256",
];

/// Rows above which the encoder refuses (same bound as every export, XLSX-3's order).
pub const ROW_LIMIT: usize = 200_000;

/// One audit entry as the backend read it. Timestamps are RFC 3339 UTC text; the hashes are
/// lower-case hex or `None` when the entry has no such payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditRow {
    pub id: i64,
    pub ts: String,
    pub actor_type: String,
    pub actor_id: Option<String>,
    pub action: String,
    pub target_type: String,
    pub target_id: Option<String>,
    pub before_sha256: Option<String>,
    pub after_sha256: Option<String>,
}

/// RFC 4180 quoting: only when the field has a comma, quote, CR or LF.
fn quote(field: &str) -> String {
    if field.contains([',', '"', '\r', '\n']) {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_owned()
    }
}

/// Text cell: formula-guarded, then quoted.
fn text(field: &str) -> String {
    quote(&super::csv::guard(field))
}

fn opt(field: Option<&str>) -> String {
    field.map_or_else(String::new, text)
}

/// The file name of an audit export.
pub fn file_name(export_id: &str) -> String {
    format!("export-{export_id}-audit-csv.csv")
}

/// Canonical CSV (UTF-8 without BOM, LF) of `rows`, in the order given.
pub fn encode(rows: &[AuditRow]) -> Result<Vec<u8>, ExportError> {
    if rows.len() > ROW_LIMIT {
        return Err(ExportError::ExportTooLarge {
            rows: rows.len(),
            limit: ROW_LIMIT,
        });
    }
    let mut out = String::new();
    out.push_str(&COLUMNS.join(","));
    out.push('\n');
    for r in rows {
        let line = [
            r.id.to_string(),
            text(&r.ts),
            text(&r.actor_type),
            opt(r.actor_id.as_deref()),
            text(&r.action),
            text(&r.target_type),
            opt(r.target_id.as_deref()),
            opt(r.before_sha256.as_deref()),
            opt(r.after_sha256.as_deref()),
        ];
        out.push_str(&line.join(","));
        out.push('\n');
    }
    Ok(out.into_bytes())
}
