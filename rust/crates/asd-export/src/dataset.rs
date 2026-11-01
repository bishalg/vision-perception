//! owner: WP-B.9
//!
//! ML dataset encoders (02 §4 `POST /jobs/export-dataset {filters, format=parquet|jsonl,
//! include_labels}`): one row per session, de-identified, with the human annotations
//! (`labels`) when asked for. Training stays in `tools/` (02 §1); this only writes the file.
//!
//! - [`encode_jsonl`]: UTF-8 JSON Lines. Line 1 is the manifest (`record_type: "manifest"`:
//!   dataset version, watermark, mandatory sentence, row count, column list); every further
//!   line is one session (`record_type: "session"`) with its clips (and their metric rows)
//!   and labels nested.
//! - [`encode_parquet`] (feature `parquet`): one flat table, one row per session. Scalar
//!   columns are typed (`DOUBLE`, `INT64`, `UTF8`, all nullable); nested values (`affect`,
//!   `clips`, `labels`) are JSON text columns. The watermark, mandatory sentence and dataset
//!   version are file key-value metadata (`curv.*`). No compression and no statistics, so
//!   the bytes depend only on the input (XC-8).
//!
//! Datasets are **de-identified only** (XC-4): the encoders refuse an identified watermark
//! and run [`crate::deid::apply`] themselves, so a caller cannot skip it. Labels keep their
//! clip, key and value; who wrote them and when are never inputs. Which sessions are in the
//! dataset is the caller's [`crate::policy`] decision (research needs research consent).

use std::collections::BTreeMap;

use asd_contract::api_v1::ClipScore;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::deid::{self, SALT_LEN};
use crate::model::ExportRecord;
use crate::policy::Mode;
use crate::watermark::{Watermark, MANDATORY_SENTENCE};
use crate::ExportError;

/// Version of the row shape; changes with any change to [`DatasetRow`].
pub const DATASET_VERSION: &str = "1";

/// One human annotation of a session (`labels` table, written by reviewers for ML).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DatasetLabel {
    /// The clip the label is about; `None` for the whole session.
    pub clip_id: Option<String>,
    pub key: String,
    pub value: Value,
}

/// Options of the dataset encoders.
#[derive(Debug, Clone)]
pub struct DatasetOptions {
    /// Must be `deidentified` (XC-4).
    pub watermark: Watermark,
    /// Per-export pseudonym salt (XC-4); the caller draws and discards it.
    pub deid_salt: [u8; SALT_LEN],
    /// Write the `labels` of each session.
    pub include_labels: bool,
}

/// One session of the dataset. Field order is the column order.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DatasetRow {
    pub export_id: String,
    /// `S-` pseudonym (XC-4).
    pub session_id: String,
    /// `P-` pseudonym, stable within this dataset only (XC-4).
    pub participant_id: String,
    /// `YYYY-MM` (UTC) of the session start.
    pub session_month: String,
    pub days_since_first_session: Option<i64>,
    pub age_bracket_at_enrol: Option<String>,
    pub battery: Option<String>,
    pub capture_source: Option<String>,
    pub age_pack: Option<String>,
    pub lang: Option<String>,
    pub schema_version: Option<String>,
    pub bundle_version: Option<String>,
    pub duration_sec: Option<f64>,
    pub quality_verdict: Option<String>,
    pub valid_frame_ratio: Option<f64>,
    pub calibrated_distance_cm: Option<f64>,
    pub tracking_verdict: Option<String>,
    pub tracking_accuracy_deg: Option<f64>,
    pub social_preference: Option<f64>,
    pub eye_dwell: Option<f64>,
    pub mouth_dwell: Option<f64>,
    pub first_eye_latency_ms: Option<f64>,
    pub composite_score: Option<f64>,
    pub composite_confidence: Option<f64>,
    pub attention_flag: Option<String>,
    /// Clinical review state (`open` .. `final`), when the session has a review.
    pub review_state: Option<String>,
    /// The clinician's disposition (a label for supervised learning), when recorded.
    pub review_disposition: Option<String>,
    /// `preliminary | final | amended` (FHX-4).
    pub review_status: Option<String>,
    pub affect: Option<BTreeMap<String, Value>>,
    pub clips: Vec<ClipScore>,
    /// `None` when labels were not asked for.
    pub labels: Option<Vec<DatasetLabel>>,
}

