//! CSV-1..4 and XC-6/XC-8 for the CSV encoders (WP-I.2). Goldens are in
//! `tests/golden/tabular/`; regenerate with `UPDATE_GOLDEN=1 cargo test -p asd-export --test csv`
//! and review the diff.

mod export_support;

use std::collections::BTreeMap;
use std::io::Read as _;
use std::path::PathBuf;

use asd_export::policy::{Mode, Purpose};
use asd_export::tabular::columns::Table;
use asd_export::tabular::csv::{encode, encode_zip};
use asd_export::tabular::{Dialect, TabularOptions};
use asd_export::watermark::{Watermark, MANDATORY_SENTENCE};
use export_support::{full_record, golden_record, sentinel};
use sha2::{Digest, Sha256};

const SALT: [u8; 32] = [9u8; 32];
const ALL_DIALECTS: [Dialect; 3] = [Dialect::Canonical, Dialect::Excel, Dialect::Sheets];

fn options(mode: Mode) -> TabularOptions {
    TabularOptions::new(
        Watermark {
            export_id: "0b9f6c1e-7a51-4c1e-9d43-2f1a7f0c9e11".into(),
            requested_by_display: "Dr. Sita Sharma".into(),
            requested_at_utc: "2026-10-31T09:15:00Z".into(),
            mode,
            purpose: if mode == Mode::Identified {
                Purpose::ClinicalCare
            } else {
                Purpose::Research
            },
            org_name: "Kathmandu Child Clinic".into(),
        },
        SALT,
    )
}

fn golden(name: &str, actual: &[u8]) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden/tabular")
        .join(name);
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, actual).unwrap();
    }
    let want = std::fs::read(&path).unwrap_or_else(|_| panic!("missing golden {name}"));
    assert_eq!(
        String::from_utf8_lossy(actual),
        String::from_utf8_lossy(&want),
        "golden {name}"
    );
    assert_eq!(actual, want.as_slice(), "golden {name} bytes");
}

/// Minimal RFC 4180 reader (quotes, doubled quotes, CR/LF/CRLF record ends).
fn parse(csv: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = csv.chars().peekable();
    while let Some(ch) = chars.next() {
        match (quoted, ch) {
            (true, '"') if chars.peek() == Some(&'"') => {
                field.push('"');
                chars.next();
            }
            (true, '"') => quoted = false,
            (true, c) => field.push(c),
            (false, '"') => quoted = true,
            (false, ',') => row.push(std::mem::take(&mut field)),
            (false, '\r') => {}
            (false, '\n') => {
                row.push(std::mem::take(&mut field));
                rows.push(std::mem::take(&mut row));
            }
            (false, c) => field.push(c),
        }
    }
    assert!(field.is_empty() && row.is_empty(), "unterminated last line");
    rows
}

/// Records of `bytes` as header-keyed maps.
fn table_rows(bytes: &[u8]) -> Vec<BTreeMap<String, String>> {
    let text = std::str::from_utf8(bytes).unwrap();
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut it = parse(text).into_iter();
    let header = it.next().unwrap();
    it.map(|r| {
        assert_eq!(r.len(), header.len(), "ragged row {r:?}");
        header.iter().cloned().zip(r).collect()
    })
    .collect()
}

fn csv_of(records: &[asd_export::model::ExportRecord], d: Dialect, t: Table) -> Vec<u8> {
    encode(records, &options(Mode::Identified), d, t).unwrap()
}

#[test]
fn header_matches_dictionary_and_export_id_is_first_everywhere() {
    let records = [full_record()];
    for d in ALL_DIALECTS {
        for t in Table::ALL {
            let bytes = csv_of(&records, d, t);
            let rows = table_rows(&bytes);
            assert!(!rows.is_empty(), "{t:?} has rows");
            let text = std::str::from_utf8(&bytes)
                .unwrap()
                .trim_start_matches('\u{feff}');
            let header: Vec<String> = parse(text).remove(0);
            let want: Vec<&str> = t.columns().iter().map(|c| c.name).collect();
            assert_eq!(header, want);
            assert_eq!(header[0], "export_id");
            for r in rows {
                assert_eq!(r["export_id"], "0b9f6c1e-7a51-4c1e-9d43-2f1a7f0c9e11");
            }
        }
    }
}

#[test]
fn bom_only_for_excel_and_crlf_only_for_excel() {
    let records = [full_record()];
    for d in ALL_DIALECTS {
        let bytes = csv_of(&records, d, Table::Sessions);
        let has_bom = bytes.starts_with(b"\xEF\xBB\xBF");
        assert_eq!(has_bom, d == Dialect::Excel, "{d:?} BOM");
        let text = std::str::from_utf8(&bytes).unwrap();
        assert!(text.ends_with(if d == Dialect::Excel { "\r\n" } else { "\n" }));
        let lf_only = text.replace("\r\n", "");
        assert_eq!(
            text.contains("\r\n"),
            d == Dialect::Excel,
            "{d:?} CRLF record ends"
        );
        if d == Dialect::Excel {
            // Every LF is part of a CRLF (no free text with newlines in sessions).
            assert!(!lf_only.contains('\n'));
        } else {
            assert!(!text.contains('\r'));
        }
    }
}

