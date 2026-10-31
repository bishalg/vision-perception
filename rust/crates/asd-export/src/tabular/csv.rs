//! owner: WP-I.2
//!
//! CSV in three dialects from one writer (CSV-1), formula-injection guard (CSV-2), ZIP of
//! `sessions.csv`, `clips.csv`, `metric_rows.csv`, `reviews.csv` and `manifest.json`
//! (CSV-3; fixed ZIP timestamps for XC-8).
//!
//! Text cells starting with `=`, `+`, `-`, `@`, TAB or CR get a leading `'` (CSV-2). Numbers
//! never pass through the guard, so `-1.5` stays a number; the `utc_offset` column
//! (`+05:45`, generated here) is [`Cell::Verbatim`] for the same reason.

use std::io::Write as _;

use serde::Serialize;
use sha2::{Digest, Sha256};

use super::columns::{rows, Cell, Column, Table};
use super::{Dialect, TabularOptions};
use crate::model::ExportRecord;
use crate::watermark::MANDATORY_SENTENCE;
use crate::ExportError;

const BOM: &[u8] = b"\xEF\xBB\xBF";

/// Prefix `'` to text a spreadsheet could read as a formula (CSV-2).
pub fn guard(s: &str) -> String {
    if s.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        format!("'{s}")
    } else {
        s.to_owned()
    }
}

/// RFC 4180 quoting: only when the field has a comma, quote, CR or LF.
fn quote(field: &str) -> String {
    if field.contains([',', '"', '\r', '\n']) {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_owned()
    }
}

fn render(cell: &Cell, dialect: Dialect) -> String {
    match cell {
        Cell::Empty => String::new(),
        Cell::Text(s) => quote(&guard(s)),
        Cell::Verbatim(s) => quote(s),
        Cell::Int(i) => i.to_string(),
        // Display is the shortest round-trip form with `.` and no exponent.
        Cell::Float(f) => f.to_string(),
        Cell::Bool(b) => match (dialect, b) {
            (Dialect::Sheets, true) => "TRUE".to_owned(),
            (Dialect::Sheets, false) => "FALSE".to_owned(),
            (_, true) => "true".to_owned(),
            (_, false) => "false".to_owned(),
        },
        Cell::Time(t) => match dialect {
            Dialect::Canonical => t.rfc3339_utc(),
            Dialect::Excel | Dialect::Sheets => t.local_text(),
        },
    }
}

/// Canonical dialect only: the offset column of a UTC timestamp is `+00:00`.
fn canonical_offset(row: &mut [Cell], columns: &[Column]) {
    for (i, col) in columns.iter().enumerate() {
        if col.name == "utc_offset" && matches!(row[i], Cell::Verbatim(_)) {
            row[i] = Cell::Verbatim("+00:00".to_owned());
        }
    }
}

fn write_table(
    table: Table,
    export_id: &str,
    records: &[ExportRecord],
    dialect: Dialect,
) -> Result<(Vec<u8>, usize), ExportError> {
    let eol: &str = if dialect == Dialect::Excel {
        "\r\n"
    } else {
        "\n"
    };
    let columns = table.columns();
    let mut out = String::new();
    let header: Vec<&str> = columns.iter().map(|c| c.name).collect();
    out.push_str(&header.join(","));
    out.push_str(eol);
    let mut data = rows(table, export_id, records)?;
    let count = data.len();
    for row in &mut data {
        if dialect == Dialect::Canonical {
            canonical_offset(row, columns);
        }
        let line: Vec<String> = row.iter().map(|c| render(c, dialect)).collect();
        out.push_str(&line.join(","));
        out.push_str(eol);
    }
    let mut bytes = Vec::with_capacity(out.len() + BOM.len());
    if dialect == Dialect::Excel {
        bytes.extend_from_slice(BOM);
    }
    bytes.extend_from_slice(out.as_bytes());
    Ok((bytes, count))
}

