//! owner: WP-X.8
//!
//! `analysis_results` (RUN-7, PRV-3): the aggregates or the per-session rows of a finished
//! analysis run, always de-identified.
//!
//! - **`aggregates`**: one table `run_id, band, metric, side, n, mean, sd, median, q1, q3,
//!   p10, p90, ci_lo, ci_hi, suppressed`, built from the stored [`RunSummary`]. A suppressed
//!   cell (PRV-1) has `suppressed = true` and every number empty; the encoder blanks the
//!   numbers itself, so a summary that still carried some cannot leak them. No ids.
//! - **`sessions`**: the run's per-session rows through the `deid.rs` rules: `S-` session
//!   pseudonym with a fresh salt, `device_id` replaced by `site_pseudonym` (same HMAC,
//!   prefix `D-`), `age_months` dropped for `age_band`, the session date reduced to
//!   `YYYY-MM`. Rows are sorted by pseudonym, so the input order never shows. The sealed
//!   rows carry no participant id, so there is no `P-` column.
//!
//! Encodings: `jsonl` (line 1 is the manifest), `csv` (ZIP: table, `manifest.json`,
//! `README.txt`), `parquet` (`sessions` only; the provenance is file metadata `curv.*`).
//! The manifest `provenance` block is the run record: every RUN-5 field. Output is
//! deterministic (no clock, no randomness; XC-8). The research frame log has no encoder
//! here and cannot be named by [`ExportFormat`](asd_contract::export::ExportFormat)
//! (PRV-5).

use std::io::Write as _;

use asd_contract::export::{AnalysisEncoding, AnalysisLayout};
use asd_contract::research::{AnalysisSessionRow, BandSummary, Dist, RunSummary};
use serde::Serialize;
use serde_json::{json, Map, Value};

use crate::deid::{self, SALT_LEN};
use crate::policy::Mode;
use crate::tabular::csv::guard;
use crate::watermark::{Watermark, MANDATORY_SENTENCE};
use crate::ExportError;

/// Version of the shapes below.
pub const ANALYSIS_VERSION: &str = "1";

/// Header of the `aggregates` table (RUN-7).
pub const AGGREGATE_COLUMNS: &[&str] = &[
    "run_id",
    "band",
    "metric",
    "side",
    "n",
    "mean",
    "sd",
    "median",
    "q1",
    "q3",
    "p10",
    "p90",
    "ci_lo",
    "ci_hi",
    "suppressed",
];

/// Columns of the `sessions` table and whether each is numeric (else text).
pub const SESSION_COLUMNS: &[(&str, bool)] = &[
    ("session_id", false),
    ("site_pseudonym", false),
    ("session_month", false),
    ("level", false),
    ("age_band", false),
    ("published_composite", true),
    ("published_flag", false),
    ("published_quality_status", false),
    ("draft_composite", true),
    ("draft_flag", false),
    ("draft_quality_status", false),
    ("delta_composite", true),
    ("published_metrics", false),
    ("draft_metrics", false),
    ("delta_metrics", false),
    ("warnings", false),
];

/// The run record's reproducibility fields (RUN-5), as the manifest prints them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Provenance {
    pub run_id: String,
    pub kind: String,
    pub definition_hash: String,
    pub cohort_hash: String,
    pub membership_hash: Option<String>,
    pub profile_hash: String,
    pub published_profile_version: String,
    pub baselines_version: String,
    pub catalog_version: String,
    pub bundle_version: Option<String>,
    pub core_version: String,
    pub contract_version: String,
    pub pipeline_version: String,
    pub registry_version: String,
    pub seed: i64,
    pub level_requested: String,
    pub sessions_n: Option<u32>,
    pub finished_at: Option<String>,
}

/// One per-session row of the run and the month of that session (`YYYY-MM`, from
/// `sessions.started_at`).
#[derive(Debug, Clone, PartialEq)]
pub struct SessionInput {
    pub row: AnalysisSessionRow,
    pub session_month: Option<String>,
}

