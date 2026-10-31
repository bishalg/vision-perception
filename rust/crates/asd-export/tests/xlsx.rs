//! XLSX-1..3 (WP-I.2). The workbook is read back with `calamine`; no binary golden.

mod export_support;

use std::io::{Cursor, Read as _};

use asd_export::policy::{Mode, Purpose};
use asd_export::tabular::columns::Table;
use asd_export::tabular::xlsx::encode;
use asd_export::tabular::{TabularOptions, ROW_LIMIT};
use asd_export::watermark::{Watermark, MANDATORY_SENTENCE};
use asd_export::ExportError;
use calamine::{Data, Reader, Xlsx};
use export_support::{full_record, golden_record, sentinel};

fn options(mode: Mode) -> TabularOptions {
    TabularOptions::new(
        Watermark {
            export_id: "0b9f6c1e-7a51-4c1e-9d43-2f1a7f0c9e11".into(),
            requested_by_display: "Dr. Sita & Sharma".into(),
            requested_at_utc: "2026-10-31T09:15:00Z".into(),
            mode,
            purpose: if mode == Mode::Identified {
                Purpose::ClinicalCare
            } else {
                Purpose::Research
            },
            org_name: "Kathmandu Child Clinic".into(),
        },
        [9u8; 32],
    )
}

fn open(bytes: &[u8]) -> Xlsx<Cursor<Vec<u8>>> {
    calamine::open_workbook_from_rs(Cursor::new(bytes.to_vec())).unwrap()
}

fn part(bytes: &[u8], name: &str) -> String {
    let mut z = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut s = String::new();
    z.by_name(name).unwrap().read_to_string(&mut s).unwrap();
    s
}

fn sheet(book: &mut Xlsx<Cursor<Vec<u8>>>, name: &str) -> Vec<Vec<Data>> {
    book.worksheet_range(name)
        .unwrap()
        .rows()
        .map(<[Data]>::to_vec)
        .collect()
}

fn col(table: Table, name: &str) -> usize {
    table.columns().iter().position(|c| c.name == name).unwrap()
}

#[test]
fn sheets_and_headers_follow_the_dictionary() {
    let bytes = encode(&[full_record()], &options(Mode::Identified)).unwrap();
    let mut book = open(&bytes);
    assert_eq!(
        book.sheet_names(),
        ["README", "Sessions", "Clips", "MetricRows", "Reviews"]
    );
    for t in Table::ALL {
        let rows = sheet(&mut book, t.sheet_name());
        let header: Vec<String> = rows[0].iter().map(ToString::to_string).collect();
        let want: Vec<&str> = t.columns().iter().map(|c| c.name).collect();
        assert_eq!(header, want, "{t:?}");
        assert!(rows.len() > 1);
        for r in &rows[1..] {
            assert_eq!(
                r[0],
                Data::String("0b9f6c1e-7a51-4c1e-9d43-2f1a7f0c9e11".into())
            );
        }
    }
}

#[test]
fn readme_has_watermark_sentence_and_dictionary() {
    let o = options(Mode::Identified);
    let bytes = encode(&[full_record()], &o).unwrap();
    let rows = sheet(&mut open(&bytes), "README");
    let cells: Vec<String> = rows.iter().flatten().map(ToString::to_string).collect();
    assert!(cells.contains(&o.watermark.line()));
    assert!(cells.iter().any(|c| c == MANDATORY_SENTENCE));
    for t in Table::ALL {
        for c in t.columns() {
            assert!(cells.iter().any(|x| x == c.name), "{} in README", c.name);
            assert!(cells.iter().any(|x| x == c.description));
        }
    }
}

#[test]
fn watermark_in_properties_and_printed_header_footer() {
    let o = options(Mode::Identified);
    let bytes = encode(&[full_record()], &o).unwrap();
    let core = part(&bytes, "docProps/core.xml");
    assert!(core.contains("<dc:title>Export 0b9f6c1e-7a51-4c1e-9d43-2f1a7f0c9e11</dc:title>"));
    // `&` is XML-escaped in the properties.
    assert!(
        core.contains("requested by Dr. Sita &amp; Sharma"),
        "{core}"
    );
    assert!(core.contains(MANDATORY_SENTENCE));
    // Creation time is the request time, not the clock.
    assert!(core.contains("2026-10-31T09:15:00Z"), "{core}");
    for n in 1..=5 {
        let xml = part(&bytes, &format!("xl/worksheets/sheet{n}.xml"));
        assert!(xml.contains("<oddHeader>"), "sheet{n} header");
        assert!(xml.contains(MANDATORY_SENTENCE));
        // The footer carries the watermark, with `&` doubled for Excel.
        assert!(xml.contains("Export 0b9f6c1e"), "sheet{n} footer");
        assert!(
            xml.contains("Dr. Sita &amp;&amp; Sharma"),
            "sheet{n} footer escaping"
        );
    }
}

