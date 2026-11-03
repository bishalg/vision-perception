//! WP-X.8 `analysis_results` encoders (RUN-7, PRV-3, PRV-5): both layouts in every encoding
//! scan clean of identifiers, suppressed cells carry no numbers, the manifest provenance is
//! the run record, Parquet metadata `curv.*` carries `run_id` and `definition_hash`, and no
//! export format reads the research frame log.
#![cfg(feature = "parquet")]

use std::io::Read as _;

use asd_contract::api_v1::SessionAttentionFlag;
use asd_contract::export::{AnalysisEncoding, AnalysisLayout, ExportFormat};
use asd_contract::research::{
    AnalysisSessionRow, BandSummary, Dist, DistComparison, Level, RunSummary, SessionDelta,
    SessionSide,
};
use asd_export::analysis::{
    aggregate_rows, encode, AnalysisOptions, Provenance, SessionInput, AGGREGATE_COLUMNS,
};
use asd_export::policy::{Mode, Purpose};
use asd_export::watermark::{Watermark, MANDATORY_SENTENCE};
use asd_export::ExportError;
use parquet::file::reader::{FileReader, SerializedFileReader};
use serde_json::Value;

const RUN_ID: &str = "0b9d5a52-6a7e-4f0c-9d1e-3b1a2c4d5e6f";
const DEF_HASH: &str = "ab12cd34ef56ab12cd34ef56ab12cd34ef56ab12cd34ef56ab12cd34ef56ab12";
const SECRET_SESSION: &str = "SESS-SECRET-001";
const SECRET_DEVICE: &str = "7c1f8a90-aaaa-4bbb-8ccc-0123456789ab";

fn provenance() -> Provenance {
    Provenance {
        run_id: RUN_ID.into(),
        kind: "cohort_rescore".into(),
        definition_hash: DEF_HASH.into(),
        cohort_hash: "c0".repeat(16),
        membership_hash: Some("m1".repeat(16)),
        profile_hash: "p2".repeat(16),
        published_profile_version: "1.2.0-pub".into(),
        baselines_version: "b-7".into(),
        catalog_version: "cat-3".into(),
        bundle_version: Some("bundle-9".into()),
        core_version: "0.9.1".into(),
        contract_version: "1.2.0".into(),
        pipeline_version: "gaze-4".into(),
        registry_version: "reg-2".into(),
        seed: 42,
        level_requested: "auto".into(),
        sessions_n: Some(21),
        finished_at: Some("2026-11-02T10:00:00Z".into()),
    }
}

fn dist(n: u32, mean: f64) -> Dist {
    Dist {
        n: Some(n),
        suppressed: false,
        mean: Some(mean),
        sd: Some(0.11),
        median: Some(mean),
        iqr: Some([mean - 0.1, mean + 0.1]),
        p10: Some(mean - 0.2),
        p90: Some(mean + 0.2),
        ci95_mean: Some([mean - 0.05, mean + 0.05]),
        hist: None,
    }
}

fn cmp(n: u32, mean: f64) -> DistComparison {
    DistComparison {
        published: dist(n, mean),
        draft: dist(n, mean + 0.01),
        delta: dist(n, 0.01),
    }
}

/// A suppressed cell that (wrongly) still carries a number: the encoder must blank it.
fn leaky_suppressed() -> Dist {
    Dist {
        suppressed: true,
        mean: Some(0.777_777),
        n: Some(3),
        ..Dist::default()
    }
}

fn summary() -> RunSummary {
    let mut ok = BandSummary {
        band: "24-36".into(),
        n: Some(21),
        ..BandSummary::default()
    };
    ok.metrics.insert("eye_dwell".into(), cmp(21, 0.5));
    ok.composite = cmp(21, 0.6);
    let mut hidden = BandSummary {
        band: "36-48".into(),
        n: None,
        suppressed: true,
        ..BandSummary::default()
    };
    hidden.metrics.insert(
        "eye_dwell".into(),
        DistComparison {
            published: leaky_suppressed(),
            draft: leaky_suppressed(),
            delta: leaky_suppressed(),
        },
    );
    hidden.composite = DistComparison {
        published: leaky_suppressed(),
        draft: leaky_suppressed(),
        delta: leaky_suppressed(),
    };
    let mut overall = BandSummary {
        band: "overall".into(),
        n: Some(21),
        ..BandSummary::default()
    };
    overall.composite = cmp(21, 0.6);
    RunSummary {
        by_age_band: vec![ok, hidden],
        overall,
        small_cell_k: 12,
        ..RunSummary::default()
    }
}