/// What to build.
#[derive(Debug, Clone)]
pub struct AnalysisOptions {
    pub watermark: Watermark,
    pub deid_salt: [u8; SALT_LEN],
    pub layout: AnalysisLayout,
    pub encoding: AnalysisEncoding,
    pub provenance: Provenance,
}

/// The finished file.
#[derive(Debug, Clone)]
pub struct AnalysisArtifact {
    pub bytes: Vec<u8>,
    pub file_name: String,
    pub content_type: &'static str,
    /// Table rows (aggregate cells or sessions).
    pub row_count: usize,
}

/// One row of the `aggregates` table.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AggregateRow {
    pub run_id: String,
    pub band: String,
    pub metric: String,
    pub side: &'static str,
    pub n: Option<u32>,
    pub mean: Option<f64>,
    pub sd: Option<f64>,
    pub median: Option<f64>,
    pub q1: Option<f64>,
    pub q3: Option<f64>,
    pub p10: Option<f64>,
    pub p90: Option<f64>,
    pub ci_lo: Option<f64>,
    pub ci_hi: Option<f64>,
    pub suppressed: bool,
}

fn aggregate_row(
    run_id: &str,
    band: &str,
    metric: &str,
    side: &'static str,
    d: &Dist,
    force_suppressed: bool,
) -> AggregateRow {
    let mut row = AggregateRow {
        run_id: run_id.to_owned(),
        band: band.to_owned(),
        metric: metric.to_owned(),
        side,
        n: None,
        mean: None,
        sd: None,
        median: None,
        q1: None,
        q3: None,
        p10: None,
        p90: None,
        ci_lo: None,
        ci_hi: None,
        suppressed: true,
    };
    if d.suppressed || force_suppressed {
        return row; // PRV-1: a suppressed cell carries no numbers.
    }
    row.suppressed = false;
    row.n = d.n;
    row.mean = d.mean;
    row.sd = d.sd;
    row.median = d.median;
    row.q1 = d.iqr.map(|i| i[0]);
    row.q3 = d.iqr.map(|i| i[1]);
    row.p10 = d.p10;
    row.p90 = d.p90;
    row.ci_lo = d.ci95_mean.map(|c| c[0]);
    row.ci_hi = d.ci95_mean.map(|c| c[1]);
    row
}

/// The aggregate table of a summary: each band, then `overall`; per band every metric then
/// `composite`; per quantity `published`, `draft`, `delta`.
pub fn aggregate_rows(run_id: &str, summary: &RunSummary) -> Vec<AggregateRow> {
    let mut out = Vec::new();
    let mut band = |b: &BandSummary| {
        let hidden = b.suppressed;
        let quantities = b
            .metrics
            .iter()
            .map(|(k, v)| (k.as_str(), v))
            .chain(std::iter::once(("composite", &b.composite)));
        for (name, cmp) in quantities {
            for (side, d) in [
                ("published", &cmp.published),
                ("draft", &cmp.draft),
                ("delta", &cmp.delta),
            ] {
                out.push(aggregate_row(run_id, &b.band, name, side, d, hidden));
            }
        }
    };
    for b in &summary.by_age_band {
        band(b);
    }
    band(&summary.overall);
    out
}

fn num(v: Option<f64>) -> String {
    v.map(|x| x.to_string()).unwrap_or_default()
}

fn quote(field: &str) -> String {
    if field.contains([',', '"', '\r', '\n']) {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_owned()
    }
}

fn text(field: &str) -> String {
    quote(&guard(field))
}

