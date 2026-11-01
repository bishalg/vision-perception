//! WP-B.9 dataset encoders: JSON Lines and Parquet carry the same de-identified rows, the
//! watermark and the mandatory sentence (XC-6), labels only when asked for, no identifier
//! of the de-identification list (XC-4), and identical bytes for identical input (XC-8).

mod export_support;

use asd_export::dataset::{
    encode_jsonl, encode_parquet, DatasetLabel, DatasetOptions, COLUMNS, DATASET_VERSION,
};
use asd_export::policy::{Mode, Purpose};
use asd_export::watermark::{Watermark, MANDATORY_SENTENCE};
use asd_export::ExportError;
use export_support::{full_record, later_session, sentinel};
use parquet::file::reader::{FileReader, SerializedFileReader};
use parquet::record::Field;
use serde_json::{json, Value};

const SALT: [u8; 32] = [3u8; 32];
const EXPORT_ID: &str = "5d1c2b3a-4e5f-4a6b-8c7d-9e0f1a2b3c4d";

fn options(mode: Mode, include_labels: bool) -> DatasetOptions {
    DatasetOptions {
        watermark: Watermark {
            export_id: EXPORT_ID.into(),
            requested_by_display: "Researcher Ram".into(),
            requested_at_utc: "2026-11-01T09:00:00Z".into(),
            mode,
            purpose: Purpose::Research,
            org_name: "Kathmandu Child Clinic".into(),
        },
        deid_salt: SALT,
        include_labels,
    }
}

fn labels() -> Vec<Vec<DatasetLabel>> {
    vec![
        vec![
            DatasetLabel {
                clip_id: None,
                key: "gold_standard".into(),
                value: json!("typical"),
            },
            DatasetLabel {
                clip_id: Some("clip-1".into()),
                key: "usable".into(),
                value: json!(true),
            },
        ],
        vec![],
    ]
}

fn records() -> Vec<asd_export::model::ExportRecord> {
    vec![full_record(), later_session(9, "SESS-LATER")]
}

fn identifiers() -> Vec<String> {
    let r = full_record();
    vec![
        sentinel::MRN.into(),
        sentinel::IMPRESSION.into(),
        sentinel::RECOMMENDATIONS.into(),
        sentinel::NOTE.into(),
        sentinel::LETTER_BODY.into(),
        sentinel::SIGNER.into(),
        sentinel::REGISTRATION.into(),
        sentinel::SIGNER_ID.into(),
        sentinel::CONTENT_HASH.into(),
        r.patient.participant_id.clone(),
        r.patient.participant_token.clone(),
        r.session.summary.id.clone(),
        "SESS-LATER".into(),
        r.session.summary.started_at.clone(),
    ]
}

#[test]
fn jsonl_has_a_manifest_then_one_deidentified_session_per_line() {
    let bytes = encode_jsonl(&records(), &labels(), &options(Mode::Deidentified, true)).unwrap();
    let text = String::from_utf8(bytes.clone()).unwrap();
    assert!(text.ends_with('\n'));
    let lines: Vec<Value> = text
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(lines.len(), 3);

    let manifest = &lines[0];
    assert_eq!(manifest["record_type"], "manifest");
    assert_eq!(manifest["dataset_version"], DATASET_VERSION);
    assert_eq!(manifest["notice"], MANDATORY_SENTENCE);
    assert_eq!(manifest["row_count"], 2);
    assert_eq!(manifest["include_labels"], true);
    assert_eq!(manifest["mode"], "deidentified");
    assert!(manifest["watermark"].as_str().unwrap().contains(EXPORT_ID));
    assert_eq!(manifest["columns"].as_array().unwrap().len(), COLUMNS.len());

    let first = &lines[1];
    assert_eq!(first["record_type"], "session");
    for column in COLUMNS {
        assert!(first.get(*column).is_some(), "column {column} missing");
    }
    assert!(first["session_id"].as_str().unwrap().starts_with("S-"));
    assert!(first["participant_id"].as_str().unwrap().starts_with("P-"));
    assert_eq!(first["participant_id"], lines[2]["participant_id"]);
    assert_eq!(first["days_since_first_session"], 0);
    assert_eq!(lines[2]["days_since_first_session"], 9);
    assert_eq!(first["review_disposition"], "monitor");
    assert_eq!(first["review_status"], "amended");
    assert_eq!(first["labels"][0]["key"], "gold_standard");
    assert_eq!(first["labels"][1]["clip_id"], "clip-1");
    assert_eq!(lines[2]["labels"], json!([]));
    assert!(!first["clips"].as_array().unwrap().is_empty());

    for secret in identifiers() {
        assert!(!text.contains(&secret), "dataset contains {secret}");
    }
    // XC-8: same input, same bytes.
    assert_eq!(
        encode_jsonl(&records(), &labels(), &options(Mode::Deidentified, true)).unwrap(),
        bytes
    );
}

