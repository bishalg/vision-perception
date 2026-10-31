//! FHIR R4 bundle parity with the iOS `FHIRBundleBuilder` (FHIR-1..4, PLC-3, SIM-5, CNS-7).
//!
//! The oracle is the iOS golden `apps/ios_asd_gaze_app/Tests/Fixtures/expected-bundle.json`,
//! read in place: if the app's exporter changes, this test fails until the Rust port follows.

mod support;

use std::collections::BTreeSet;

use asd_export::fhir::{self, BundleInput, FhirDevice, FhirSubject, ReportDocument};
use asd_export::{export_refusal, observation_status, Refusal};
use serde_json::{json, Value};
use support::*;

fn golden_device() -> FhirDevice {
    FhirDevice::new("EarlyGaze Observation Diary", "1.0.0")
}

fn golden_subject() -> FhirSubject {
    FhirSubject::new("child-GOLDEN01", Some(24), None)
}

fn golden_bundle() -> Value {
    let record = ios_fhir_golden_record();
    let (mchat, answers) = ios_golden_mchat();
    fhir::bundle(&BundleInput {
        record: &record,
        subject: golden_subject(),
        device: golden_device(),
        mchat: Some((&mchat, &answers)),
        document: None,
    })
    .expect("bundle")
}

fn simple_bundle(record: &asd_contract::session_v1_6::GazeSessionRecord) -> Value {
    fhir::bundle(&BundleInput {
        record,
        subject: golden_subject(),
        device: golden_device(),
        mchat: None,
        document: None,
    })
    .expect("bundle")
}

fn resources<'a>(bundle: &'a Value, ty: &str) -> Vec<&'a Value> {
    bundle["entry"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| &e["resource"])
        .filter(|r| r["resourceType"] == ty)
        .collect()
}