fn session(id: &str, device: Option<&str>) -> SessionInput {
    let side = |c: f64| SessionSide {
        composite: Some(c),
        flag: Some(SessionAttentionFlag::Yellow),
        quality_status: Some("ok".into()),
        metrics: [("eye_dwell".to_owned(), Some(0.5))].into(),
    };
    SessionInput {
        row: AnalysisSessionRow {
            session_id: id.into(),
            level: Level::Score,
            age_months: Some(30),
            age_band: Some("24-36".into()),
            device_id: device.map(str::to_owned),
            published: side(0.6),
            draft: side(0.65),
            delta: SessionDelta {
                composite: Some(0.05),
                metrics: [("eye_dwell".to_owned(), Some(0.0))].into(),
            },
            warnings: vec![format!("note about {id}"), "param not applied".into()],
        },
        session_month: Some("2026-10".into()),
    }
}

fn sessions() -> Vec<SessionInput> {
    vec![
        session(SECRET_SESSION, Some(SECRET_DEVICE)),
        session("SESS-SECRET-002", None),
    ]
}

fn options(layout: AnalysisLayout, encoding: AnalysisEncoding, mode: Mode) -> AnalysisOptions {
    AnalysisOptions {
        watermark: Watermark {
            export_id: "5d1c2b3a-4e5f-4a6b-8c7d-9e0f1a2b3c4d".into(),
            requested_by_display: "Researcher Ram".into(),
            requested_at_utc: "2026-11-03T09:00:00Z".into(),
            mode,
            purpose: Purpose::Research,
            org_name: "Kathmandu Child Clinic".into(),
        },
        deid_salt: [5u8; 32],
        layout,
        encoding,
        provenance: provenance(),
    }
}

fn build(layout: AnalysisLayout, encoding: AnalysisEncoding) -> Vec<u8> {
    encode(
        &options(layout, encoding, Mode::Deidentified),
        &summary(),
        &sessions(),
    )
    .expect("encodes")
    .bytes
}

fn unzip(bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("zip");
    (0..z.len())
        .map(|i| {
            let mut f = z.by_index(i).unwrap();
            let mut data = Vec::new();
            f.read_to_end(&mut data).unwrap();
            (f.name().unwrap().to_string(), data)
        })
        .collect()
}

/// Every text the artifact holds, whatever the layout.
fn texts(layout: AnalysisLayout, encoding: AnalysisEncoding) -> Vec<(String, String)> {
    let bytes = build(layout, encoding);
    match encoding {
        AnalysisEncoding::Csv => unzip(&bytes)
            .into_iter()
            .map(|(n, d)| (n, String::from_utf8(d).unwrap()))
            .collect(),
        AnalysisEncoding::Jsonl => vec![("file".into(), String::from_utf8(bytes).unwrap())],
        AnalysisEncoding::Parquet => vec![(
            "file".into(),
            // Column data is stored uncompressed; lossy text is enough for a substring scan.
            String::from_utf8_lossy(&bytes).into_owned(),
        )],
    }
}

const FORBIDDEN: &[&str] = &[
    SECRET_SESSION,
    "SESS-SECRET-002",
    SECRET_DEVICE,
    "TOKEN-",
    "MRN",
    "0.777777",
];