#[test]
fn formula_injection_table() {
    let mut r = full_record();
    r.session.summary.battery = Some("=1+1".into());
    r.session.lang = Some("+x".into());
    r.session.age_pack = Some("-x".into());
    r.session.summary.bundle_version = Some("@x".into());
    r.session.schema_version = Some("\tx".into());
    r.session.summary.quality_verdict = "\rx".into();
    r.session.summary.device_id = Some("safe=1".into());
    // A negative number is a number, never prefixed.
    r.session.calibrated_distance_cm = Some(-1.5);
    r.clips[0].rows[0].source = "-x".into();
    r.clips[0].rows[0].deviation = Some(-1.5);
    for d in ALL_DIALECTS {
        let s = &table_rows(&csv_of(std::slice::from_ref(&r), d, Table::Sessions))[0];
        assert_eq!(s["battery"], "'=1+1", "{d:?}");
        assert_eq!(s["lang"], "'+x");
        assert_eq!(s["age_pack"], "'-x");
        assert_eq!(s["bundle_version"], "'@x");
        assert_eq!(s["schema_version"], "'\tx");
        assert_eq!(s["quality_verdict"], "'\rx");
        assert_eq!(s["device_id"], "safe=1");
        assert_eq!(s["calibrated_distance_cm"], "-1.5");
        let m = &table_rows(&csv_of(std::slice::from_ref(&r), d, Table::MetricRows))[0];
        assert_eq!(m["source"], "'-x");
        assert_eq!(m["deviation"], "-1.5");
    }
}

#[test]
fn free_text_is_quoted_and_guarded_in_reviews() {
    let mut r = full_record();
    r.review.as_mut().unwrap().impression = Some("=cmd|' /C calc'!A0,\n\"quoted\"".into());
    let rows = table_rows(&csv_of(&[r], Dialect::Excel, Table::Reviews));
    assert_eq!(rows[0]["impression"], "'=cmd|' /C calc'!A0,\n\"quoted\"");
    assert_eq!(
        rows[0]["signers"],
        "Dr. SIGNER-SENTINEL; Dr. COSIGNER-SENTINEL"
    );
}

#[test]
fn org_time_zone_and_offset_column() {
    let mut r = full_record();
    r.session.summary.started_at = "2026-10-01T06:00:00.000Z".into();
    let records = [r];
    let canon = &table_rows(&csv_of(&records, Dialect::Canonical, Table::Sessions))[0];
    assert_eq!(canon["started_at"], "2026-10-01T06:00:00.000Z");
    assert_eq!(canon["utc_offset"], "+00:00");
    for d in [Dialect::Excel, Dialect::Sheets] {
        let s = &table_rows(&csv_of(&records, d, Table::Sessions))[0];
        assert_eq!(s["started_at"], "2026-10-01 11:45:00", "{d:?}");
        assert_eq!(
            s["utc_offset"], "+05:45",
            "{d:?}: offset must not be guarded"
        );
    }
    let rev = &table_rows(&csv_of(&records, Dialect::Excel, Table::Reviews))[0];
    assert_eq!(rev["signed_at"], "2026-10-02 14:15:00");
    assert_eq!(rev["utc_offset"], "+05:45");
}

#[test]
fn unknown_time_zone_is_an_error_not_utc() {
    let mut r = golden_record();
    r.org.timezone = "Mars/Olympus".into();
    let e = encode(
        &[r],
        &options(Mode::Identified),
        Dialect::Excel,
        Table::Sessions,
    );
    assert!(matches!(
        e,
        Err(asd_export::ExportError::InvalidTimezone(_))
    ));
}

#[test]
fn absent_values_are_empty_cells_and_numbers_use_dot() {
    let mut r = golden_record();
    r.session.summary.composite_score = None;
    r.session.summary.battery = None;
    r.session.first_eye_latency_ms = Some(f64::NAN);
    r.session.eye_dwell = Some(0.25);
    let s = &table_rows(&csv_of(&[r], Dialect::Canonical, Table::Sessions))[0];
    assert_eq!(s["composite_score"], "");
    assert_eq!(s["battery"], "");
    assert_eq!(s["first_eye_latency_ms"], "", "NaN is absent, not 'NaN'");
    assert_eq!(s["mrn"], "");
    assert_eq!(s["review_state"], "");
    assert_eq!(s["eye_dwell"], "0.25");
}

#[test]
fn booleans_per_dialect() {
    let records = [full_record()];
    let want = [
        (Dialect::Canonical, "true"),
        (Dialect::Excel, "true"),
        (Dialect::Sheets, "TRUE"),
    ];
    for (d, v) in want {
        let rev = &table_rows(&csv_of(&records, d, Table::Reviews))[0];
        assert_eq!(rev["amended"], v, "{d:?}");
    }
}