/// Column names in order, as written to the manifest and the Parquet schema.
pub const COLUMNS: &[&str] = &[
    "export_id",
    "session_id",
    "participant_id",
    "session_month",
    "days_since_first_session",
    "age_bracket_at_enrol",
    "battery",
    "capture_source",
    "age_pack",
    "lang",
    "schema_version",
    "bundle_version",
    "duration_sec",
    "quality_verdict",
    "valid_frame_ratio",
    "calibrated_distance_cm",
    "tracking_verdict",
    "tracking_accuracy_deg",
    "social_preference",
    "eye_dwell",
    "mouth_dwell",
    "first_eye_latency_ms",
    "composite_score",
    "composite_confidence",
    "attention_flag",
    "review_state",
    "review_disposition",
    "review_status",
    "affect",
    "clips",
    "labels",
];

/// The wire spelling of a unit enum (`truedepth`, `red`).
fn wire<T: Serialize>(v: &T) -> Option<String> {
    match serde_json::to_value(v) {
        Ok(Value::String(s)) => Some(s),
        _ => None,
    }
}

/// The dataset rows of `records` (de-identified here), labels aligned per index.
pub fn rows(
    records: &[ExportRecord],
    labels: &[Vec<DatasetLabel>],
    opts: &DatasetOptions,
) -> Result<Vec<DatasetRow>, ExportError> {
    if opts.watermark.mode != Mode::Deidentified {
        return Err(ExportError::DatasetIdentified);
    }
    if opts.include_labels && labels.len() != records.len() {
        return Err(ExportError::DatasetInput(
            "labels must be aligned with the records",
        ));
    }
    let deid = deid::apply(records, &opts.deid_salt)?;
    Ok(deid
        .into_iter()
        .enumerate()
        .map(|(i, r)| {
            let s = &r.session;
            let review = r.review.as_ref();
            DatasetRow {
                export_id: opts.watermark.export_id.clone(),
                session_id: s.summary.id.clone(),
                participant_id: s.summary.participant_id.clone(),
                // `deid::apply` reduced `started_at` to `YYYY-MM`.
                session_month: s.summary.started_at.clone(),
                days_since_first_session: r.days_since_first_session,
                age_bracket_at_enrol: r.patient.age_bracket_at_enrol.clone(),
                battery: s.summary.battery.clone(),
                capture_source: s.summary.capture_source.as_ref().and_then(wire),
                age_pack: s.age_pack.clone(),
                lang: s.lang.clone(),
                schema_version: s.schema_version.clone(),
                bundle_version: s.summary.bundle_version.clone(),
                duration_sec: Some(s.summary.duration_sec),
                quality_verdict: Some(s.summary.quality_verdict.clone()),
                valid_frame_ratio: s.valid_frame_ratio,
                calibrated_distance_cm: s.calibrated_distance_cm,
                tracking_verdict: s.summary.tracking_verdict.as_ref().and_then(wire),
                tracking_accuracy_deg: s.summary.tracking_accuracy_deg,
                social_preference: s.social_preference,
                eye_dwell: s.eye_dwell,
                mouth_dwell: s.mouth_dwell,
                first_eye_latency_ms: s.first_eye_latency_ms,
                composite_score: s.summary.composite_score,
                composite_confidence: s.summary.composite_confidence,
                attention_flag: wire(&s.summary.attention_flag),
                review_state: review.and_then(|v| wire(&v.state)),
                review_disposition: review
                    .and_then(|v| v.disposition)
                    .map(|d| d.as_str().to_owned()),
                review_status: review.map(|_| r.report_status().as_str().to_owned()),
                affect: s.affect.clone(),
                clips: r.clips.clone(),
                labels: opts
                    .include_labels
                    .then(|| labels.get(i).cloned().unwrap_or_default()),
            }
        })
        .collect())
}

#[derive(Serialize)]
struct Manifest<'a> {
    record_type: &'static str,
    dataset_version: &'static str,
    export_id: &'a str,
    watermark: String,
    notice: &'static str,
    mode: Mode,
    purpose: crate::policy::Purpose,
    row_count: usize,
    include_labels: bool,
    columns: &'static [&'static str],
}

fn manifest(opts: &DatasetOptions, row_count: usize) -> Manifest<'_> {
    Manifest {
        record_type: "manifest",
        dataset_version: DATASET_VERSION,
        export_id: &opts.watermark.export_id,
        watermark: opts.watermark.line(),
        notice: MANDATORY_SENTENCE,
        mode: opts.watermark.mode,
        purpose: opts.watermark.purpose,
        row_count,
        include_labels: opts.include_labels,
        columns: COLUMNS,
    }
}

