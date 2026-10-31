//! FHIR R4 clinical report bundle (FHX-1..8, XC-4, XC-6, XC-7, XC-8; WP-I.3).
//!
//! Validation is structural (FHX-8, ADR-0003 D8): resource types, required elements, no
//! empty values, every reference resolves inside the bundle, no forbidden Patient elements.

mod export_support;
mod support;

use std::collections::BTreeSet;

use asd_contract::session_v1_6::GazeSessionRecord;
use asd_export::codes::{self, CodePolicy};
use asd_export::deid;
use asd_export::fhir::clinical::{
    self, BundleKind, ClinicalBundleInput, ClinicalDocument, DocumentKind, FhirProfile,
};
use asd_export::fhir::{self, BundleInput, FhirDevice, FhirSubject};
use asd_export::model::{ExportRecord, ReviewState};
use asd_export::policy::{Mode, Purpose};
use asd_export::watermark::{Watermark, MANDATORY_SENTENCE};
use asd_export::{ExportError, ReportError};
use base64::Engine;
use export_support::{full_record, golden_ingest, golden_record, sentinel};
use serde_json::{json, Value};

const EXPORT_ID: &str = "0b7e5a2c-3d4f-4e6a-8b9c-1d2e3f4a5b6c";
const PDF: &[u8] = b"%PDF-1.7 clinical summary golden bytes";
const MRN_SYSTEM: &str = "urn:oid:1.2.524.1.1.1";
const REG_SYSTEM: &str = "https://nmc.org.np/registration";
const SALT: [u8; deid::SALT_LEN] = [7; deid::SALT_LEN];

// ---- fixtures ------------------------------------------------------------------------------

fn gaze() -> GazeSessionRecord {
    golden_ingest().raw_payload
}

fn device() -> FhirDevice {
    FhirDevice::new(fhir::DEFAULT_DEVICE_NAME, "1.0")
}

fn watermark(mode: Mode) -> Watermark {
    Watermark {
        export_id: EXPORT_ID.into(),
        requested_by_display: "Dr. Requester".into(),
        requested_at_utc: "2026-10-05T06:15:00Z".into(),
        mode,
        purpose: Purpose::ClinicalCare,
        org_name: "Kathmandu Child Clinic".into(),
    }
}

fn profile() -> FhirProfile {
    FhirProfile {
        mrn_system: Some(MRN_SYSTEM.into()),
        registration_system: Some(REG_SYSTEM.into()),
        codes: CodePolicy::default(),
    }
}

fn opt_in_profile() -> FhirProfile {
    FhirProfile {
        codes: CodePolicy {
            allow_unverified: true,
        },
        ..profile()
    }
}

struct Case {
    record: ExportRecord,
    gaze: GazeSessionRecord,
    watermark: Watermark,
    profile: FhirProfile,
    document: Option<DocumentKind>,
    url: Option<&'static str>,
    kind: BundleKind,
}

impl Case {
    fn identified(record: ExportRecord) -> Self {
        Self {
            record,
            gaze: gaze(),
            watermark: watermark(Mode::Identified),
            profile: profile(),
            document: Some(DocumentKind::ClinicalSummary),
            url: None,
            kind: BundleKind::Collection,
        }
    }

    fn deidentified() -> Self {
        let record = deid::apply(&[full_record()], &SALT).unwrap().remove(0);
        Self {
            watermark: watermark(Mode::Deidentified),
            ..Self::identified(record)
        }
    }

    fn try_build(&self) -> Result<Value, ExportError> {
        clinical::clinical_report_bundle(&ClinicalBundleInput {
            record: &self.record,
            gaze: &self.gaze,
            device: device(),
            watermark: &self.watermark,
            profile: &self.profile,
            document: self.document.map(|kind| ClinicalDocument {
                kind,
                pdf: PDF,
                url: self.url,
            }),
            kind: self.kind,
        })
    }

    fn build(&self) -> Value {
        self.try_build().expect("clinical bundle")
    }
}

fn with_state(state: ReviewState, amended: bool) -> ExportRecord {
    let mut r = full_record();
    let review = r.review.as_mut().unwrap();
    review.state = state;
    review.amended = amended;
    r
}

// ---- helpers -------------------------------------------------------------------------------