#[test]
fn labels_are_left_out_unless_asked_for() {
    let text = String::from_utf8(
        encode_jsonl(&records(), &[], &options(Mode::Deidentified, false)).unwrap(),
    )
    .unwrap();
    let session: Value = serde_json::from_str(text.lines().nth(1).unwrap()).unwrap();
    assert_eq!(session["labels"], Value::Null);
    assert!(!text.contains("gold_standard"));
}

#[test]
fn identified_datasets_and_misaligned_labels_are_refused() {
    assert!(matches!(
        encode_jsonl(&records(), &labels(), &options(Mode::Identified, true)),
        Err(ExportError::DatasetIdentified)
    ));
    assert!(matches!(
        encode_parquet(&records(), &labels(), &options(Mode::Identified, true)),
        Err(ExportError::DatasetIdentified)
    ));
    assert!(matches!(
        encode_jsonl(
            &records(),
            &labels()[..1],
            &options(Mode::Deidentified, true)
        ),
        Err(ExportError::DatasetInput(_))
    ));
}

#[test]
fn parquet_reads_back_with_the_same_rows_and_metadata() {
    let opts = options(Mode::Deidentified, true);
    let bytes = encode_parquet(&records(), &labels(), &opts).unwrap();
    assert_eq!(&bytes[..4], b"PAR1");
    assert_eq!(encode_parquet(&records(), &labels(), &opts).unwrap(), bytes);

    let reader = SerializedFileReader::new(bytes::Bytes::from(bytes.clone())).unwrap();
    let meta = reader.metadata().file_metadata();
    assert_eq!(meta.num_rows(), 2);
    let names: Vec<_> = meta
        .schema_descr()
        .columns()
        .iter()
        .map(|c| c.name().to_owned())
        .collect();
    assert_eq!(names, COLUMNS);
    let kv: std::collections::BTreeMap<_, _> = meta
        .key_value_metadata()
        .unwrap()
        .iter()
        .map(|k| (k.key.clone(), k.value.clone().unwrap_or_default()))
        .collect();
    assert_eq!(kv["curv.notice"], MANDATORY_SENTENCE);
    assert_eq!(kv["curv.dataset_version"], DATASET_VERSION);
    assert!(kv["curv.watermark"].contains(EXPORT_ID));

    // The Parquet rows hold the JSON Lines rows (nested values as JSON text).
    let jsonl = String::from_utf8(encode_jsonl(&records(), &labels(), &opts).unwrap()).unwrap();
    let want: Vec<Value> = jsonl
        .lines()
        .skip(1)
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let rows: Vec<_> = reader
        .get_row_iter(None)
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(rows.len(), 2);
    for (row, want) in rows.iter().zip(&want) {
        for (name, field) in row.get_column_iter() {
            let expected = &want[name.as_str()];
            match field {
                Field::Null => assert_eq!(expected, &Value::Null, "{name}"),
                Field::Str(s) => {
                    if matches!(name.as_str(), "affect" | "clips" | "labels") {
                        assert_eq!(
                            &serde_json::from_str::<Value>(s).unwrap(),
                            expected,
                            "{name}"
                        );
                    } else {
                        assert_eq!(expected, &json!(s), "{name}");
                    }
                }
                Field::Double(d) => assert_eq!(expected.as_f64(), Some(*d), "{name}"),
                Field::Long(n) => assert_eq!(expected.as_i64(), Some(*n), "{name}"),
                other => panic!("{name}: unexpected {other:?}"),
            }
        }
    }

    let raw = String::from_utf8_lossy(&bytes);
    for secret in identifiers() {
        assert!(!raw.contains(&secret), "parquet contains {secret}");
    }
}