fn codes(cc: &Value) -> Vec<String> {
    cc["coding"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|c| c["code"].as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

fn gaze_observations(bundle: &Value) -> Vec<&Value> {
    resources(bundle, "Observation")
        .into_iter()
        .filter(|o| codes(&o["code"]).contains(&"93044-6".to_owned()))
        .collect()
}

fn component_codes(obs: &Value) -> Vec<String> {
    obs["component"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|c| codes(&c["code"]))
        .collect()
}

fn all_references(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::Object(m) => {
            for (k, val) in m {
                if k == "reference" {
                    if let Some(s) = val.as_str() {
                        out.push(s.to_owned());
                    }
                }
                all_references(val, out);
            }
        }
        Value::Array(a) => a.iter().for_each(|x| all_references(x, out)),
        _ => {}
    }
}

// ---- golden ------------------------------------------------------------------------------

#[test]
fn bundle_matches_ios_golden_fixture() {
    let path = repo_root().join("apps/ios_asd_gaze_app/Tests/Fixtures/expected-bundle.json");
    let expected: Value =
        serde_json::from_str(&std::fs::read_to_string(path).expect("iOS golden")).unwrap();
    let actual = golden_bundle();
    if !json_eq(&actual, &expected) {
        let out = std::env::temp_dir().join("asd-report-actual-bundle.json");
        std::fs::write(&out, serde_json::to_string_pretty(&actual).unwrap()).ok();
        panic!(
            "bundle differs from the iOS expected-bundle.json; actual at {}",
            out.display()
        );
    }
}

#[test]
fn bundle_is_deterministic() {
    assert_eq!(golden_bundle(), golden_bundle());
}

#[test]
fn server_bundle_with_document_matches_golden() {
    let record = ios_session_record();
    let pdf = b"%PDF-1.7 golden".to_vec();
    let bundle = fhir::bundle(&BundleInput {
        record: &record,
        subject: FhirSubject::new(&record.participant_id, Some(28), None),
        device: FhirDevice::new(fhir::DEFAULT_DEVICE_NAME, "1.0"),
        mchat: None,
        document: Some(ReportDocument { pdf: &pdf }),
    })
    .unwrap();
    let mut text = serde_json::to_string_pretty(&bundle).unwrap();
    text.push('\n');
    assert_golden_bytes("server-bundle.json", text.as_bytes());
}

// ---- FHIR-1 shape --------------------------------------------------------------------------

#[test]
fn bundle_is_a_collection_with_unique_urn_uuid_full_urls() {
    let bundle = golden_bundle();
    assert_eq!(bundle["resourceType"], "Bundle");
    assert_eq!(bundle["type"], "collection");
    let urls: Vec<&str> = bundle["entry"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["fullUrl"].as_str().unwrap())
        .collect();
    assert_eq!(urls.iter().collect::<BTreeSet<_>>().len(), urls.len());
    for url in urls {
        let id = url.strip_prefix("urn:uuid:").expect(url);
        assert!(uuid::Uuid::parse_str(id).is_ok(), "{url}");
    }
}

#[test]
fn every_reference_resolves_inside_the_bundle() {
    let record = ios_session_record();
    let pdf = b"%PDF".to_vec();
    let (mchat, answers) = ios_golden_mchat();
    let bundle = fhir::bundle(&BundleInput {
        record: &record,
        subject: FhirSubject::new("child-SESS-GOL", None, Some("Sam")),
        device: golden_device(),
        mchat: Some((&mchat, &answers)),
        document: Some(ReportDocument { pdf: &pdf }),
    })
    .unwrap();
    let urls: BTreeSet<String> = bundle["entry"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["fullUrl"].as_str().unwrap().to_owned())
        .collect();
    let mut refs = Vec::new();
    all_references(&bundle, &mut refs);
    assert!(!refs.is_empty());
    for r in refs {
        assert!(urls.contains(&r), "dangling reference {r}");
    }
}

#[test]
fn patient_is_pseudonymous_and_age_is_an_extension() {
    let bundle = golden_bundle();
    let patients = resources(&bundle, "Patient");
    assert_eq!(patients.len(), 1);
    let p = patients[0];
    assert_eq!(p["id"], "child-GOLDEN01");
    assert!(p.get("name").is_none(), "CNS-7: no name");
    assert!(p.get("birthDate").is_none(), "FHIR-1: no birth date");
    assert_eq!(p["extension"][0]["valueQuantity"]["value"], 24);

    let no_age = fhir::bundle(&BundleInput {
        record: &ios_fhir_golden_record(),
        subject: FhirSubject::new("child-GOLDEN01", None, None),
        device: golden_device(),
        mchat: None,
        document: None,
    })
    .unwrap();
    assert!(resources(&no_age, "Patient")[0].get("extension").is_none());
}

#[test]
fn caregiver_name_is_only_on_related_person_and_blank_is_dropped() {
    let named = fhir::bundle(&BundleInput {
        record: &ios_fhir_golden_record(),
        subject: FhirSubject::new("child-GOLDEN01", None, Some("  Alex  ")),
        device: golden_device(),
        mchat: None,
        document: None,
    })
    .unwrap();
    assert_eq!(
        resources(&named, "RelatedPerson")[0]["name"],
        json!([{"text": "Alex"}])
    );
    assert!(resources(&named, "Patient")[0].get("name").is_none());

    let blank = FhirSubject::new("child-GOLDEN01", None, Some("   "));
    assert_eq!(blank.caregiver_display_name, None);
}

// ---- FHIR-2, SIM-5: status and refusal -------------------------------------------------

#[test]
fn simulated_and_synthetic_records_have_no_gaze_observation() {
    let base = ios_fhir_golden_record();
    for (source, refusal) in [
        ("simulated", Refusal::SimulatedCapture),
        ("synthetic", Refusal::SyntheticCapture),
    ] {
        let rec = record_with(&base, json!({"captureSource": source}));
        assert_eq!(export_refusal(&rec), Some(refusal), "{source}");
        assert_eq!(observation_status(&rec), None);
        let bundle = simple_bundle(&rec);
        assert!(gaze_observations(&bundle).is_empty(), "{source}");
        assert!(resources(&bundle, "Provenance").is_empty(), "{source}");
        // Patient, RelatedPerson and Device stay (FHIRBundleBuilder parity).
        assert_eq!(bundle["entry"].as_array().unwrap().len(), 3, "{source}");
    }
}

#[test]
fn quality_verdict_maps_to_observation_status() {
    let base = ios_fhir_golden_record();
    for (verdict, status) in [
        ("optimal", Some("final")),
        ("acceptable", Some("final")),
        ("degraded", Some("preliminary")),
        ("rejected", None),
    ] {
        let rec = record_with(&base, json!({"qualityVerdict": verdict}));
        assert_eq!(observation_status(&rec), status, "{verdict}");
        let bundle = simple_bundle(&rec);
        let obs = gaze_observations(&bundle);
        match status {
            Some(s) => assert_eq!(obs[0]["status"], s),
            None => {
                assert!(obs.is_empty());
                assert_eq!(export_refusal(&rec), Some(Refusal::InsufficientSamples));
            }
        }
    }
}

#[test]
fn refusal_messages_match_the_ios_copy() {
    assert_eq!(
        Refusal::SimulatedCapture.message(),
        "Simulated data cannot be exported as a health record."
    );
    assert_eq!(
        Refusal::InsufficientSamples.message(),
        "This observation did not have enough usable samples to export."
    );
}

// ---- FHIR-3, PLC-3: components ---------------------------------------------------------

#[test]
fn unmeasured_metrics_and_first_look_latency_are_never_exported() {
    let bundle = golden_bundle();
    let codes = component_codes(gaze_observations(&bundle)[0]);
    assert!(!codes.contains(&"mouth-region-dwell-ratio".to_owned()));
    assert!(!codes.iter().any(|c| c.contains("latency")));
    assert!(codes.contains(&"clip-score-clip-a".to_owned()));
    assert!(
        !codes.contains(&"clip-score-clip-b".to_owned()),
        "no baseline, no subscore"
    );
    // No SNOMED finding is used as a component code (FHIR-3).
    assert!(!codes
        .iter()
        .any(|c| c.chars().all(|ch| ch.is_ascii_digit())));
}

#[test]
fn legacy_provenance_is_tagged() {
    let rec = record_with(&ios_fhir_golden_record(), json!({"legacyProvenance": true}));
    let bundle = simple_bundle(&rec);
    let obs = gaze_observations(&bundle)[0];
    assert_eq!(obs["meta"]["tag"][0]["code"], "legacy-provenance");
    let prov = resources(&bundle, "Provenance")[0];
    let urls: Vec<&str> = prov["extension"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["url"].as_str().unwrap())
        .collect();
    assert!(urls.contains(&"http://curv.org/fhir/StructureDefinition/capture-source-inferred"));
}

// ---- DocumentReference (the regenerated PDF) -------------------------------------------

#[test]
fn document_reference_embeds_the_pdf_and_points_at_the_patient() {
    let record = ios_fhir_golden_record();
    let pdf = b"%PDF-1.7 bytes".to_vec();
    let bundle = fhir::bundle(&BundleInput {
        record: &record,
        subject: golden_subject(),
        device: golden_device(),
        mchat: None,
        document: Some(ReportDocument { pdf: &pdf }),
    })
    .unwrap();
    let docs = resources(&bundle, "DocumentReference");
    assert_eq!(docs.len(), 1);
    let d = docs[0];
    let att = &d["content"][0]["attachment"];
    assert_eq!(att["contentType"], "application/pdf");
    use base64::Engine;
    let data = base64::engine::general_purpose::STANDARD
        .decode(att["data"].as_str().unwrap())
        .unwrap();
    assert_eq!(data, pdf);
    assert_eq!(att["size"], pdf.len());
    let patient_url = bundle["entry"][0]["fullUrl"].as_str().unwrap();
    assert_eq!(d["subject"]["reference"], patient_url);
    assert_eq!(codes(&d["type"]), ["34108-1"]);
    // FHIR forbids empty arrays: no `author: []` when there is no practitioner.
    assert!(d.get("author").is_none());
    assert_eq!(d["date"], "2026-10-01T10:00:00Z");
    // The document is last and gets no Provenance (iOS provenances observations only).
    assert_eq!(
        bundle["entry"].as_array().unwrap().last().unwrap()["resource"]["resourceType"],
        "DocumentReference"
    );
    assert_eq!(resources(&bundle, "Provenance").len(), 1);
}

#[test]
fn timestamps_are_whole_second_utc() {
    let rec = record_with(
        &ios_fhir_golden_record(),
        json!({"startedAt": "2026-10-01T12:00:00.987+02:00"}),
    );
    let bundle = simple_bundle(&rec);
    assert_eq!(
        gaze_observations(&bundle)[0]["effectiveDateTime"],
        "2026-10-01T10:00:00Z"
    );
}

#[test]
fn a_malformed_timestamp_is_an_error_not_a_guess() {
    let rec = record_with(&ios_fhir_golden_record(), json!({"startedAt": "yesterday"}));
    let err = fhir::bundle(&BundleInput {
        record: &rec,
        subject: golden_subject(),
        device: golden_device(),
        mchat: None,
        document: None,
    })
    .unwrap_err();
    assert!(err.to_string().contains("startedAt"), "{err}");
}