fn entries(bundle: &Value) -> &Vec<Value> {
    bundle["entry"].as_array().expect("entry array")
}

fn resources<'a>(bundle: &'a Value, ty: &str) -> Vec<&'a Value> {
    entries(bundle)
        .iter()
        .map(|e| &e["resource"])
        .filter(|r| r["resourceType"] == ty)
        .collect()
}

fn one<'a>(bundle: &'a Value, ty: &str) -> &'a Value {
    let found = resources(bundle, ty);
    assert_eq!(found.len(), 1, "expected one {ty}");
    found[0]
}

fn full_url_of(bundle: &Value, ty: &str) -> String {
    entries(bundle)
        .iter()
        .find(|e| e["resource"]["resourceType"] == ty)
        .and_then(|e| e["fullUrl"].as_str())
        .unwrap_or_else(|| panic!("no {ty}"))
        .to_owned()
}

fn all_references(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::Object(m) => {
            for (k, val) in m {
                if k == "reference" {
                    out.push(val.as_str().expect("reference is a string").to_owned());
                }
                all_references(val, out);
            }
        }
        Value::Array(a) => a.iter().for_each(|x| all_references(x, out)),
        _ => {}
    }
}

fn codings(v: &Value, out: &mut Vec<(String, String)>) {
    match v {
        Value::Object(m) => {
            if let (Some(Value::String(s)), Some(Value::String(c))) =
                (m.get("system"), m.get("code"))
            {
                out.push((s.clone(), c.clone()));
            }
            m.values().for_each(|x| codings(x, out));
        }
        Value::Array(a) => a.iter().for_each(|x| codings(x, out)),
        _ => {}
    }
}

fn text(bundle: &Value) -> String {
    serde_json::to_string(bundle).unwrap()
}

/// FHIR forbids empty strings, arrays and objects, and `null` (R4 JSON rules).
fn assert_no_empty_values(v: &Value, path: &str) {
    match v {
        Value::Null => panic!("null at {path}"),
        Value::String(s) => assert!(!s.trim().is_empty(), "empty string at {path}"),
        Value::Array(a) => {
            assert!(!a.is_empty(), "empty array at {path}");
            for (i, x) in a.iter().enumerate() {
                assert_no_empty_values(x, &format!("{path}[{i}]"));
            }
        }
        Value::Object(m) => {
            assert!(!m.is_empty(), "empty object at {path}");
            for (k, x) in m {
                assert_no_empty_values(x, &format!("{path}.{k}"));
            }
        }
        _ => {}
    }
}

/// FHX-8 structural validation.
fn assert_structurally_valid(bundle: &Value) {
    assert_eq!(bundle["resourceType"], "Bundle");
    assert_no_empty_values(bundle, "Bundle");
    let ty = bundle["type"].as_str().unwrap();
    assert!(ty == "collection" || ty == "transaction", "{ty}");

    let allowed = [
        "Patient",
        "Organization",
        "Practitioner",
        "Device",
        "Observation",
        "DiagnosticReport",
        "DocumentReference",
        "Provenance",
    ];
    let mut urls = BTreeSet::new();
    for e in entries(bundle) {
        let url = e["fullUrl"].as_str().expect("fullUrl");
        let id = url.strip_prefix("urn:uuid:").expect("urn:uuid fullUrl");
        assert!(uuid::Uuid::parse_str(id).is_ok(), "{url}");
        assert!(urls.insert(url.to_owned()), "duplicate fullUrl {url}");
        let r = &e["resource"];
        let rt = r["resourceType"].as_str().expect("resourceType");
        assert!(allowed.contains(&rt), "unexpected resource {rt}");
        assert!(r["id"].is_string(), "{rt} has an id");
        let required: &[&str] = match rt {
            "Observation" | "DiagnosticReport" => &["status", "code"],
            "DocumentReference" => &["status", "content"],
            "Provenance" => &["target", "recorded", "agent"],
            "Organization" => &["name"],
            _ => &[],
        };
        for field in required {
            assert!(r.get(*field).is_some(), "{rt}.{field} is required");
        }
        if rt == "DocumentReference" {
            for c in r["content"].as_array().unwrap() {
                assert!(c.get("attachment").is_some(), "content.attachment required");
            }
        }
        if rt == "Provenance" {
            for a in r["agent"].as_array().unwrap() {
                assert!(a.get("who").is_some(), "Provenance.agent.who required");
            }
        }
        if rt == "Patient" {
            for forbidden in [
                "name",
                "birthDate",
                "telecom",
                "address",
                "photo",
                "contact",
            ] {
                assert!(
                    r.get(forbidden).is_none(),
                    "Patient.{forbidden} is forbidden"
                );
            }
        }
        if ty == "transaction" {
            let req = &e["request"];
            let method = req["method"].as_str().expect("request.method");
            assert!(method == "POST" || method == "PUT", "{method}");
            assert!(req["url"].is_string(), "request.url");
        } else {
            assert!(e.get("request").is_none());
        }
    }
    let mut refs = Vec::new();
    all_references(bundle, &mut refs);
    assert!(!refs.is_empty());
    for r in refs {
        assert!(urls.contains(&r), "dangling reference {r}");
    }
}

