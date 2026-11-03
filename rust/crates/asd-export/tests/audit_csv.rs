//! owner: WP-B.23
//!
//! `audit_csv` encoder (HIP-3): header, canonical dialect, formula escaping (CSV-2), ids and
//! hashes only, determinism, row limit.

use asd_export::tabular::audit::{encode, file_name, AuditRow, COLUMNS, ROW_LIMIT};
use asd_export::ExportError;

fn row(id: i64) -> AuditRow {
    AuditRow {
        id,
        ts: "2026-10-31T09:15:00.000Z".into(),
        actor_type: "user".into(),
        actor_id: Some("0b9f6c1e-7a51-4c1e-9d43-2f1a7f0c9e11".into()),
        action: "export.create".into(),
        target_type: "export".into(),
        target_id: Some("e1".into()),
        before_sha256: None,
        after_sha256: Some("ab".repeat(32)),
    }
}

fn text(rows: &[AuditRow]) -> String {
    String::from_utf8(encode(rows).unwrap()).unwrap()
}

#[test]
fn canonical_dialect_header_and_rows() {
    let out = text(&[row(1), row(2)]);
    assert!(!out.starts_with('\u{feff}'), "no BOM");
    assert!(!out.contains('\r'), "LF only");
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0], COLUMNS.join(","));
    assert_eq!(
        lines[1],
        format!(
            "1,2026-10-31T09:15:00.000Z,user,0b9f6c1e-7a51-4c1e-9d43-2f1a7f0c9e11,export.create,export,e1,,{}",
            "ab".repeat(32)
        )
    );
    assert!(out.ends_with('\n'));
}

#[test]
fn there_are_no_payload_or_network_columns() {
    for c in COLUMNS {
        for banned in ["before", "after", "ip", "user_agent", "payload"] {
            assert!(
                c != banned,
                "column {c} must not exist: ids and hashes only"
            );
        }
    }
    assert_eq!(COLUMNS.len(), 9);
}

#[test]
fn formula_cells_are_escaped_and_quoting_is_rfc4180() {
    let mut r = row(7);
    r.actor_id = Some("=HYPERLINK(\"http://x\")".into());
    r.action = "+cmd|' /C calc'!A0".into();
    r.target_id = Some("a,b\"c".into());
    r.target_type = "@sum".into();
    let out = text(&[r]);
    let line = out.lines().nth(1).unwrap();
    assert!(line.contains("\"'=HYPERLINK(\"\"http://x\"\")\""), "{line}");
    assert!(line.contains(",'+cmd|' /C calc'!A0,"), "{line}");
    assert!(line.contains(",'@sum,"), "{line}");
    assert!(line.contains("\"a,b\"\"c\""), "{line}");
    // The numeric id is never guarded.
    assert!(line.starts_with("7,"));
}

#[test]
fn output_is_deterministic_and_empty_input_is_a_header() {
    assert_eq!(encode(&[row(1)]).unwrap(), encode(&[row(1)]).unwrap());
    assert_eq!(text(&[]), format!("{}\n", COLUMNS.join(",")));
    assert_eq!(file_name("abc"), "export-abc-audit-csv.csv");
}

#[test]
fn the_row_limit_is_enforced() {
    let rows = vec![row(1); ROW_LIMIT + 1];
    assert!(matches!(
        encode(&rows),
        Err(ExportError::ExportTooLarge { .. })
    ));
}
