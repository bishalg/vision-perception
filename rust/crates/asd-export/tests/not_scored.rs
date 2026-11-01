//! owner: WP-G.12
//!
//! SCH-5: a schema 1.8 `unscorable` record has no score numbers. `stored_record` reads it
//! (and every older record) as the report record type; the diary PDF prints "Not scored
//! (reason)" where the numbers would be and the FHIR gaze Observation carries no score or
//! affect component, only the measured ones and a note.

mod support;

use asd_contract::session_v1_8::UnscorableReason;
use asd_export::fhir::{self, BundleInput, FhirDevice, FhirSubject};
use asd_export::pdf::{self, DoctorReportInput};
use asd_export::{not_scored, stored_record, NotScored};
use serde_json::Value;
use support::*;

fn record_json(name: &str) -> Value {
    let path = repo_root().join(format!("rust/crates/asd-contract/tests/fixtures/{name}"));
    let v: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    v["rawPayload"].clone()
}

fn pdf_text(record: &asd_contract::session_v1_6::GazeSessionRecord) -> String {
    let bytes = pdf::render(&DoctorReportInput {
        participant_label: record.participant_id.clone(),
        child_age_months: Some(28),
        guardian_name: pdf::DEFAULT_GUARDIAN.into(),
        assessment_date: record.started_at.clone(),
        gaze_record: Some(record),
        mchat: None,
        care: None,
    })
    .expect("render");
    pdf_text_runs(&bytes).join("\n")
}

fn gaze_components(record: &asd_contract::session_v1_6::GazeSessionRecord) -> (Vec<String>, Value) {
    let bundle = fhir::bundle(&BundleInput {
        record,
        subject: FhirSubject::new("child-SESS-GOL", Some(28), None),
        device: FhirDevice::new(fhir::DEFAULT_DEVICE_NAME, "1.0"),
        mchat: None,
        document: None,
    })
    .expect("bundle");
    let obs = bundle["entry"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| &e["resource"])
        .find(|r| r["resourceType"] == "Observation" && r["component"].is_array())
        .expect("gaze observation")
        .clone();
    let codes = obs["component"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["code"]["coding"][0]["code"].as_str().unwrap().to_owned())
        .collect();
    (codes, obs)
}

const SCORE_CODES: [&str; 5] = [
    "social-preference-score",
    "social-smile-intensity-mean",
    "smile-reactivity-index",
    "flat-affect-score",
    "surprise-score-mean",
];

#[test]
fn unscorable_record_decodes_and_is_not_scored() {
    let rec = stored_record(record_json("session-v1.8-unscorable.json")).expect("decodes");
    assert_eq!(
        not_scored(&rec),
        Some(NotScored {
            reason: Some(UnscorableReason::AggregationFailed)
        })
    );
    assert_eq!(
        not_scored(&rec).unwrap().label(),
        "Not scored (aggregation failed)"
    );
    assert_eq!(NotScored { reason: None }.label(), "Not scored");
}

#[test]
fn older_and_scored_records_are_scored() {
    let scored = stored_record(record_json("session-v1.8-scored.json")).unwrap();
    assert_eq!(not_scored(&scored), None);
    assert_eq!(scored.social_preference_score, 0.62);
    assert_eq!(not_scored(&ios_session_record()), None);
    // A 1.7/1.8 record without calibratedDistanceCm still decodes (QS-5).
    let mut v = record_json("session-v1.8-scored.json");
    v.as_object_mut().unwrap().remove("calibratedDistanceCm");
    assert!(stored_record(v).is_ok());
    // A scored record missing a score is refused, never filled.
    let mut v = record_json("session-v1.8-scored.json");
    v.as_object_mut().unwrap().remove("flatAffectScore");
    assert!(stored_record(v).is_err());
}

#[test]
fn diary_prints_not_scored_instead_of_numbers() {
    let rec = stored_record(record_json("session-v1.8-unscorable.json")).unwrap();
    let text = pdf_text(&rec);
    assert!(text.contains("NOT SCORED (AGGREGATION FAILED)"), "{text}");
    assert!(
        text.contains("Smile reactivity: Not scored (aggregation failed)"),
        "{text}"
    );
    assert!(
        !text.contains("Smile reactivity: 0%"),
        "no placeholder number"
    );
    assert!(
        !text.contains("+0%") && !text.contains("\n0%"),
        "no placeholder number"
    );

    let scored = stored_record(record_json("session-v1.8-scored.json")).unwrap();
    let text = pdf_text(&scored);
    assert!(text.contains("+62%"), "{text}");
    assert!(!text.contains("Not scored"));
}

#[test]
fn fhir_observation_has_no_score_components_when_not_scored() {
    let rec = stored_record(record_json("session-v1.8-unscorable.json")).unwrap();
    let (codes, obs) = gaze_components(&rec);
    for code in SCORE_CODES {
        assert!(!codes.contains(&code.to_owned()), "{code} must be absent");
    }
    assert!(
        codes.contains(&"valid-frame-ratio".to_owned()),
        "measured values stay"
    );
    assert!(obs["note"][0]["text"]
        .as_str()
        .unwrap()
        .starts_with("Not scored (aggregation failed)"));

    let scored = stored_record(record_json("session-v1.8-scored.json")).unwrap();
    let (codes, obs) = gaze_components(&scored);
    for code in SCORE_CODES {
        assert!(
            codes.contains(&code.to_owned()),
            "{code} present when scored"
        );
    }
    assert!(obs.get("note").is_none());
}