fn aggregates_csv(rows: &[AggregateRow]) -> Vec<u8> {
    let mut out = AGGREGATE_COLUMNS.join(",");
    out.push_str("\r\n");
    for r in rows {
        let cells = [
            text(&r.run_id),
            text(&r.band),
            text(&r.metric),
            r.side.to_owned(),
            r.n.map(|n| n.to_string()).unwrap_or_default(),
            num(r.mean),
            num(r.sd),
            num(r.median),
            num(r.q1),
            num(r.q3),
            num(r.p10),
            num(r.p90),
            num(r.ci_lo),
            num(r.ci_hi),
            r.suppressed.to_string(),
        ];
        out.push_str(&cells.join(","));
        out.push_str("\r\n");
    }
    out.into_bytes()
}

/// One de-identified session row.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SessionOut {
    pub session_id: String,
    pub site_pseudonym: Option<String>,
    pub session_month: Option<String>,
    pub level: String,
    pub age_band: Option<String>,
    pub published_composite: Option<f64>,
    pub published_flag: Option<String>,
    pub published_quality_status: Option<String>,
    pub draft_composite: Option<f64>,
    pub draft_flag: Option<String>,
    pub draft_quality_status: Option<String>,
    pub delta_composite: Option<f64>,
    pub published_metrics: Value,
    pub draft_metrics: Value,
    pub delta_metrics: Value,
    pub warnings: Vec<String>,
}

fn wire<T: Serialize>(v: &T) -> Option<String> {
    match serde_json::to_value(v) {
        Ok(Value::String(s)) => Some(s),
        _ => None,
    }
}

/// De-identified rows (PRV-3), sorted by session pseudonym.
pub fn session_rows(inputs: &[SessionInput], salt: &[u8; SALT_LEN]) -> Vec<SessionOut> {
    let mut out: Vec<SessionOut> = inputs
        .iter()
        .map(|i| {
            let r = &i.row;
            // Free-text warnings never carry the raw ids (defence in depth).
            let warnings = r
                .warnings
                .iter()
                .filter(|w| {
                    !w.contains(&r.session_id)
                        && r.device_id.as_deref().is_none_or(|d| !w.contains(d))
                })
                .cloned()
                .collect();
            SessionOut {
                session_id: deid::pseudonym(salt, "S-", &r.session_id),
                site_pseudonym: r
                    .device_id
                    .as_deref()
                    .map(|d| deid::pseudonym(salt, "D-", d)),
                session_month: i.session_month.clone(),
                level: wire(&r.level).unwrap_or_default(),
                age_band: r.age_band.clone(),
                published_composite: r.published.composite,
                published_flag: r.published.flag.as_ref().and_then(wire),
                published_quality_status: r.published.quality_status.clone(),
                draft_composite: r.draft.composite,
                draft_flag: r.draft.flag.as_ref().and_then(wire),
                draft_quality_status: r.draft.quality_status.clone(),
                delta_composite: r.delta.composite,
                published_metrics: json!(r.published.metrics),
                draft_metrics: json!(r.draft.metrics),
                delta_metrics: json!(r.delta.metrics),
                warnings,
            }
        })
        .collect();
    out.sort_by(|a, b| a.session_id.cmp(&b.session_id));
    out
}

/// A cell of the flat session table.
enum Cell {
    Text(Option<String>),
    Num(Option<f64>),
}

fn json_text(v: &Value) -> String {
    serde_json::to_string(v).expect("plain data")
}

/// The cells of `r`, in [`SESSION_COLUMNS`] order.
fn cells(r: &SessionOut) -> Vec<Cell> {
    use Cell::{Num, Text};
    vec![
        Text(Some(r.session_id.clone())),
        Text(r.site_pseudonym.clone()),
        Text(r.session_month.clone()),
        Text(Some(r.level.clone())),
        Text(r.age_band.clone()),
        Num(r.published_composite),
        Text(r.published_flag.clone()),
        Text(r.published_quality_status.clone()),
        Num(r.draft_composite),
        Text(r.draft_flag.clone()),
        Text(r.draft_quality_status.clone()),
        Num(r.delta_composite),
        Text(Some(json_text(&r.published_metrics))),
        Text(Some(json_text(&r.draft_metrics))),
        Text(Some(json_text(&r.delta_metrics))),
        Text(Some(json_text(&json!(r.warnings)))),
    ]
}