// ---- FHX-8: structure ----------------------------------------------------------------------

#[test]
fn every_variant_is_structurally_valid_and_references_resolve() {
    let mut cases = vec![
        Case::identified(full_record()),
        Case::identified(golden_record()),
        Case::deidentified(),
    ];
    let mut tx = Case::identified(full_record());
    tx.kind = BundleKind::Transaction;
    tx.url = Some("https://blob.example/exports/summary.pdf");
    cases.push(tx);
    let mut no_doc = Case::identified(full_record());
    no_doc.document = None;
    cases.push(no_doc);
    for case in &cases {
        assert_structurally_valid(&case.build());
    }
}

#[test]
fn entry_order_is_fixed() {
    let bundle = Case::identified(full_record()).build();
    let order: Vec<&str> = entries(&bundle)
        .iter()
        .map(|e| e["resource"]["resourceType"].as_str().unwrap())
        .collect();
    assert_eq!(
        order,
        [
            "Patient",
            "Organization",
            "Practitioner",
            "Practitioner",
            "Device",
            "Observation",
            "DiagnosticReport",
            "DocumentReference",
            "Provenance",
            "Provenance",
        ]
    );
}

// ---- FHX-1: ids and the parity Observation -------------------------------------------------

fn parity_observation(record: &GazeSessionRecord, token: &str) -> (String, Value) {
    let bundle = fhir::bundle(&BundleInput {
        record,
        subject: FhirSubject::new(token, None, None),
        device: device(),
        mchat: None,
        document: None,
    })
    .unwrap();
    let e = entries(&bundle)
        .iter()
        .find(|e| e["resource"]["resourceType"] == "Observation")
        .unwrap();
    (
        e["fullUrl"].as_str().unwrap().to_owned(),
        e["resource"].clone(),
    )
}

fn jcs(v: &Value) -> Vec<u8> {
    asd_contract::signing::canonical_bytes(v).unwrap()
}

#[test]
fn gaze_observation_equals_the_parity_observation_after_jcs_when_the_profile_opts_in() {
    let mut case = Case::identified(full_record());
    case.profile = opt_in_profile();
    let bundle = case.build();
    let (url, expected) = parity_observation(&case.gaze, &case.record.patient.participant_token);
    assert_eq!(full_url_of(&bundle, "Observation"), url);
    assert_eq!(jcs(one(&bundle, "Observation")), jcs(&expected));
    // Patient and Device fullUrls follow the iOS v5 scheme too.
    assert_eq!(
        full_url_of(&bundle, "Patient"),
        fhir::patient_reference(&FhirSubject::new(
            &case.record.patient.participant_token,
            None,
            None
        ))
    );
}

#[test]
fn strict_profile_drops_only_the_unverified_codings_from_the_parity_observation() {
    let case = Case::identified(full_record());
    let bundle = case.build();
    let (_, mut expected) = parity_observation(&case.gaze, &case.record.patient.participant_token);
    // LOINC 93044-6 and SNOMED 713444003 are unverified (G8, XC-7): not emitted. Nothing is
    // added in their place; `code.text` stays.
    expected["code"]
        .as_object_mut()
        .unwrap()
        .remove("coding")
        .unwrap();
    assert_eq!(jcs(one(&bundle, "Observation")), jcs(&expected));
}