fn combos() -> Vec<(AnalysisLayout, AnalysisEncoding)> {
    vec![
        (AnalysisLayout::Aggregates, AnalysisEncoding::Jsonl),
        (AnalysisLayout::Aggregates, AnalysisEncoding::Csv),
        (AnalysisLayout::Sessions, AnalysisEncoding::Jsonl),
        (AnalysisLayout::Sessions, AnalysisEncoding::Csv),
        (AnalysisLayout::Sessions, AnalysisEncoding::Parquet),
    ]
}

#[test]
fn every_layout_and_encoding_scans_clean_of_identifiers() {
    for (layout, enc) in combos() {
        for (name, text) in texts(layout, enc) {
            for bad in FORBIDDEN {
                assert!(
                    !text.contains(bad),
                    "{layout:?}/{enc:?} {name} contains {bad}"
                );
            }
        }
    }
    // The pseudonyms are in: S- and D- prefixes, 10 base32 characters.
    let jsonl = &texts(AnalysisLayout::Sessions, AnalysisEncoding::Jsonl)[0].1;
    let first: Value = serde_json::from_str(jsonl.lines().nth(1).unwrap()).unwrap();
    let id = first["session_id"].as_str().unwrap();
    assert!(id.starts_with("S-") && id.len() == 12, "{id}");
    assert!(jsonl.contains("\"site_pseudonym\":\"D-"));
    assert!(jsonl.contains("\"session_month\":\"2026-10\""));
    assert!(
        !jsonl.contains("age_months"),
        "age_months is dropped for the band"
    );
    assert!(jsonl.contains("param not applied"));
}

#[test]
fn suppressed_cells_carry_no_numbers() {
    let rows = aggregate_rows(RUN_ID, &summary());
    let hidden: Vec<_> = rows.iter().filter(|r| r.band == "36-48").collect();
    assert_eq!(hidden.len(), 6, "two quantities x three sides");
    for r in hidden {
        assert!(r.suppressed);
        assert!(
            r.n.is_none()
                && r.mean.is_none()
                && r.sd.is_none()
                && r.median.is_none()
                && r.q1.is_none()
                && r.q3.is_none()
                && r.p10.is_none()
                && r.p90.is_none()
                && r.ci_lo.is_none()
                && r.ci_hi.is_none()
        );
    }
    // In the files: the CSV row has only empty cells between the keys.
    let csv = &texts(AnalysisLayout::Aggregates, AnalysisEncoding::Csv)
        .into_iter()
        .find(|(n, _)| n == "aggregates.csv")
        .unwrap()
        .1;
    assert!(csv.starts_with(&AGGREGATE_COLUMNS.join(",")));
    assert!(csv.contains(&format!(
        "{RUN_ID},36-48,composite,draft,,,,,,,,,,,true\r\n"
    )));
    assert!(csv.contains(&format!(
        "{RUN_ID},36-48,eye_dwell,published,,,,,,,,,,,true\r\n"
    )));
    assert!(!csv.contains("0.777"));
    // A visible band keeps its numbers.
    assert!(csv.contains(&format!(
        "{RUN_ID},24-36,composite,published,21,0.6,0.11,0.6,0.5,0.7,"
    )));
}

fn manifest_of(layout: AnalysisLayout, enc: AnalysisEncoding) -> Value {
    let (_, text) = texts(layout, enc)
        .into_iter()
        .find(|(n, _)| n == "manifest.json" || n == "file")
        .unwrap();
    let first = text.lines().next().unwrap();
    if enc == AnalysisEncoding::Csv {
        serde_json::from_str(&text).unwrap()
    } else {
        serde_json::from_str(first).unwrap()
    }
}