fn sessions_csv(rows: &[SessionOut]) -> Vec<u8> {
    let header: Vec<&str> = SESSION_COLUMNS.iter().map(|(n, _)| *n).collect();
    let mut out = header.join(",");
    out.push_str("\r\n");
    for r in rows {
        let line: Vec<String> = cells(r)
            .into_iter()
            .map(|c| match c {
                Cell::Text(t) => t.map(|s| text(&s)).unwrap_or_default(),
                Cell::Num(n) => num(n),
            })
            .collect();
        out.push_str(&line.join(","));
        out.push_str("\r\n");
    }
    out.into_bytes()
}

fn manifest(opts: &AnalysisOptions, summary: &RunSummary, row_count: usize) -> Value {
    let columns: Vec<&str> = match opts.layout {
        AnalysisLayout::Aggregates => AGGREGATE_COLUMNS.to_vec(),
        AnalysisLayout::Sessions => SESSION_COLUMNS.iter().map(|(n, _)| *n).collect(),
    };
    json!({
        "record_type": "manifest",
        "analysis_version": ANALYSIS_VERSION,
        "export_id": opts.watermark.export_id,
        "layout": opts.layout.as_str(),
        "encoding": opts.encoding.as_str(),
        "watermark": opts.watermark.line(),
        "notice": MANDATORY_SENTENCE,
        "mode": opts.watermark.mode,
        "purpose": opts.watermark.purpose,
        "row_count": row_count,
        "columns": columns,
        "small_cell_k": summary.small_cell_k,
        "includes_synthetic": summary.includes_synthetic,
        "provenance": opts.provenance,
    })
}

fn readme(opts: &AnalysisOptions, summary: &RunSummary, row_count: usize) -> String {
    let [line, notice] = opts.watermark.lines();
    let p = serde_json::to_value(&opts.provenance).expect("plain data");
    let mut s = format!(
        "{line}\n{notice}\n\nAnalysis run results, {} table, {row_count} rows.\n\
         De-identified. Cells with fewer than {} sessions are suppressed and hold no numbers.\n\
         Flags, metrics, distributions and statuses describe a cohort; they are not a clinical conclusion.\n\n\
         Provenance (the analysis run record):\n",
        opts.layout.as_str(),
        summary.small_cell_k,
    );
    if let Value::Object(m) = p {
        for (k, v) in m {
            let v = match v {
                Value::Null => "-".to_owned(),
                Value::String(x) => x,
                other => other.to_string(),
            };
            s.push_str(&format!("{k}: {v}\n"));
        }
    }
    s
}

fn zip_files(files: Vec<(&str, Vec<u8>)>) -> Result<Vec<u8>, ExportError> {
    let err = |e: zip::result::ZipError| ExportError::Analysis(format!("zip: {e}"));
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .last_modified_time(zip::DateTime::default())
        .unix_permissions(0o644);
    for (name, bytes) in files {
        zip.start_file(name, opts).map_err(err)?;
        zip.write_all(&bytes)
            .map_err(|e| ExportError::Analysis(format!("zip: {e}")))?;
    }
    Ok(zip.finish().map_err(err)?.into_inner())
}

fn jsonl<T: Serialize>(
    manifest: &Value,
    record_type: &str,
    rows: &[T],
) -> Result<Vec<u8>, ExportError> {
    let mut out = serde_json::to_vec(manifest).map_err(|e| ExportError::Analysis(e.to_string()))?;
    out.push(b'\n');
    for r in rows {
        let mut v = serde_json::to_value(r).map_err(|e| ExportError::Analysis(e.to_string()))?;
        if let Value::Object(m) = &mut v {
            m.insert("record_type".into(), Value::String(record_type.to_owned()));
        }
        out.extend(serde_json::to_vec(&v).map_err(|e| ExportError::Analysis(e.to_string()))?);
        out.push(b'\n');
    }
    Ok(out)
}

