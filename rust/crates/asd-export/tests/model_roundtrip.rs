//! XC-3 / XC-8: `ExportRecord` built from the iOS golden session fixture serialises
//! deterministically and round-trips. The golden pins the record's wire shape: a change
//! to it is a `record_version` decision, not an accident.
//!
//! Rewrite the golden with `ASD_WRITE_GOLDEN=1 cargo test -p asd-export --test
//! model_roundtrip` and review the diff.

mod export_support;

use std::path::PathBuf;

use asd_export::model::{
    ExportRecord, Mrn, OrgFacts, Paper, ReportStatus, ReviewState, RECORD_VERSION,
};
use export_support::{full_record, golden_ingest, golden_record};

fn golden_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden/export-record.json")
}

#[test]
fn record_from_the_ios_golden_session_matches_the_golden_bytes() {
    let bytes = full_record().to_json_bytes();
    if std::env::var("ASD_WRITE_GOLDEN").is_ok_and(|v| v == "1") {
        std::fs::write(golden_path(), &bytes).unwrap();
        return;
    }
    let expected = std::fs::read(golden_path())
        .expect("missing golden export-record.json; run with ASD_WRITE_GOLDEN=1 and review it");
    assert!(
        expected == bytes,
        "export-record.json differs:\n{}",
        String::from_utf8_lossy(&bytes)
    );
}

#[test]
fn serialisation_is_deterministic() {
    assert_eq!(full_record().to_json_bytes(), full_record().to_json_bytes());
    assert_eq!(
        golden_record().to_json_bytes(),
        golden_record().to_json_bytes()
    );
}

#[test]
fn round_trip_is_lossless() {
    for record in [golden_record(), full_record()] {
        let bytes = record.to_json_bytes();
        let back: ExportRecord = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(back, record);
        assert_eq!(back.to_json_bytes(), bytes);
    }
}

#[test]
fn record_carries_the_fixture_facts() {
    let ingest = golden_ingest();
    let r = golden_record();
    assert_eq!(r.record_version, RECORD_VERSION);
    assert_eq!(RECORD_VERSION, "1");
    assert_eq!(r.session.summary.id, ingest.session_id);
    assert_eq!(r.patient.participant_token, ingest.participant_token);
    assert_eq!(r.clips.len(), ingest.per_video.as_ref().unwrap().len());
    let fixture_rows: usize = ingest
        .per_video
        .iter()
        .flatten()
        .map(|v| v.details.rows.len())
        .sum();
    assert!(fixture_rows > 0);
    assert_eq!(
        r.clips.iter().map(|c| c.rows.len()).sum::<usize>(),
        fixture_rows
    );
    assert!(r.review.is_none() && r.notes.is_empty() && r.letters.is_empty());
    assert_eq!(r.days_since_first_session, None);
    let json = serde_json::to_value(&r).unwrap();
    assert_eq!(json["record_version"], "1");
    assert_eq!(json["org"]["paper"], "a4");
}

#[test]
fn an_unknown_record_version_is_refused() {
    let mut json = serde_json::to_value(golden_record()).unwrap();
    json["record_version"] = "2".into();
    assert!(serde_json::from_value::<ExportRecord>(json).is_err());
}

#[test]
fn org_time_zone_must_be_iana() {
    let org = export_support::org();
    assert_eq!(org.tz().unwrap(), chrono_tz::Asia::Kathmandu);
    let bad = OrgFacts {
        timezone: "Nepal Standard Time".into(),
        ..org
    };
    assert!(bad.tz().is_err());
    assert_eq!(Paper::default(), Paper::A4);
}

#[test]
fn mrn_value_never_appears_in_debug_output() {
    let mrn = Mrn {
        value: "MRN-0042".into(),
        assigning_authority: "TUTH".into(),
    };
    let debug = format!("{mrn:?} {:?}", full_record());
    assert!(!debug.contains("MRN-0042"));
    assert!(!debug.contains(export_support::sentinel::MRN));
    assert!(format!("{mrn:?}").contains("TUTH"));
}

#[test]
fn report_status_follows_fhx4() {
    let mut r = full_record();
    let review = r.review.as_mut().unwrap();
    let cases = [
        (ReviewState::Open, false, false, ReportStatus::Preliminary),
        (
            ReviewState::InProgress,
            false,
            false,
            ReportStatus::Preliminary,
        ),
        (
            ReviewState::AwaitingCosign,
            true,
            false,
            ReportStatus::Preliminary,
        ),
        (ReviewState::Amending, true, true, ReportStatus::Preliminary),
        (ReviewState::Final, true, false, ReportStatus::Final),
        (ReviewState::Final, true, true, ReportStatus::Amended),
    ];
    for (state, signed, amended, expected) in cases {
        review.state = state;
        review.amended = amended;
        if !signed {
            review.signatures.clear();
        }
        assert_eq!(
            review.report_status(),
            expected,
            "{state:?} amended={amended}"
        );
    }
    assert_eq!(ReportStatus::Final.as_str(), "final");
    assert_eq!(ReportStatus::Amended.as_str(), "amended");
    assert_eq!(ReportStatus::Preliminary.as_str(), "preliminary");
    // No review at all is preliminary too.
    assert_eq!(golden_record().report_status(), ReportStatus::Preliminary);
}