#[test]
fn deidentified_mode_is_applied_by_the_encoder() {
    let records = [full_record()];
    let opts = options(Mode::Deidentified);
    let mut all = Vec::new();
    for d in ALL_DIALECTS {
        for t in Table::ALL {
            all.extend(encode(&records, &opts, d, t).unwrap());
        }
        all.extend(encode_zip(&records, &opts, d).unwrap());
    }
    // The ZIP is deflated, so also check the plain CSV and the unpacked manifest below.
    let text = String::from_utf8_lossy(&all);
    for s in [
        sentinel::MRN,
        sentinel::AUTHORITY,
        sentinel::IMPRESSION,
        sentinel::RECOMMENDATIONS,
        sentinel::SIGNER,
        sentinel::COSIGNER,
        sentinel::REGISTRATION,
        sentinel::CONTENT_HASH,
        export_support::DEVICE_ID,
        export_support::PARTICIPANT_ID,
    ] {
        assert!(!text.contains(s), "{s} leaked");
    }
    let s = &table_rows(&encode(&records, &opts, Dialect::Excel, Table::Sessions).unwrap())[0];
    assert_eq!(s["started_at"], "2026-10");
    assert_eq!(s["utc_offset"], "");
    assert_eq!(s["days_since_first_session"], "0");
    assert!(s["session_id"].starts_with("S-") && s["participant_token"].starts_with("P-"));
    assert_eq!(s["device_id"], "");
}

fn unzip(bytes: &[u8]) -> Vec<(String, Vec<u8>, zip::DateTime)> {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    (0..z.len())
        .map(|i| {
            let mut f = z.by_index(i).unwrap();
            let mut buf = Vec::new();
            f.read_to_end(&mut buf).unwrap();
            (
                f.name().unwrap().to_string(),
                buf,
                f.last_modified().unwrap(),
            )
        })
        .collect()
}

#[test]
fn zip_has_five_files_and_manifest_hashes_match_file_bytes() {
    let records = [full_record()];
    let bytes = encode_zip(&records, &options(Mode::Identified), Dialect::Excel).unwrap();
    let files = unzip(&bytes);
    let names: Vec<&str> = files.iter().map(|f| f.0.as_str()).collect();
    assert_eq!(
        names,
        [
            "sessions.csv",
            "clips.csv",
            "metric_rows.csv",
            "reviews.csv",
            "manifest.json"
        ]
    );
    for (_, _, t) in &files {
        assert_eq!(*t, zip::DateTime::default(), "fixed ZIP timestamps");
    }
    let manifest: serde_json::Value = serde_json::from_slice(&files[4].1).unwrap();
    assert_eq!(
        manifest["export_id"],
        "0b9f6c1e-7a51-4c1e-9d43-2f1a7f0c9e11"
    );
    assert_eq!(manifest["dialect"], "csv_excel");
    assert_eq!(manifest["notice"], MANDATORY_SENTENCE);
    assert!(manifest["watermark_line"]
        .as_str()
        .unwrap()
        .contains("Kathmandu Child Clinic"));
    assert_eq!(manifest["watermark"]["mode"], "identified");
    let tables = manifest["tables"].as_array().unwrap();
    assert_eq!(tables.len(), 4);
    for (t, (name, data, _)) in tables.iter().zip(&files) {
        assert_eq!(t["file"], name.as_str());
        let hash: String = Sha256::digest(data)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        assert_eq!(t["sha256"], hash.as_str(), "{name}");
        let rows = table_rows(data).len();
        assert_eq!(t["rows"], rows as u64, "{name}");
        let cols: Vec<&str> = t["columns"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["name"].as_str().unwrap())
            .collect();
        assert_eq!(cols[0], "export_id");
        assert!(t["columns"][0]["description"].as_str().unwrap().len() > 3);
    }
    // The excel files in the ZIP keep their BOM.
    assert!(files[0].1.starts_with(b"\xEF\xBB\xBF"));
}

#[test]
fn output_is_byte_identical_for_identical_input() {
    let records = [full_record()];
    for mode in [Mode::Identified, Mode::Deidentified] {
        let o = options(mode);
        for d in ALL_DIALECTS {
            assert_eq!(
                encode_zip(&records, &o, d).unwrap(),
                encode_zip(&records, &o, d).unwrap(),
                "{mode:?} {d:?}"
            );
        }
    }
}

#[test]
fn goldens() {
    let records = [full_record()];
    golden(
        "sessions.csv",
        &csv_of(&records, Dialect::Canonical, Table::Sessions),
    );
    golden(
        "sessions_excel.csv",
        &csv_of(&records, Dialect::Excel, Table::Sessions),
    );
    golden(
        "sessions_sheets.csv",
        &csv_of(&records, Dialect::Sheets, Table::Sessions),
    );
    golden(
        "clips.csv",
        &csv_of(&records, Dialect::Canonical, Table::Clips),
    );
    golden(
        "metric_rows.csv",
        &csv_of(&records, Dialect::Canonical, Table::MetricRows),
    );
    golden(
        "reviews.csv",
        &csv_of(&records, Dialect::Canonical, Table::Reviews),
    );
    golden(
        "reviews_excel.csv",
        &csv_of(&records, Dialect::Excel, Table::Reviews),
    );
    let zip = encode_zip(&records, &options(Mode::Identified), Dialect::Canonical).unwrap();
    golden("manifest.json", &unzip(&zip).pop().unwrap().1);
}