/// Builds the artifact. `summary` is the run's stored summary; `sessions` is read for the
/// `sessions` layout only. Refuses an identified watermark, `parquet` for `aggregates`, and
/// a run without a summary at the call site.
pub fn encode(
    opts: &AnalysisOptions,
    summary: &RunSummary,
    sessions: &[SessionInput],
) -> Result<AnalysisArtifact, ExportError> {
    if opts.watermark.mode != Mode::Deidentified {
        return Err(ExportError::DatasetIdentified);
    }
    let base = format!(
        "analysis_{}_{}",
        opts.layout.as_str(),
        opts.provenance.run_id
    );
    let (bytes, row_count, ext, content_type) = match (opts.layout, opts.encoding) {
        (AnalysisLayout::Aggregates, AnalysisEncoding::Parquet) => {
            return Err(ExportError::Analysis(
                "parquet is for the sessions layout only".into(),
            ))
        }
        (AnalysisLayout::Aggregates, enc) => {
            let rows = aggregate_rows(&opts.provenance.run_id, summary);
            let m = manifest(opts, summary, rows.len());
            if enc == AnalysisEncoding::Jsonl {
                (
                    jsonl(&m, "aggregate", &rows)?,
                    rows.len(),
                    "jsonl",
                    "application/x-ndjson",
                )
            } else {
                let zip = zip_files(vec![
                    ("aggregates.csv", aggregates_csv(&rows)),
                    ("manifest.json", pretty(&m)?),
                    ("README.txt", readme(opts, summary, rows.len()).into_bytes()),
                ])?;
                (zip, rows.len(), "zip", "application/zip")
            }
        }
        (AnalysisLayout::Sessions, enc) => {
            let rows = session_rows(sessions, &opts.deid_salt);
            let m = manifest(opts, summary, rows.len());
            match enc {
                AnalysisEncoding::Jsonl => (
                    jsonl(&m, "session", &rows)?,
                    rows.len(),
                    "jsonl",
                    "application/x-ndjson",
                ),
                AnalysisEncoding::Csv => {
                    let zip = zip_files(vec![
                        ("sessions.csv", sessions_csv(&rows)),
                        ("manifest.json", pretty(&m)?),
                        ("README.txt", readme(opts, summary, rows.len()).into_bytes()),
                    ])?;
                    (zip, rows.len(), "zip", "application/zip")
                }
                AnalysisEncoding::Parquet => (
                    parquet(opts, &m, &rows)?,
                    rows.len(),
                    "parquet",
                    "application/vnd.apache.parquet",
                ),
            }
        }
    };
    Ok(AnalysisArtifact {
        bytes,
        file_name: format!("{base}.{ext}"),
        content_type,
        row_count,
    })
}

fn pretty(m: &Value) -> Result<Vec<u8>, ExportError> {
    let mut b = serde_json::to_vec_pretty(m).map_err(|e| ExportError::Analysis(e.to_string()))?;
    b.push(b'\n');
    Ok(b)
}