/// One table as CSV in `dialect` (CSV-1). De-identifies first when the mode says so.
pub fn encode(
    records: &[ExportRecord],
    options: &TabularOptions,
    dialect: Dialect,
    table: Table,
) -> Result<Vec<u8>, ExportError> {
    let records = options.prepare(records)?;
    write_table(table, &options.watermark.export_id, &records, dialect).map(|(b, _)| b)
}

#[derive(Serialize)]
struct ManifestTable<'a> {
    file: &'static str,
    rows: usize,
    sha256: String,
    columns: &'a [Column],
}

#[derive(Serialize)]
struct Manifest<'a> {
    manifest_version: u32,
    export_id: &'a str,
    dialect: &'static str,
    watermark: &'a crate::watermark::Watermark,
    watermark_line: String,
    notice: &'static str,
    tables: Vec<ManifestTable<'a>>,
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

/// ZIP of the four tables plus `manifest.json` (watermark, column dictionary, row counts,
/// SHA-256 of each file) (CSV-3). Entries have a fixed timestamp and order (XC-8).
pub fn encode_zip(
    records: &[ExportRecord],
    options: &TabularOptions,
    dialect: Dialect,
) -> Result<Vec<u8>, ExportError> {
    let records = options.prepare(records)?;
    let export_id = options.watermark.export_id.as_str();

    let mut files: Vec<(&'static str, Vec<u8>)> = Vec::new();
    let mut tables = Vec::new();
    for table in Table::ALL {
        let (bytes, count) = write_table(table, export_id, &records, dialect)?;
        tables.push(ManifestTable {
            file: table.file_name(),
            rows: count,
            sha256: hex(&Sha256::digest(&bytes)),
            columns: table.columns(),
        });
        files.push((table.file_name(), bytes));
    }
    let manifest = Manifest {
        manifest_version: 1,
        export_id,
        dialect: dialect.name(),
        watermark: &options.watermark,
        watermark_line: options.watermark.line(),
        notice: MANDATORY_SENTENCE,
        tables,
    };
    let mut json = serde_json::to_vec_pretty(&manifest)
        .map_err(|e| ExportError::Tabular(format!("manifest: {e}")))?;
    json.push(b'\n');
    files.push(("manifest.json", json));

    let zip_err = |e: zip::result::ZipError| ExportError::Tabular(format!("zip: {e}"));
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .last_modified_time(zip::DateTime::default())
        .unix_permissions(0o644);
    for (name, bytes) in files {
        zip.start_file(name, opts).map_err(zip_err)?;
        zip.write_all(&bytes)
            .map_err(|e| ExportError::Tabular(format!("zip: {e}")))?;
    }
    Ok(zip.finish().map_err(zip_err)?.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guard_table() {
        for (input, want) in [
            ("=1+1", "'=1+1"),
            ("+x", "'+x"),
            ("-x", "'-x"),
            ("@x", "'@x"),
            ("\tx", "'\tx"),
            ("\rx", "'\rx"),
            ("plain", "plain"),
            ("a=b", "a=b"),
            ("", ""),
        ] {
            assert_eq!(guard(input), want, "{input:?}");
        }
    }

    #[test]
    fn quoting() {
        assert_eq!(quote("a,b"), "\"a,b\"");
        assert_eq!(quote("say \"hi\""), "\"say \"\"hi\"\"\"");
        assert_eq!(quote("l1\nl2"), "\"l1\nl2\"");
        assert_eq!(quote("plain"), "plain");
    }

    #[test]
    fn numbers_are_never_guarded() {
        assert_eq!(render(&Cell::Float(-1.5), Dialect::Canonical), "-1.5");
        assert_eq!(render(&Cell::Int(-3), Dialect::Excel), "-3");
        assert_eq!(render(&Cell::Float(1e-7), Dialect::Sheets), "0.0000001");
        assert_eq!(
            render(&Cell::Verbatim("+05:45".into()), Dialect::Excel),
            "+05:45"
        );
    }
}