#[test]
fn dates_are_date_cells_in_org_time_zone_with_offset_text() {
    let mut r = full_record();
    r.session.summary.started_at = "2026-10-01T06:00:00.000Z".into();
    let bytes = encode(&[r], &options(Mode::Identified)).unwrap();
    let mut book = open(&bytes);
    let rows = sheet(&mut book, "Sessions");
    let cell = &rows[1][col(Table::Sessions, "started_at")];
    let Data::DateTime(dt) = cell else {
        panic!("started_at must be a date cell, got {cell:?}");
    };
    // Excel serial for 2026-10-01 11:45:00 local.
    let serial = dt.as_f64();
    let want = 46_296.0 + (11.0 * 3600.0 + 45.0 * 60.0) / 86_400.0;
    assert!((serial - want).abs() < 1e-6, "{serial} vs {want}");
    assert_eq!(
        rows[1][col(Table::Sessions, "utc_offset")],
        Data::String("+05:45".into())
    );
    let rev = sheet(&mut book, "Reviews");
    assert!(matches!(
        rev[1][col(Table::Reviews, "signed_at")],
        Data::DateTime(_)
    ));
}

#[test]
fn text_is_never_a_formula_and_numbers_stay_numbers() {
    let mut r = full_record();
    r.session.summary.battery = Some("=1+1".into());
    r.session.lang = Some("+x".into());
    r.session.age_pack = Some("-x".into());
    r.session.summary.bundle_version = Some("@x".into());
    r.session.calibrated_distance_cm = Some(-1.5);
    r.session.eye_dwell = None;
    r.patient.mrn = Some(asd_export::model::Mrn {
        value: "00123".into(),
        assigning_authority: "TUTH".into(),
    });
    let bytes = encode(&[r], &options(Mode::Identified)).unwrap();
    let rows = sheet(&mut open(&bytes), "Sessions");
    let s = |n: &str| rows[1][col(Table::Sessions, n)].clone();
    assert_eq!(s("battery"), Data::String("=1+1".into()));
    assert_eq!(s("lang"), Data::String("+x".into()));
    assert_eq!(s("age_pack"), Data::String("-x".into()));
    assert_eq!(s("bundle_version"), Data::String("@x".into()));
    assert_eq!(s("mrn"), Data::String("00123".into()));
    assert_eq!(s("calibrated_distance_cm"), Data::Float(-1.5));
    assert_eq!(s("eye_dwell"), Data::Empty);
    assert_eq!(s("reviewed"), Data::Bool(false));
    for n in 1..=5 {
        let xml = part(&bytes, &format!("xl/worksheets/sheet{n}.xml"));
        assert!(
            !xml.contains("<f>") && !xml.contains("<f "),
            "sheet{n} has a formula"
        );
    }
}

#[test]
fn deidentified_workbook_has_no_identifying_values_and_month_text() {
    let bytes = encode(&[full_record()], &options(Mode::Deidentified)).unwrap();
    let mut z = zip::ZipArchive::new(Cursor::new(bytes.as_slice())).unwrap();
    let mut all = String::new();
    for i in 0..z.len() {
        let mut s = String::new();
        if z.by_index(i).unwrap().read_to_string(&mut s).is_ok() {
            all.push_str(&s);
        }
    }
    for s in [
        sentinel::MRN,
        sentinel::IMPRESSION,
        sentinel::SIGNER,
        sentinel::REGISTRATION,
        sentinel::CONTENT_HASH,
        export_support::DEVICE_ID,
        export_support::PARTICIPANT_ID,
    ] {
        assert!(!all.contains(s), "{s} leaked");
    }
    let rows = sheet(&mut open(&bytes), "Sessions");
    assert_eq!(
        rows[1][col(Table::Sessions, "started_at")],
        Data::String("2026-10".into())
    );
    assert_eq!(rows[1][col(Table::Sessions, "utc_offset")], Data::Empty);
}

#[test]
fn more_than_200_000_rows_is_refused() {
    let one = golden_record();
    let records = vec![one; ROW_LIMIT + 1];
    match encode(&records, &options(Mode::Identified)) {
        Err(ExportError::ExportTooLarge { rows, limit }) => {
            assert_eq!((rows, limit), (200_001, 200_000));
        }
        other => panic!("expected ExportTooLarge, got {:?}", other.map(|b| b.len())),
    }
}

#[test]
fn output_is_byte_identical_for_identical_input() {
    let records = [full_record()];
    for mode in [Mode::Identified, Mode::Deidentified] {
        let o = options(mode);
        assert_eq!(encode(&records, &o).unwrap(), encode(&records, &o).unwrap());
    }
}