#[cfg(feature = "parquet")]
fn parquet(
    opts: &AnalysisOptions,
    manifest: &Value,
    rows: &[SessionOut],
) -> Result<Vec<u8>, ExportError> {
    use std::sync::Arc;

    use parquet::basic::Compression;
    use parquet::data_type::{ByteArray, ByteArrayType, DoubleType};
    use parquet::file::metadata::KeyValue;
    use parquet::file::properties::{EnabledStatistics, WriterProperties, WriterVersion};
    use parquet::file::writer::SerializedFileWriter;
    use parquet::schema::parser::parse_message_type;

    let err = |e: parquet::errors::ParquetError| ExportError::Analysis(format!("parquet: {e}"));
    let mut schema = String::from("message curv_asd_analysis {\n");
    for (name, numeric) in SESSION_COLUMNS {
        if *numeric {
            schema.push_str(&format!("  optional double {name};\n"));
        } else {
            schema.push_str(&format!("  optional binary {name} (UTF8);\n"));
        }
    }
    schema.push('}');
    let schema = Arc::new(parse_message_type(&schema).map_err(err)?);

    let mut meta = vec![
        KeyValue::new("curv.analysis_version".into(), ANALYSIS_VERSION.to_owned()),
        KeyValue::new("curv.export_id".into(), opts.watermark.export_id.clone()),
        KeyValue::new("curv.watermark".into(), opts.watermark.line()),
        KeyValue::new("curv.notice".into(), MANDATORY_SENTENCE.to_owned()),
        KeyValue::new("curv.mode".into(), opts.watermark.mode.label().to_owned()),
        KeyValue::new(
            "curv.purpose".into(),
            opts.watermark.purpose.label().to_owned(),
        ),
        KeyValue::new("curv.layout".into(), opts.layout.as_str().to_owned()),
        KeyValue::new("curv.row_count".into(), rows.len().to_string()),
    ];
    // The provenance block, one key per RUN-5 field (`curv.run_id`, `curv.definition_hash`, ...).
    let prov: Map<String, Value> = match manifest.get("provenance") {
        Some(Value::Object(m)) => m.clone(),
        _ => Map::new(),
    };
    for (k, v) in prov {
        let v = match v {
            Value::Null => continue,
            Value::String(s) => s,
            other => other.to_string(),
        };
        meta.push(KeyValue::new(format!("curv.{k}"), v));
    }
    let props = WriterProperties::builder()
        .set_writer_version(WriterVersion::PARQUET_1_0)
        .set_compression(Compression::UNCOMPRESSED)
        .set_dictionary_enabled(false)
        .set_statistics_enabled(EnabledStatistics::None)
        .set_created_by(format!("curv asd-export analysis v{ANALYSIS_VERSION}"))
        .set_key_value_metadata(Some(meta))
        .build();
    let mut writer = SerializedFileWriter::new(Vec::new(), schema, Arc::new(props)).map_err(err)?;
    let mut group = writer.next_row_group().map_err(err)?;
    let table: Vec<Vec<Cell>> = rows.iter().map(cells).collect();
    for (i, (_, numeric)) in SESSION_COLUMNS.iter().enumerate() {
        let mut column = group
            .next_column()
            .map_err(err)?
            .ok_or_else(|| ExportError::Analysis("parquet: schema has fewer columns".into()))?;
        let mut levels = Vec::with_capacity(table.len());
        if *numeric {
            let mut values = Vec::new();
            for row in &table {
                if let Cell::Num(Some(x)) = &row[i] {
                    values.push(*x);
                    levels.push(1);
                } else {
                    levels.push(0);
                }
            }
            column
                .typed::<DoubleType>()
                .write_batch(&values, Some(&levels), None)
                .map_err(err)?;
        } else {
            let mut values = Vec::new();
            for row in &table {
                if let Cell::Text(Some(s)) = &row[i] {
                    values.push(ByteArray::from(s.clone().into_bytes()));
                    levels.push(1);
                } else {
                    levels.push(0);
                }
            }
            column
                .typed::<ByteArrayType>()
                .write_batch(&values, Some(&levels), None)
                .map_err(err)?;
        }
        column.close().map_err(err)?;
    }
    group.close().map_err(err)?;
    writer.into_inner().map_err(err)
}

#[cfg(not(feature = "parquet"))]
fn parquet(_: &AnalysisOptions, _: &Value, _: &[SessionOut]) -> Result<Vec<u8>, ExportError> {
    Err(ExportError::Analysis(
        "parquet support is not compiled in".into(),
    ))
}