#[test]
fn manifest_provenance_equals_the_run_record() {
    let want = serde_json::to_value(provenance()).unwrap();
    for (layout, enc) in combos()
        .into_iter()
        .filter(|(_, e)| *e != AnalysisEncoding::Parquet)
    {
        let m = manifest_of(layout, enc);
        assert_eq!(m["provenance"], want, "{layout:?}/{enc:?}");
        assert_eq!(m["notice"], MANDATORY_SENTENCE);
        assert_eq!(m["mode"], "deidentified");
        assert_eq!(m["small_cell_k"], 12);
        assert_eq!(m["record_type"], "manifest");
    }
    // Every RUN-5 field is in the block.
    for k in [
        "definition_hash",
        "cohort_hash",
        "membership_hash",
        "profile_hash",
        "published_profile_version",
        "baselines_version",
        "catalog_version",
        "bundle_version",
        "core_version",
        "contract_version",
        "pipeline_version",
        "registry_version",
        "seed",
    ] {
        assert!(want.get(k).is_some(), "{k}");
    }
    // The ZIP carries the README with the notice.
    let readme = texts(AnalysisLayout::Sessions, AnalysisEncoding::Csv)
        .into_iter()
        .find(|(n, _)| n == "README.txt")
        .unwrap()
        .1;
    assert!(readme.contains(MANDATORY_SENTENCE));
    assert!(readme.contains(&format!("definition_hash: {DEF_HASH}")));
}

#[test]
fn parquet_metadata_carries_run_id_and_definition_hash() {
    let bytes = build(AnalysisLayout::Sessions, AnalysisEncoding::Parquet);
    let reader = SerializedFileReader::new(bytes::Bytes::from(bytes)).unwrap();
    let meta = reader.metadata().file_metadata();
    let kv: std::collections::BTreeMap<String, String> = meta
        .key_value_metadata()
        .unwrap()
        .iter()
        .map(|k| (k.key.clone(), k.value.clone().unwrap_or_default()))
        .collect();
    assert_eq!(kv["curv.run_id"], RUN_ID);
    assert_eq!(kv["curv.definition_hash"], DEF_HASH);
    assert_eq!(kv["curv.seed"], "42");
    assert_eq!(kv["curv.notice"], MANDATORY_SENTENCE);
    assert_eq!(kv["curv.mode"], "de-identified");
    assert_eq!(kv["curv.row_count"], "2");
    assert_eq!(meta.num_rows(), 2);
}

#[test]
fn refusals_and_determinism() {
    let err = encode(
        &options(
            AnalysisLayout::Sessions,
            AnalysisEncoding::Jsonl,
            Mode::Identified,
        ),
        &summary(),
        &sessions(),
    )
    .unwrap_err();
    assert!(matches!(err, ExportError::DatasetIdentified));
    let err = encode(
        &options(
            AnalysisLayout::Aggregates,
            AnalysisEncoding::Parquet,
            Mode::Deidentified,
        ),
        &summary(),
        &[],
    )
    .unwrap_err();
    assert!(matches!(err, ExportError::Analysis(_)));
    // Same input, same bytes; input order does not matter.
    for (layout, enc) in combos() {
        assert_eq!(build(layout, enc), build(layout, enc));
    }
    let mut reversed = sessions();
    reversed.reverse();
    let o = options(
        AnalysisLayout::Sessions,
        AnalysisEncoding::Jsonl,
        Mode::Deidentified,
    );
    assert_eq!(
        encode(&o, &summary(), &sessions()).unwrap().bytes,
        encode(&o, &summary(), &reversed).unwrap().bytes
    );
}

/// PRV-5: no export format reads the research frame log, and this crate has no encoder for it.
#[test]
fn export_format_has_no_research_log_reader() {
    let names: Vec<&str> = ExportFormat::ALL.iter().map(|f| f.as_str()).collect();
    assert_eq!(
        names,
        [
            "csv",
            "csv_excel",
            "csv_sheets",
            "xlsx",
            "pdf_summary",
            "fhir_bundle",
            "fhir_ndjson",
            "hl7v2",
            "dataset_jsonl",
            "dataset_parquet",
            "case_package",
            "audit_csv",
            "analysis_results",
        ],
        "a new format must be reviewed against PRV-5 and added here"
    );
    assert!(names
        .iter()
        .all(|n| !n.contains("research") && !n.contains("frames")));
    // Nothing in the encoder sources names the log's storage column.
    let src = include_str!("../src/analysis.rs");
    assert!(!src.contains(&["research", "_frames", "_blob_key"].concat()));
}