#[test]
fn strict_profile_emits_no_unverified_external_code() {
    for case in [Case::identified(full_record()), Case::deidentified()] {
        let bundle = case.build();
        let mut found = Vec::new();
        codings(&bundle, &mut found);
        for (system, code) in found {
            if system.starts_with(codes::system::LOCAL) {
                continue;
            }
            let entry = codes::lookup(&system, &code)
                .unwrap_or_else(|| panic!("{system}|{code} is not in codes.rs"));
            assert!(entry.verified, "{system}|{code} is unverified (XC-7)");
        }
    }
}

/// XC-7 verification done in WP-I.3: what the clinical bundle needs is verified with a
/// cited source; the iOS codes the terminology server refuted stay unverified.
#[test]
fn wp_i3_code_verification_outcome() {
    for code in [
        &codes::LOINC_SUMMARY_OF_EPISODE_NOTE,
        &codes::LOINC_REFERRAL_NOTE,
        &codes::IHE_FORMAT_MIME_TYPE_SUFFICIENT,
        &codes::V2_IDENTIFIER_TYPE_MR,
        &codes::PROVENANCE_PARTICIPANT_AUTHOR,
        &codes::PROVENANCE_PARTICIPANT_VERIFIER,
        &codes::UCUM_UNITY,
    ] {
        assert!(code.verified, "{code}");
        assert!(code.source.contains("2026-10-31"), "{code}: dated source");
        assert!(code.source.contains("https://"), "{code}: cited source");
    }
    let refuted = [
        &codes::LOINC_GAZE_PANEL,
        &codes::LOINC_MCHAT_TOTAL,
        &codes::LOINC_MCHAT_RISK,
        &codes::LOINC_MCHAT_PANEL,
        &codes::SNOMED_EYE_TRACKING_EXAM,
        &codes::PROVENANCE_PARTICIPANT_DEVICE,
        &codes::IHE_FORMAT_XFDF,
    ];
    for code in refuted.into_iter().chain(codes::LOINC_MCHAT_ITEMS.iter()) {
        assert!(!code.verified, "{code}");
    }
}

#[test]
fn opt_in_profile_codes_are_all_in_the_table() {
    let mut case = Case::identified(full_record());
    case.profile = opt_in_profile();
    let mut found = Vec::new();
    codings(&case.build(), &mut found);
    for (system, code) in found {
        if !system.starts_with(codes::system::LOCAL) {
            assert!(codes::lookup(&system, &code).is_some(), "{system}|{code}");
        }
    }
}

#[test]
fn resource_ids_are_deterministic_v5_uuids() {
    let bundle = Case::identified(full_record()).build();
    let session = "SESS-GOLDEN-0001";
    assert_eq!(
        full_url_of(&bundle, "DiagnosticReport"),
        format!(
            "urn:uuid:{}",
            clinical::resource_id(&format!("DiagnosticReport/{session}"))
        )
    );
    let doc = one(&bundle, "DocumentReference");
    assert_eq!(
        doc["id"],
        clinical::resource_id(&format!("DocumentReference/{EXPORT_ID}"))
    );
    assert_eq!(
        full_url_of(&bundle, "DocumentReference"),
        format!("urn:uuid:{}", doc["id"].as_str().unwrap())
    );
    assert_eq!(
        bundle["id"],
        clinical::resource_id(&format!("Bundle/clinical/{EXPORT_ID}/{session}"))
    );
}

// ---- FHX-3: Patient ------------------------------------------------------------------------

#[test]
fn identified_patient_has_mr_identifier_and_the_token() {
    let bundle = Case::identified(full_record()).build();
    let p = one(&bundle, "Patient");
    assert_eq!(p["id"], "child-SESS-GOL");
    let ids = p["identifier"].as_array().unwrap();
    assert_eq!(ids.len(), 2);
    assert_eq!(ids[0]["system"], MRN_SYSTEM);
    assert_eq!(ids[0]["value"], sentinel::MRN);
    assert_eq!(
        ids[0]["type"]["coding"][0],
        json!({
            "system": "http://terminology.hl7.org/CodeSystem/v2-0203",
            "code": "MR",
            "display": "Medical record number",
        })
    );
    assert_eq!(ids[0]["assigner"]["display"], sentinel::AUTHORITY);
    assert_eq!(
        ids[1],
        json!({
            "system": "http://curv.org/fhir/NamingSystem/participant-token",
            "value": "child-SESS-GOL",
        })
    );
}