#[derive(Serialize)]
struct SessionLine<'a> {
    record_type: &'static str,
    #[serde(flatten)]
    row: &'a DatasetRow,
}

/// JSON Lines: the manifest, then one session per line (LF after every line).
pub fn encode_jsonl(
    records: &[ExportRecord],
    labels: &[Vec<DatasetLabel>],
    opts: &DatasetOptions,
) -> Result<Vec<u8>, ExportError> {
    let rows = rows(records, labels, opts)?;
    let mut out = serde_json::to_vec(&manifest(opts, rows.len())).expect("plain data");
    out.push(b'\n');
    for row in &rows {
        serde_json::to_writer(
            &mut out,
            &SessionLine {
                record_type: "session",
                row,
            },
        )
        .expect("plain data");
        out.push(b'\n');
    }
    Ok(out)
}

#[cfg(feature = "parquet")]
pub use parquet_writer::encode_parquet;

#[cfg(feature = "parquet")]
mod parquet_writer {
    use std::sync::Arc;

    use parquet::basic::Compression;
    use parquet::data_type::{ByteArray, ByteArrayType, DoubleType, Int64Type};
    use parquet::file::metadata::KeyValue;
    use parquet::file::properties::{EnabledStatistics, WriterProperties, WriterVersion};
    use parquet::file::writer::SerializedFileWriter;
    use parquet::schema::parser::parse_message_type;

    use super::{manifest, rows, DatasetLabel, DatasetOptions, DatasetRow, COLUMNS};
    use crate::model::ExportRecord;
    use crate::watermark::MANDATORY_SENTENCE;
    use crate::ExportError;

    /// A typed column: how to read it from a row.
    enum Col {
        Text(fn(&DatasetRow) -> Option<String>),
        Double(fn(&DatasetRow) -> Option<f64>),
        Int(fn(&DatasetRow) -> Option<i64>),
    }

    fn json<T: serde::Serialize>(v: &T) -> String {
        serde_json::to_string(v).expect("plain data")
    }

    /// One entry per [`COLUMNS`] name, same order.
    fn columns() -> Vec<(&'static str, Col)> {
        use Col::{Double, Int, Text};
        vec![
            ("export_id", Text(|r| Some(r.export_id.clone()))),
            ("session_id", Text(|r| Some(r.session_id.clone()))),
            ("participant_id", Text(|r| Some(r.participant_id.clone()))),
            ("session_month", Text(|r| Some(r.session_month.clone()))),
            (
                "days_since_first_session",
                Int(|r| r.days_since_first_session),
            ),
            (
                "age_bracket_at_enrol",
                Text(|r| r.age_bracket_at_enrol.clone()),
            ),
            ("battery", Text(|r| r.battery.clone())),
            ("capture_source", Text(|r| r.capture_source.clone())),
            ("age_pack", Text(|r| r.age_pack.clone())),
            ("lang", Text(|r| r.lang.clone())),
            ("schema_version", Text(|r| r.schema_version.clone())),
            ("bundle_version", Text(|r| r.bundle_version.clone())),
            ("duration_sec", Double(|r| r.duration_sec)),
            ("quality_verdict", Text(|r| r.quality_verdict.clone())),
            ("valid_frame_ratio", Double(|r| r.valid_frame_ratio)),
            (
                "calibrated_distance_cm",
                Double(|r| r.calibrated_distance_cm),
            ),
            ("tracking_verdict", Text(|r| r.tracking_verdict.clone())),
            ("tracking_accuracy_deg", Double(|r| r.tracking_accuracy_deg)),
            ("social_preference", Double(|r| r.social_preference)),
            ("eye_dwell", Double(|r| r.eye_dwell)),
            ("mouth_dwell", Double(|r| r.mouth_dwell)),
            ("first_eye_latency_ms", Double(|r| r.first_eye_latency_ms)),
            ("composite_score", Double(|r| r.composite_score)),
            ("composite_confidence", Double(|r| r.composite_confidence)),
            ("attention_flag", Text(|r| r.attention_flag.clone())),
            ("review_state", Text(|r| r.review_state.clone())),
            ("review_disposition", Text(|r| r.review_disposition.clone())),
            ("review_status", Text(|r| r.review_status.clone())),
            ("affect", Text(|r| r.affect.as_ref().map(json))),
            ("clips", Text(|r| Some(json(&r.clips)))),
            ("labels", Text(|r| r.labels.as_ref().map(json))),
        ]
    }