#[test]
fn identified_patient_without_mrn_carries_only_the_token() {
    let bundle = Case::identified(golden_record()).build();
    let ids = one(&bundle, "Patient")["identifier"]
        .as_array()
        .unwrap()
        .clone();
    assert_eq!(ids.len(), 1);
    assert_eq!(ids[0]["value"], "child-SESS-GOL");
}

#[test]
fn deidentified_bundle_has_no_practitioner_mrn_conclusion_or_identity() {
    let case = Case::deidentified();
    let bundle = case.build();
    assert!(resources(&bundle, "Practitioner").is_empty());
    assert!(resources(&bundle, "Provenance").is_empty());
    let report = one(&bundle, "DiagnosticReport");
    assert!(report.get("conclusion").is_none());
    assert!(report.get("resultsInterpreter").is_none());
    assert!(report.get("issued").is_none());
    // Disposition is kept (XC-4).
    assert_eq!(report["conclusionCode"][0]["coding"][0]["code"], "monitor");

    let p = one(&bundle, "Patient");
    let pseudonym = case.record.patient.participant_token.clone();
    assert!(pseudonym.starts_with("P-"));
    assert_eq!(p["id"], pseudonym.as_str());
    assert_eq!(
        p["identifier"],
        json!([{
            "system": "http://curv.org/fhir/NamingSystem/export-pseudonym",
            "value": pseudonym,
        }])
    );

    let all = text(&bundle);
    for leaked in [
        sentinel::MRN,
        sentinel::AUTHORITY,
        sentinel::IMPRESSION,
        sentinel::RECOMMENDATIONS,
        sentinel::SIGNER,
        sentinel::COSIGNER,
        sentinel::REGISTRATION,
        sentinel::SIGNER_ID,
        sentinel::CONTENT_HASH,
        "SESS-GOLDEN-0001",
        "child-SESS-GOL",
        "2026-10-26T",
        export_support::PARTICIPANT_ID,
        export_support::DEVICE_ID,
    ] {
        assert!(
            !all.contains(leaked),
            "de-identified bundle leaks {leaked:?}"
        );
    }
    // The Observation id is not the v5 hash of the real session id either.
    assert!(!all.contains(&clinical::resource_id("Observation/gaze/SESS-GOLDEN-0001")));
}

#[test]
fn deidentified_dates_are_month_precision() {
    let bundle = Case::deidentified().build();
    assert_eq!(one(&bundle, "Observation")["effectiveDateTime"], "2026-10");
    assert_eq!(
        one(&bundle, "DiagnosticReport")["effectiveDateTime"],
        "2026-10"
    );
}

#[test]
fn deidentified_mode_refuses_a_record_that_skipped_deid() {
    let mut case = Case::identified(full_record());
    case.watermark = watermark(Mode::Deidentified);
    assert!(matches!(
        case.try_build(),
        Err(ExportError::NotDeidentified)
    ));
}

// ---- FHX-4: DiagnosticReport ---------------------------------------------------------------

#[test]
fn diagnostic_report_status_table() {
    let cases: Vec<(Option<(ReviewState, bool)>, &str)> = vec![
        (None, "preliminary"),
        (Some((ReviewState::Open, false)), "preliminary"),
        (Some((ReviewState::InProgress, false)), "preliminary"),
        (Some((ReviewState::AwaitingCosign, false)), "preliminary"),
        (Some((ReviewState::Amending, true)), "preliminary"),
        (Some((ReviewState::Final, false)), "final"),
        (Some((ReviewState::Final, true)), "amended"),
    ];
    for (state, expected) in cases {
        let record = match state {
            None => golden_record(),
            Some((s, amended)) => with_state(s, amended),
        };
        let bundle = Case::identified(record).build();
        assert_eq!(
            one(&bundle, "DiagnosticReport")["status"],
            expected,
            "{state:?}"
        );
        assert_eq!(
            one(&bundle, "DocumentReference")["docStatus"],
            expected,
            "{state:?}"
        );
    }
}

#[test]
fn diagnostic_report_links_subject_result_interpreters_and_performer() {
    let bundle = Case::identified(full_record()).build();
    let r = one(&bundle, "DiagnosticReport");
    assert_eq!(r["subject"]["reference"], full_url_of(&bundle, "Patient"));
    assert_eq!(
        r["result"],
        json!([{"reference": full_url_of(&bundle, "Observation")}])
    );
    assert_eq!(
        r["performer"],
        json!([{"reference": full_url_of(&bundle, "Organization")}])
    );
    let practitioners: Vec<String> = entries(&bundle)
        .iter()
        .filter(|e| e["resource"]["resourceType"] == "Practitioner")
        .map(|e| e["fullUrl"].as_str().unwrap().to_owned())
        .collect();
    let interpreters: Vec<String> = r["resultsInterpreter"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x["reference"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(interpreters, practitioners);
    assert_eq!(
        r["code"]["coding"],
        json!([{
            "system": "http://curv.org/fhir/CodeSystem/gaze-engagement",
            "code": "gaze-observation-review",
            "display": "Gaze observation review",
        }])
    );
    assert_eq!(r["conclusion"], sentinel::IMPRESSION);
    assert_eq!(
        r["conclusionCode"][0]["coding"][0]["system"],
        "http://curv.org/fhir/CodeSystem/clinical-disposition"
    );
    assert_eq!(r["issued"], "2026-10-02T08:30:00Z");
    assert!(
        r.get("presentedForm").is_none(),
        "the PDF lives only in the DocumentReference"
    );
}

#[test]
fn narrative_carries_the_mandatory_sentence_and_watermark() {
    for case in [Case::identified(full_record()), Case::deidentified()] {
        let bundle = case.build();
        let t = &one(&bundle, "DiagnosticReport")["text"];
        assert_eq!(t["status"], "generated");
        let div = t["div"].as_str().unwrap();
        assert!(div.starts_with("<div xmlns=\"http://www.w3.org/1999/xhtml\">"));
        assert!(div.ends_with("</div>"));
        assert!(div.contains(MANDATORY_SENTENCE), "XC-6 sentence");
        assert!(div.contains(&format!("Export {EXPORT_ID}")), "watermark");
    }
}

#[test]
fn narrative_escapes_markup_in_the_watermark() {
    let mut case = Case::identified(full_record());
    case.watermark.requested_by_display = "Dr. <script>&".into();
    let bundle = case.build();
    let div = one(&bundle, "DiagnosticReport")["text"]["div"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(!div.contains("<script>"));
    assert!(div.contains("Dr. &lt;script&gt;&amp;"));
}

#[test]
fn rejected_quality_session_has_a_report_without_result() {
    let mut case = Case::identified(full_record());
    case.gaze = support::record_with(&case.gaze, json!({"qualityVerdict": "rejected"}));
    let bundle = case.build();
    assert!(resources(&bundle, "Observation").is_empty());
    assert!(one(&bundle, "DiagnosticReport").get("result").is_none());
    assert_structurally_valid(&bundle);
}

#[test]
fn simulated_and_synthetic_sessions_are_refused() {
    for source in ["simulated", "synthetic"] {
        let mut case = Case::identified(full_record());
        case.gaze = support::record_with(&case.gaze, json!({"captureSource": source}));
        assert!(
            matches!(
                case.try_build(),
                Err(ExportError::Report(ReportError::Refused(_)))
            ),
            "{source}"
        );
    }
}

// ---- FHX-5: DocumentReference --------------------------------------------------------------

#[test]
fn document_reference_embeds_the_pdf_with_sha1_hash_and_verified_codes() {
    let bundle = Case::identified(full_record()).build();
    let d = one(&bundle, "DocumentReference");
    assert_eq!(d["status"], "current");
    assert_eq!(
        d["type"]["coding"],
        json!([{"system": "http://loinc.org", "code": "34133-9", "display": "Summary of Episode Note"}])
    );
    let content = &d["content"][0];
    assert_eq!(
        content["format"],
        json!({
            "system": "http://ihe.net/fhir/ValueSet/IHE.FormatCode.codesystem",
            "code": "urn:ihe:iti:xds:2017:mimeTypeSufficient",
            "display": "mimeType Sufficient",
        })
    );
    let att = &content["attachment"];
    let b64 = base64::engine::general_purpose::STANDARD;
    assert_eq!(att["contentType"], "application/pdf");
    assert_eq!(b64.decode(att["data"].as_str().unwrap()).unwrap(), PDF);
    assert_eq!(att["size"], PDF.len());
    let sha1 = b64.encode(sha1_smol::Sha1::from(PDF).digest().bytes());
    assert_eq!(att["hash"], sha1, "R4 Attachment.hash is SHA-1 (trap 15)");
    assert_eq!(d["subject"]["reference"], full_url_of(&bundle, "Patient"));
    assert_eq!(
        d["context"]["related"],
        json!([{"reference": full_url_of(&bundle, "DiagnosticReport")}])
    );
}

#[test]
fn referral_letters_use_the_referral_note_code() {
    let mut case = Case::identified(full_record());
    case.document = Some(DocumentKind::Referral);
    let bundle = case.build();
    assert_eq!(
        one(&bundle, "DocumentReference")["type"]["coding"][0]["code"],
        "57133-1"
    );
}

#[test]
fn push_bundles_link_the_pdf_by_url() {
    let mut case = Case::identified(full_record());
    case.kind = BundleKind::Transaction;
    case.url = Some("https://blob.example/exports/summary.pdf");
    let bundle = case.build();
    let att = &one(&bundle, "DocumentReference")["content"][0]["attachment"];
    assert_eq!(att["url"], "https://blob.example/exports/summary.pdf");
    assert!(att.get("data").is_none());
    assert_eq!(att["size"], PDF.len());
    assert!(att["hash"].is_string());
}

// ---- FHX-6: Practitioner, Provenance -------------------------------------------------------

#[test]
fn practitioners_carry_console_user_and_registration_identifiers() {
    let bundle = Case::identified(full_record()).build();
    let ps = resources(&bundle, "Practitioner");
    assert_eq!(ps.len(), 2);
    assert_eq!(
        ps[0]["identifier"],
        json!([
            {"system": "http://curv.org/fhir/NamingSystem/console-user", "value": sentinel::SIGNER_ID},
            {"system": REG_SYSTEM, "value": sentinel::REGISTRATION},
        ])
    );
    assert_eq!(ps[0]["name"], json!([{"text": sentinel::SIGNER}]));
    // The cosigner has no registration on file.
    assert_eq!(ps[1]["identifier"].as_array().unwrap().len(), 1);

    let mut no_reg = Case::identified(full_record());
    no_reg.profile.registration_system = None;
    let bundle = no_reg.build();
    assert_eq!(
        resources(&bundle, "Practitioner")[0]["identifier"]
            .as_array()
            .unwrap()
            .len(),
        1,
        "no registration identifier without a configured system"
    );
}

#[test]
fn one_provenance_per_signature_targets_the_report() {
    let bundle = Case::identified(full_record()).build();
    let report_url = full_url_of(&bundle, "DiagnosticReport");
    let provs = resources(&bundle, "Provenance");
    assert_eq!(provs.len(), 2);
    let types: Vec<&str> = provs
        .iter()
        .map(|p| p["agent"][0]["type"]["coding"][0]["code"].as_str().unwrap())
        .collect();
    assert_eq!(types, ["author", "verifier"]);
    for p in &provs {
        assert_eq!(p["target"], json!([{"reference": report_url}]));
        assert_eq!(
            p["agent"][0]["onBehalfOf"]["reference"],
            full_url_of(&bundle, "Organization")
        );
    }
    assert_eq!(provs[0]["recorded"], "2026-10-02T08:00:00Z");
}

// ---- FHX-7: watermark ----------------------------------------------------------------------

#[test]
fn bundle_identifier_and_tags_carry_the_watermark() {
    let bundle = Case::deidentified().build();
    assert_eq!(
        bundle["identifier"],
        json!({"system": "http://curv.org/fhir/NamingSystem/export", "value": EXPORT_ID})
    );
    let tags: Vec<&str> = bundle["meta"]["tag"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| {
            assert_eq!(t["system"], "http://curv.org/fhir/CodeSystem/export-mode");
            t["code"].as_str().unwrap()
        })
        .collect();
    assert_eq!(tags, ["deidentified", "clinical_care"]);
    assert_eq!(bundle["timestamp"], "2026-10-05T06:15:00Z");
}

// ---- transaction ---------------------------------------------------------------------------

#[test]
fn transaction_uses_conditional_requests_on_identifiers() {
    let mut case = Case::identified(full_record());
    case.kind = BundleKind::Transaction;
    let bundle = case.build();
    assert_eq!(bundle["type"], "transaction");
    let req = |ty: &str| -> Value {
        entries(&bundle)
            .iter()
            .find(|e| e["resource"]["resourceType"] == ty)
            .unwrap()["request"]
            .clone()
    };
    assert_eq!(
        req("Patient"),
        json!({
            "method": "POST",
            "url": "Patient",
            "ifNoneExist": format!("identifier={MRN_SYSTEM}|{}", sentinel::MRN),
        })
    );
    assert_eq!(req("Practitioner")["method"], "POST");
    assert_eq!(
        req("Practitioner")["ifNoneExist"],
        format!(
            "identifier=http://curv.org/fhir/NamingSystem/console-user|{}",
            sentinel::SIGNER_ID
        )
    );
    assert_eq!(req("Organization")["method"], "POST");
    assert_eq!(
        req("DiagnosticReport"),
        json!({
            "method": "PUT",
            "url": "DiagnosticReport?identifier=http://curv.org/fhir/NamingSystem/gaze-session-review|SESS-GOLDEN-0001",
        })
    );
    assert_eq!(
        req("Observation")["url"],
        "Observation?identifier=http://curv.org/fhir/NamingSystem/gaze-session|SESS-GOLDEN-0001"
    );
    assert_eq!(
        req("DocumentReference")["url"],
        format!(
            "DocumentReference?identifier=http://curv.org/fhir/NamingSystem/export|{EXPORT_ID}"
        )
    );
    let device_id = one(&bundle, "Device")["id"].as_str().unwrap().to_owned();
    assert_eq!(
        req("Device"),
        json!({"method": "PUT", "url": format!("Device/{device_id}")})
    );
    assert_eq!(req("Provenance")["method"], "PUT");
}

#[test]
fn conditional_urls_percent_encode_identifier_values() {
    let mut record = full_record();
    record.patient.mrn.as_mut().unwrap().value = "A 1&b=2|c".into();
    let mut case = Case::identified(record);
    case.kind = BundleKind::Transaction;
    let bundle = case.build();
    let patient = entries(&bundle)
        .iter()
        .find(|e| e["resource"]["resourceType"] == "Patient")
        .unwrap();
    assert_eq!(
        patient["request"]["ifNoneExist"],
        format!("identifier={MRN_SYSTEM}|A%201%26b%3D2%7Cc")
    );
}

// ---- XC-8: determinism and goldens ---------------------------------------------------------

#[test]
fn bundle_bytes_are_deterministic() {
    let case = Case::identified(full_record());
    let input = || ClinicalBundleInput {
        record: &case.record,
        gaze: &case.gaze,
        device: device(),
        watermark: &case.watermark,
        profile: &case.profile,
        document: Some(ClinicalDocument {
            kind: DocumentKind::ClinicalSummary,
            pdf: PDF,
            url: None,
        }),
        kind: BundleKind::Collection,
    };
    let a = clinical::encode(&input()).unwrap();
    let b = clinical::encode(&input()).unwrap();
    assert_eq!(a, b);
    let parsed: Value = serde_json::from_slice(&a).unwrap();
    assert_eq!(parsed, case.build());
}

fn golden(name: &str, bundle: &Value) {
    let mut text = serde_json::to_string_pretty(bundle).unwrap();
    text.push('\n');
    support::assert_golden_bytes(&format!("fhir/{name}"), text.as_bytes());
}

#[test]
fn golden_identified_amended_collection() {
    golden(
        "clinical-identified-amended.json",
        &Case::identified(full_record()).build(),
    );
}

#[test]
fn golden_identified_unsigned_collection() {
    golden(
        "clinical-identified-unsigned.json",
        &Case::identified(golden_record()).build(),
    );
}

#[test]
fn golden_deidentified_collection() {
    golden("clinical-deidentified.json", &Case::deidentified().build());
}

#[test]
fn golden_identified_final_transaction() {
    let mut case = Case::identified(with_state(ReviewState::Final, false));
    case.kind = BundleKind::Transaction;
    case.url = Some("https://blob.example/exports/summary.pdf");
    golden("clinical-transaction-final.json", &case.build());
}