    fn schema(cols: &[(&'static str, Col)]) -> String {
        let mut s = String::from("message curv_asd_dataset {\n");
        for (name, col) in cols {
            let ty = match col {
                Col::Text(_) => "binary",
                Col::Double(_) => "double",
                Col::Int(_) => "int64",
            };
            let annotation = if matches!(col, Col::Text(_)) {
                " (UTF8)"
            } else {
                ""
            };
            s.push_str(&format!("  optional {ty} {name}{annotation};\n"));
        }
        s.push('}');
        s
    }

    fn err(e: parquet::errors::ParquetError) -> ExportError {
        ExportError::Tabular(format!("parquet: {e}"))
    }

    /// Values and definition levels (1 = present, 0 = null) of one optional column.
    fn split<T>(values: impl Iterator<Item = Option<T>>) -> (Vec<T>, Vec<i16>) {
        let mut present = Vec::new();
        let mut levels = Vec::new();
        for v in values {
            match v {
                Some(v) => {
                    present.push(v);
                    levels.push(1);
                }
                None => levels.push(0),
            }
        }
        (present, levels)
    }

    /// Parquet: one row group, one row per session (see the module docs).
    pub fn encode_parquet(
        records: &[ExportRecord],
        labels: &[Vec<DatasetLabel>],
        opts: &DatasetOptions,
    ) -> Result<Vec<u8>, ExportError> {
        let rows = rows(records, labels, opts)?;
        let cols = columns();
        debug_assert!(cols.iter().map(|(n, _)| *n).eq(COLUMNS.iter().copied()));
        let schema = Arc::new(parse_message_type(&schema(&cols)).map_err(err)?);
        let m = manifest(opts, rows.len());
        let props = WriterProperties::builder()
            .set_writer_version(WriterVersion::PARQUET_1_0)
            .set_compression(Compression::UNCOMPRESSED)
            .set_dictionary_enabled(false)
            .set_statistics_enabled(EnabledStatistics::None)
            .set_created_by(format!("curv asd-export dataset v{}", m.dataset_version))
            .set_key_value_metadata(Some(vec![
                KeyValue::new("curv.dataset_version".into(), m.dataset_version.to_owned()),
                KeyValue::new("curv.export_id".into(), m.export_id.to_owned()),
                KeyValue::new("curv.watermark".into(), m.watermark.clone()),
                KeyValue::new("curv.notice".into(), MANDATORY_SENTENCE.to_owned()),
                KeyValue::new("curv.mode".into(), m.mode.label().to_owned()),
                KeyValue::new("curv.purpose".into(), m.purpose.label().to_owned()),
                KeyValue::new("curv.row_count".into(), m.row_count.to_string()),
                KeyValue::new("curv.include_labels".into(), m.include_labels.to_string()),
            ]))
            .build();
        let mut writer =
            SerializedFileWriter::new(Vec::new(), schema, Arc::new(props)).map_err(err)?;
        let mut group = writer.next_row_group().map_err(err)?;
        for (_, col) in &cols {
            let mut column = group
                .next_column()
                .map_err(err)?
                .ok_or_else(|| ExportError::Tabular("parquet: schema has fewer columns".into()))?;
            match col {
                Col::Text(get) => {
                    let (values, levels) = split(
                        rows.iter()
                            .map(|r| get(r).map(|s| ByteArray::from(s.into_bytes()))),
                    );
                    column
                        .typed::<ByteArrayType>()
                        .write_batch(&values, Some(&levels), None)
                        .map_err(err)?;
                }
                Col::Double(get) => {
                    let (values, levels) = split(rows.iter().map(get));
                    column
                        .typed::<DoubleType>()
                        .write_batch(&values, Some(&levels), None)
                        .map_err(err)?;
                }
                Col::Int(get) => {
                    let (values, levels) = split(rows.iter().map(get));
                    column
                        .typed::<Int64Type>()
                        .write_batch(&values, Some(&levels), None)
                        .map_err(err)?;
                }
            }
            column.close().map_err(err)?;
        }
        group.close().map_err(err)?;
        writer.into_inner().map_err(err)
    }
}
